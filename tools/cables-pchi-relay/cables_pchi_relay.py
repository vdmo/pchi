#!/usr/bin/env python3
"""
cables.gl PCHI Relay
Connects cables.gl (https://github.com/cables-gl/cables) patches — which
run in a browser sandbox — to the PCHI Conductor.

Unlike TiXL or Resolume, a browser cannot open a raw UDP socket, which is
what the Conductor's control-update path (PCHIConductor::run_udp_receive_loop)
requires. This relay is the translation point: an HTTP endpoint a cables
patch can `fetch()` (any browser-side network primitive works — no
UDP capability needed on the cables side), and a WebSocket server cables'
existing, already-proven WebSocket op can connect to directly
(cables-gl/websocket-example confirms this op exists and works against a
plain `ws` server — this relay's WS side is deliberately that same shape).

    cables (fetch POST)  --HTTP-->  this relay  --raw JSON UDP-->  PCHI Conductor   (control updates)
    PCHI Conductor  --WebSocket-->  this relay  --WebSocket-->  cables               (governance telemetry)

The Conductor leg is raw JSON-over-UDP / a real WebSocket client to the
Conductor's own governance-event WS (port 8889, requires vdmo/pchi#5) —
not translated further. Only the cables-facing side needs browser-safe
transports (HTTP, WebSocket) and CORS.

## Wiring this up inside a cables.gl patch

**Outbound — a value in your patch -> the Conductor:**
Use an HTTP request op (Ajax/Fetch — whatever cables' current op library
calls it) to POST JSON to `http://<this relay>:9100/pchi/control`:
    { "target_id": "kraken_main", "parameter": "wobbliness", "value": 0.42 }

**Inbound — a live governance verdict -> your patch:**
Use cables' WebSocket op, pointed at `ws://<this relay>:9101`. Each
message is JSON with both a flattened convenience shape and the full
original event, in case your op's JSON-path access differs:
    { "gate_state": "DENY", "receipt_id": "...", "tool": "...",
      "agent_id": "...", "action": "...", "raw": { ...full PCHIMessage... } }

Unlike the TiXL bridge, gate_state is sent as text here, not a numeric
code — cables' JSON handling isn't constrained the way TiXL's
float-only OscInput outputs are (confirmed from TiXL's real source; not
independently confirmed for cables, so the numeric code is included too
under `raw`, in case a text comparison turns out to be awkward in your
op graph).

## CORS

A cables.gl patch is served from a page whose origin differs from
`http://localhost:9100` (or wherever this relay runs), so this is a
cross-origin request — browsers send a CORS preflight (OPTIONS) before
the actual POST because it carries a JSON body. Both are handled below.
`--cors-origin` defaults to `*` for local development; tighten it for
anything reachable beyond your own machine.
"""

import argparse
import asyncio
import json
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any, Dict, Optional

import websockets


GATE_STATE_CODES = {"ALLOW": 1.0, "ESCALATE": 0.5, "DENY": 0.0}


def zero_pir_invariants() -> Dict[str, Any]:
    """See tools/tixl-pchi-bridge's identical helper: PCHIMessage.pir_invariants
    has no #[serde(default)] on any field, so a control-parameter/heartbeat
    message (no genuine multi-value invariant to report — see vdmo/pchi#4)
    must still carry a complete, honest zero-residual block."""
    return {
        "equilibrium_check": {"residual": 0.0, "precision": 1e-12, "sign_sequence": []},
        "coherence_gap": 0.0,
        "curvature_signature": "0.0, 0.0, 0.0",
        "residual": 0.0,
    }


def build_wire_message(message_type: str, source_id: str, payload_type: str, data: Dict[str, Any]) -> bytes:
    """The real PCHIMessage wire shape (type/payloadType, not the Rust
    struct's own field names) — see tools/resolume-pchi-bridge's docstring
    for the history of getting this wrong."""
    message = {
        "version": "2.0.0",
        "type": message_type,
        "source_id": source_id,
        "timestamp": time.time() * 1000,
        "pir_invariants": zero_pir_invariants(),
        "payload": {"payloadType": payload_type, "data": data},
    }
    return json.dumps(message).encode("utf-8")


