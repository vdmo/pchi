#!/usr/bin/env python3
"""
TiXL PCHI Bridge
Connects TiXL (https://github.com/tixl3d/tixl) to the PCHI Conductor via OSC.

TiXL's OscOutput/OscInput operators use Rug.Osc — a real, spec-compliant
OSC 1.0 library — so this bridge, unlike the first version of the Resolume
bridge, does not need to guess at wire compatibility: it uses python-osc,
which speaks the same real OSC framing. Verified against TiXL's actual
operator source (Operators/Io/Symbols/lib/io/osc/{OscOutput,OscInput}.cs),
not just its docs.

Two independent legs, each a real UDP socket, deliberately not shared:

  TiXL --OSC--> this bridge --raw JSON UDP--> PCHI Conductor  (control updates)
  PCHI Conductor --WebSocket--> this bridge --OSC--> TiXL      (governance telemetry)

The PCHI Conductor leg is raw JSON-over-UDP, not OSC — see
resolume-pchi-bridge's docstring for why sending OSC-wrapped JSON to the
conductor fails outright (its serde_json::from_slice has no OSC decoder).
The TiXL legs are real OSC in both directions.

## Wiring this up inside TiXL

**Outbound (a value in your graph -> the Conductor):**
Add an `OscOutput` operator. Set:
  - IpAddress / Port: this bridge's --osc-in-port (default 9001)
  - Address: `/pchi/control` (fixed — see below for why)
  - Values: your float value
  - Strings: a single string `"<target_id>.<parameter>"`, e.g. `"kraken_main.wobbliness"`
A fixed address with the target/parameter passed as a string payload,
rather than encoding them into the OSC address itself, avoids needing
string-formatting nodes in the graph just to build a dynamic address —
and avoids ordering ambiguity between multiple MultiInputSlot strings.

**Inbound (a live governance verdict -> your graph):**
Add an `OscInput` operator listening on --osc-out-port (default 9002).
It will receive:
  - `/pchi/governance/gate_state` (float): 1.0 = ALLOW, 0.5 = ESCALATE,
    0.0 = DENY. OscInput's Contents/Values outputs are float-only (no
    string channel — confirmed from its actual source), so gate_state is
    sent as a numeric code, not text; wire it into a Less/Greater node to
    gate downstream output on a live verdict (e.g. force a DMX dimmer to
    zero when gate_state < 1.0).

## What this does NOT do

This bridge does not implement the DMX-safety use case itself — that is
downstream, an artist's own graph wiring, gating one of TiXL's DMX
operators on `/pchi/governance/gate_state`. This only carries the signal.
"""

import argparse
import json
import socket
import threading
import time
from dataclasses import dataclass
from typing import Any, Dict, Optional

from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import BlockingOSCUDPServer
from pythonosc import udp_client

try:
    import websocket  # websocket-client
except ImportError:
    websocket = None


GATE_STATE_CODES = {"ALLOW": 1.0, "ESCALATE": 0.5, "DENY": 0.0}


@dataclass
class PCHIMessage:
    """PCHI v2.0 message. See to_wire_dict() for the actual wire shape —
    the field names here are Python-conventional, not the wire names."""
    version: str = "2.0.0"
    message_type: str = "control_parameter"
    source_id: str = "tixl_bridge"
    timestamp: float = 0.0
    pir_invariants: Dict[str, Any] = None
    payload: Dict[str, Any] = None

    def __post_init__(self):
        if self.timestamp == 0.0:
            self.timestamp = time.time() * 1000
        if self.pir_invariants is None:
            self.pir_invariants = {}
        if self.payload is None:
            self.payload = {}

    def to_wire_dict(self) -> Dict[str, Any]:
        payload = dict(self.payload)
        if "payload_type" in payload:
            payload["payloadType"] = payload.pop("payload_type")
        return {
            "version": self.version,
            "type": self.message_type,
            "source_id": self.source_id,
            "timestamp": self.timestamp,
            "pir_invariants": self.pir_invariants,
            "payload": payload,
        }


