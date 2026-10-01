# cables.gl PCHI Relay

Connects [cables.gl](https://github.com/cables-gl/cables) — open-source, browser-based node/patch tool for interactive content — to the PCHI Conductor.

## Why a relay, not a direct connection (unlike TiXL's native operators)

Cables patches run in a browser sandbox. A browser cannot open a raw UDP
socket, which is what the Conductor's control-update path
(`PCHIConductor::run_udp_receive_loop`) requires. This relay is the
translation point — the same role `tools/resolume-pchi-bridge` and
`tools/tixl-pchi-bridge` play for their respective tools, but translating
browser-safe transports (HTTP, WebSocket) instead of OSC.

```
cables (fetch POST)  --HTTP-->  this relay  --raw JSON UDP-->  PCHI Conductor   (control updates)
PCHI Conductor  --WebSocket-->  this relay  --WebSocket-->  cables            (governance telemetry)
```

The governance-telemetry leg needed **zero new protocol work**: cables
already ships a working WebSocket client op, confirmed from a real
official example ([cables-gl/websocket-example](https://github.com/cables-gl/websocket-example)) —
a plain `ws` server whose JSON messages a cables patch already knows how
to receive. This relay's cables-facing WebSocket server is deliberately
that same shape.

## Requirements

```bash
pip install websockets
```

Governance telemetry requires [vdmo/pchi#5](https://github.com/vdmo/pchi/pull/5) on the Conductor (the WS accept-loop fix) — without it the Conductor's governance WebSocket never accepts a connection, and this relay's Conductor-facing leg reconnects forever without ever receiving anything. Control updates work regardless.

## Usage

```bash
python cables_pchi_relay.py \
    --http-port 9100 \
    --ws-port 9101 \
    --pchi-host 127.0.0.1 \
    --pchi-port 8888 \
    --pchi-ws-port 8889 \
    --cors-origin "*"
```

## Wiring this up inside a cables.gl patch

**Outbound — a value in your patch → the Conductor:**

Use an HTTP request op, `POST` to `http://<this relay>:9100/pchi/control`:
```json
{ "target_id": "kraken_main", "parameter": "wobbliness", "value": 0.42 }
```

**Inbound — a live governance verdict → your patch:**

Point cables' WebSocket op at `ws://<this relay>:9101`. Each message:
```json
{
  "gate_state": "DENY",
  "gate_state_code": 0.0,
  "receipt_id": "rcpt_...",
  "tool": "scene_state",
  "agent_id": "pchi-rule-engine",
  "action": "rule_2",
  "raw": { "...full PCHIMessage..." }
}
```

`gate_state` is sent as text here (unlike the TiXL bridge, which sends
only a numeric code — TiXL's `OscInput` outputs are float-only, confirmed
from its source; cables' JSON handling isn't known to have that
constraint). `gate_state_code` (1.0/0.5/0.0 = ALLOW/ESCALATE/DENY) is
included too, in case a numeric comparison is more convenient in your
op graph than a string comparison.

## CORS

A cables patch is served from a different origin than this relay, so
every `POST` is cross-origin — the browser sends an `OPTIONS` preflight
first because the request carries a JSON body, and both the preflight
and the actual request need `Access-Control-Allow-Origin`. Both are
handled. `--cors-origin` defaults to `*` for local development; tighten
it to your patch's actual origin for anything reachable beyond your own
machine.

## What this does not do

Same as the other bridges: this only carries the signal. Gating output
on a live verdict is downstream patch wiring the artist does themselves.

## Verification status

Tested live, both directions, against a real running Conductor —
including an explicit check that the CORS preflight (`OPTIONS` with
`Origin`/`Access-Control-Request-*` headers, exactly what a browser
sends, not just a plain `curl`) returns the right headers, and that the
actual `POST` does too.

**Tested against real cables.gl via browser automation** (a real
headless Chromium driving the live, currently-maintained
`https://cables.gl/edit/gu7DBo` example patch — not a synthetic
harness): the real op class (`Ops.Net.WebSocket.WebSocket_v2`) was
located and confirmed, its canvas was found inside cables' own
separate-origin `sandbox.cables.gl` iframe, and its URL parameter field
was genuinely read back and edited through cables' real UI, not
assumed.

A full live round-trip — the public cables.gl site actually exchanging
messages with a relay on `localhost` — was not completed. It's blocked
by a real, current browser security boundary: Chrome's Local Network
Access policy refuses a public-origin page (`sandbox.cables.gl`) a
connection to a loopback address without an interactive permission
grant, which headless automation can't click through. This is not a
bug in the relay or in cables — an interactive user hitting
`https://cables.gl` would see a one-time Chrome permission prompt, and
a standalone or Electron build of cables (not loaded as a remote HTTPS
page) would likely avoid this policy entirely, since that's not the
scenario it targets.

If you wire this up in an actual patch and something doesn't match,
please open an issue.