class CablesPCHIRelay:
    def __init__(
        self,
        http_port: int = 9100,
        ws_port: int = 9101,
        pchi_host: str = "127.0.0.1",
        pchi_port: int = 8888,
        pchi_ws_port: int = 8889,
        cors_origin: str = "*",
    ):
        self.http_port = http_port
        self.ws_port = ws_port
        self.pchi_host = pchi_host
        self.pchi_port = pchi_port
        self.pchi_ws_port = pchi_ws_port
        self.cors_origin = cors_origin
        self.running = False

        self.pchi_socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.cables_clients: "set[websockets.ServerConnection]" = set()
        self._loop: Optional[asyncio.AbstractEventLoop] = None

        print("cables.gl PCHI Relay initialized")
        print(f"  HTTP (cables -> Conductor) on http://0.0.0.0:{http_port}/pchi/control")
        print(f"  WebSocket (Conductor -> cables) on ws://0.0.0.0:{ws_port}")
        print(f"  Conductor at {pchi_host}:{pchi_port} (UDP), governance WS at {pchi_host}:{pchi_ws_port}")

    # ---- cables -> Conductor -------------------------------------------

    def send_control_update(self, target_id: str, parameter: str, value: float):
        wire = build_wire_message(
            "control_parameter",
            "cables_relay",
            "control_parameter",
            {"target_id": target_id, "parameter": parameter, "value": float(value)},
        )
        self.pchi_socket.sendto(wire, (self.pchi_host, self.pchi_port))

    def send_heartbeat(self):
        while self.running:
            try:
                wire = build_wire_message(
                    "heartbeat",
                    "cables_relay",
                    "control_parameter",
                    {"target_id": "cables_relay", "parameter": "heartbeat", "value": 1.0},
                )
                self.pchi_socket.sendto(wire, (self.pchi_host, self.pchi_port))
            except OSError as e:
                print(f"Error sending heartbeat: {e}")
            time.sleep(5)

    # ---- Conductor -> cables --------------------------------------------

    async def watch_conductor_governance(self):
        """Reconnect loop for the Conductor's governance-event WebSocket.
        Requires vdmo/pchi#5 — without it the Conductor's WS is bound but
        never accepts a connection and this call blocks (harmlessly) until
        cancelled, never actually receiving anything."""
        target = f"ws://{self.pchi_host}:{self.pchi_ws_port}"
        while self.running:
            try:
                # ping_timeout=60 (the `websockets` library default is 20):
                # a busy peer — a loaded conductor, a CPU-starved browser
                # tab on the cables side of this relay — can miss a pong
                # well inside 20s without actually being dead. Seen in
                # practice: real "1011 keepalive ping timeout" disconnects
                # under heavy unrelated CPU load, not a sign either side
                # had actually stopped responding.
                async with websockets.connect(target, ping_timeout=60) as ws:
                    print(f"[Conductor WS] connected to {target}")
                    async for raw in ws:
                        await self._handle_conductor_message(raw)
            except Exception as e:  # noqa: BLE001 - long-lived watcher, reconnect on anything
                print(f"[Conductor WS] error: {e} — retrying")
            if self.running:
                await asyncio.sleep(2)

    async def _handle_conductor_message(self, raw: str):
        try:
            msg = json.loads(raw)
        except json.JSONDecodeError:
            return
        payload = msg.get("payload", {})
        if payload.get("payloadType") != "governance_event":
            return
        data = payload.get("data", {})
        gate_state = data.get("gate_state")
        out = {
            "gate_state": gate_state,
            "gate_state_code": GATE_STATE_CODES.get(gate_state),
            "receipt_id": data.get("receipt_id"),
            "tool": data.get("tool"),
            "agent_id": data.get("agent_id"),
            "action": data.get("action"),
            "raw": msg,
        }
        await self._broadcast_to_cables(json.dumps(out))

    async def _broadcast_to_cables(self, text: str):
        stale = set()
        for client in self.cables_clients:
            try:
                await client.send(text)
            except websockets.ConnectionClosed:
                stale.add(client)
        self.cables_clients -= stale

    async def _cables_ws_handler(self, websocket):
        self.cables_clients.add(websocket)
        print(f"[cables WS] client connected ({len(self.cables_clients)} total)")
        try:
            async for _ in websocket:
                pass  # this relay is receive-only from cables over WS; control updates use HTTP
        finally:
            self.cables_clients.discard(websocket)
            print(f"[cables WS] client disconnected ({len(self.cables_clients)} total)")

    # ---- Lifecycle ---------------------------------------------------------

    def _run_asyncio_loop(self):
        self._loop = asyncio.new_event_loop()
        asyncio.set_event_loop(self._loop)

        async def main():
            # Same reasoning as the Conductor-facing client above: a
            # cables.gl tab running in a CPU-starved browser can miss a
            # pong well inside the library's 20s default without its
            # connection actually being dead.
            async with websockets.serve(self._cables_ws_handler, "0.0.0.0", self.ws_port, ping_timeout=60):
                print(f"cables-facing WebSocket server listening on port {self.ws_port}")
                await self.watch_conductor_governance()

        self._loop.run_until_complete(main())

    def start(self):
        self.running = True
        relay = self

        class ControlHandler(BaseHTTPRequestHandler):
            def _cors_headers(self):
                self.send_header("Access-Control-Allow-Origin", relay.cors_origin)
                self.send_header("Access-Control-Allow-Methods", "POST, OPTIONS")
                self.send_header("Access-Control-Allow-Headers", "Content-Type")

            def do_OPTIONS(self):  # CORS preflight — a JSON POST body triggers this in every browser
                self.send_response(204)
                self._cors_headers()
                self.end_headers()

            def do_GET(self):
                if self.path != "/status":
                    self.send_response(404)
                    self._cors_headers()
                    self.end_headers()
                    return
                self.send_response(200)
                self._cors_headers()
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"cables_clients_connected": len(relay.cables_clients)}).encode())

            def do_POST(self):
                if self.path != "/pchi/control":
                    self.send_response(404)
                    self._cors_headers()
                    self.end_headers()
                    return
                try:
                    length = int(self.headers.get("Content-Length", 0))
                    body = json.loads(self.rfile.read(length))
                    relay.send_control_update(body["target_id"], body["parameter"], float(body["value"]))
                    self.send_response(200)
                    self._cors_headers()
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(b'{"ok":true}')
                except (KeyError, ValueError, json.JSONDecodeError) as e:
                    self.send_response(400)
                    self._cors_headers()
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps({"ok": False, "error": str(e)}).encode())

            def log_message(self, fmt, *args):
                print(f"[HTTP] {fmt % args}")

        http_server = ThreadingHTTPServer(("0.0.0.0", self.http_port), ControlHandler)
        threading.Thread(target=http_server.serve_forever, daemon=True).start()
        threading.Thread(target=self.send_heartbeat, daemon=True).start()

        try:
            self._run_asyncio_loop()
        except KeyboardInterrupt:
            print("\nShutting down...")
        finally:
            self.running = False
            http_server.shutdown()


def main():
    parser = argparse.ArgumentParser(description="cables.gl PCHI Relay")
    parser.add_argument("--http-port", type=int, default=9100, help="HTTP port for cables control updates (default: 9100)")
    parser.add_argument("--ws-port", type=int, default=9101, help="WebSocket port for cables governance telemetry (default: 9101)")
    parser.add_argument("--pchi-host", type=str, default="127.0.0.1", help="PCHI Conductor host (default: 127.0.0.1)")
    parser.add_argument("--pchi-port", type=int, default=8888, help="PCHI Conductor UDP port (default: 8888)")
    parser.add_argument("--pchi-ws-port", type=int, default=8889, help="PCHI Conductor governance-event WS port (default: 8889)")
    parser.add_argument("--cors-origin", type=str, default="*", help="Access-Control-Allow-Origin value (default: *)")
    args = parser.parse_args()

    relay = CablesPCHIRelay(
        http_port=args.http_port,
        ws_port=args.ws_port,
        pchi_host=args.pchi_host,
        pchi_port=args.pchi_port,
        pchi_ws_port=args.pchi_ws_port,
        cors_origin=args.cors_origin,
    )
    relay.start()


if __name__ == "__main__":
    main()