def zero_pir_invariants() -> Dict[str, Any]:
    """An honest, structurally-complete zero-residual invariant set for
    messages (control updates, heartbeats) that don't carry a genuine
    multi-value equilibrium claim. PCHIMessage's schema requires the full
    shape on every message regardless of type — see PIRInvariants in
    pchi-schema/src/lib.rs, no #[serde(default)] on any field."""
    return {
        "equilibrium_check": {"residual": 0.0, "precision": 1e-12, "sign_sequence": []},
        "coherence_gap": 0.0,
        "curvature_signature": "0.0, 0.0, 0.0",
        "residual": 0.0,
    }


class TiXLPCHIBridge:
    def __init__(
        self,
        osc_in_port: int = 9001,
        osc_out_host: str = "127.0.0.1",
        osc_out_port: int = 9002,
        pchi_host: str = "127.0.0.1",
        pchi_port: int = 8888,
        pchi_ws_port: int = 8889,
    ):
        self.osc_in_port = osc_in_port
        self.pchi_host = pchi_host
        self.pchi_port = pchi_port
        self.pchi_ws_port = pchi_ws_port
        self.running = False

        # Leg 1: this bridge -> PCHI Conductor. Raw JSON over UDP, NOT OSC —
        # see module docstring and resolume-pchi-bridge's history for why.
        self.pchi_socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)

        # Leg 2: this bridge -> TiXL's OscInput. Real OSC.
        self.tixl_osc_client = udp_client.SimpleUDPClient(osc_out_host, osc_out_port)

        self._last_gate_state: Optional[str] = None

        print("TiXL PCHI Bridge initialized")
        print(f"  Listening for TiXL OSC (OscOutput) on port {osc_in_port}")
        print(f"  Sending governance telemetry (OscInput) to {osc_out_host}:{osc_out_port}")
        print(f"  Sending control updates to PCHI Conductor at {pchi_host}:{pchi_port}")
        print(f"  Watching governance events on ws://{pchi_host}:{pchi_ws_port}")

    # ---- Leg 1: TiXL -> PCHI --------------------------------------------

    def send_pchi_message(self, message: PCHIMessage):
        try:
            message_bytes = json.dumps(message.to_wire_dict()).encode("utf-8")
            self.pchi_socket.sendto(message_bytes, (self.pchi_host, self.pchi_port))
        except OSError as e:
            print(f"Error sending PCHI message: {e}")

    def handle_control_osc(self, address: str, *args):
        """Handle /pchi/control from TiXL's OscOutput.

        OscOutput sends its Values (float), IntValues (int), then Strings
        (string) parameters in that fixed order (confirmed from
        OscOutput.cs's `parameters` array construction) — so with one
        Values input and one Strings input wired, args arrive as
        (value: float, key: str).
        """
        if len(args) < 2:
            print(f"[OSC] {address}: expected (value, key), got {args!r} — ignoring")
            return
        value, key = args[0], args[1]
        if "." not in key:
            print(f"[OSC] {address}: key '{key}' must be 'target_id.parameter' — ignoring")
            return
        target_id, parameter = key.split(".", 1)

        message = PCHIMessage(
            message_type="control_parameter",
            source_id="tixl_bridge",
            pir_invariants=zero_pir_invariants(),
            payload={
                "payload_type": "control_parameter",
                "data": {"target_id": target_id, "parameter": parameter, "value": float(value)},
            },
        )
        self.send_pchi_message(message)
        print(f"[OSC] {key} = {value} -> PCHI sent")

    def send_heartbeat(self):
        while self.running:
            try:
                heartbeat = PCHIMessage(
                    message_type="heartbeat",
                    source_id="tixl_bridge",
                    pir_invariants=zero_pir_invariants(),
                    payload={
                        "payload_type": "control_parameter",
                        "data": {"target_id": "tixl_bridge", "parameter": "heartbeat", "value": 1.0},
                    },
                )
                self.send_pchi_message(heartbeat)
                time.sleep(5)
            except OSError as e:
                print(f"Error sending heartbeat: {e}")
                time.sleep(1)

    # ---- Leg 2: PCHI -> TiXL ---------------------------------------------

    def _on_ws_message(self, _ws, raw: str):
        try:
            msg = json.loads(raw)
        except json.JSONDecodeError:
            return
        payload = msg.get("payload", {})
        if payload.get("payloadType") != "governance_event":
            return
        gate_state = payload.get("data", {}).get("gate_state")
        code = GATE_STATE_CODES.get(gate_state)
        if code is None:
            print(f"[WS] unknown gate_state {gate_state!r} — not forwarded")
            return
        self.tixl_osc_client.send_message("/pchi/governance/gate_state", code)
        self._last_gate_state = gate_state
        print(f"[WS] governance_event gate_state={gate_state} -> OSC /pchi/governance/gate_state={code}")

    def _on_ws_error(self, _ws, error):
        print(f"[WS] error: {error}")

    def _on_ws_close(self, _ws, *_args):
        print("[WS] connection closed")

    def watch_governance_events(self):
        """Reconnect loop for the governance-event WebSocket. Requires
        vdmo/pchi#5 (the WS accept-loop fix) — without it, the conductor
        binds :8889 but never accepts a connection, and this call blocks
        forever without ever calling _on_ws_message."""
        if websocket is None:
            print("websocket-client not installed (`pip install websocket-client`) — "
                  "governance telemetry to TiXL is disabled; control updates still work.")
            return
        while self.running:
            try:
                ws = websocket.WebSocketApp(
                    f"ws://{self.pchi_host}:{self.pchi_ws_port}",
                    on_message=self._on_ws_message,
                    on_error=self._on_ws_error,
                    on_close=self._on_ws_close,
                )
                ws.run_forever()
            except Exception as e:  # noqa: BLE001 - reconnect on anything, this is a long-lived watcher
                print(f"[WS] connection failed: {e}")
            if self.running:
                time.sleep(2)  # backoff before reconnecting

    # ---- Lifecycle ---------------------------------------------------------

    def start(self):
        self.running = True

        dispatcher = Dispatcher()
        dispatcher.map("/pchi/control", self.handle_control_osc)

        threading.Thread(target=self.send_heartbeat, daemon=True).start()
        threading.Thread(target=self.watch_governance_events, daemon=True).start()

        server = BlockingOSCUDPServer(("0.0.0.0", self.osc_in_port), dispatcher)
        print(f"OSC server listening on port {self.osc_in_port}")
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down...")
        finally:
            self.running = False


def main():
    parser = argparse.ArgumentParser(description="TiXL PCHI Bridge")
    parser.add_argument("--osc-in-port", type=int, default=9001, help="Port to listen for TiXL's OscOutput (default: 9001)")
    parser.add_argument("--osc-out-host", type=str, default="127.0.0.1", help="Host running TiXL's OscInput (default: 127.0.0.1)")
    parser.add_argument("--osc-out-port", type=int, default=9002, help="Port for TiXL's OscInput (default: 9002)")
    parser.add_argument("--pchi-host", type=str, default="127.0.0.1", help="PCHI Conductor host (default: 127.0.0.1)")
    parser.add_argument("--pchi-port", type=int, default=8888, help="PCHI Conductor UDP port (default: 8888)")
    parser.add_argument("--pchi-ws-port", type=int, default=8889, help="PCHI Conductor governance-event WS port (default: 8889)")
    args = parser.parse_args()

    bridge = TiXLPCHIBridge(
        osc_in_port=args.osc_in_port,
        osc_out_host=args.osc_out_host,
        osc_out_port=args.osc_out_port,
        pchi_host=args.pchi_host,
        pchi_port=args.pchi_port,
        pchi_ws_port=args.pchi_ws_port,
    )
    bridge.start()


if __name__ == "__main__":
    main()
