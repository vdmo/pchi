# TiXL PCHI Bridge

Connects [TiXL](https://github.com/tixl3d/tixl) — open-source real-time motion graphics — to the PCHI Conductor via OSC.

## Overview

Two independent legs:

```
TiXL (OscOutput) --OSC-->  this bridge  --raw JSON UDP--> PCHI Conductor   (control updates)
PCHI Conductor --WebSocket--> this bridge  --OSC-->  TiXL (OscInput)       (governance telemetry, live verdicts)
```

Unlike the Resolume bridge, this one didn't need to guess at wire compatibility: TiXL's `OscOutput`/`OscInput` operators are built on `Rug.Osc`, a real spec-compliant OSC library — confirmed directly from TiXL's own source (`Operators/Io/Symbols/lib/io/osc/{OscOutput,OscInput}.cs`), not just its docs. `python-osc` speaks the same real OSC framing, so the TiXL-facing legs work without translation guesswork.

## Requirements

```bash
pip install python-osc websocket-client
```

`websocket-client` is optional — without it, control updates (TiXL → PCHI) still work; only governance telemetry (PCHI → TiXL) is disabled, with a clear message at startup.

## Usage

```bash
python tixl_pchi_bridge.py \
    --osc-in-port 9001 \
    --osc-out-host 127.0.0.1 \
    --osc-out-port 9002 \
    --pchi-host 127.0.0.1 \
    --pchi-port 8888 \
    --pchi-ws-port 8889
```

**Governance telemetry requires [vdmo/pchi#5](https://github.com/vdmo/pchi/pull/5)** (the WebSocket accept-loop fix) — without it, the conductor's governance-event WebSocket is bound but never accepts a connection, and this bridge's telemetry leg will silently receive nothing, forever. Control updates work regardless.

## Wiring this up inside TiXL

**Outbound — a value in your graph → the Conductor:**

Add an `OscOutput` operator:
| Parameter | Value |
|---|---|
| IpAddress / Port | this bridge's host, `--osc-in-port` (default 9001) |
| Address | `/pchi/control` (fixed) |
| Values | your float value |
| Strings | one string: `"<target_id>.<parameter>"`, e.g. `"kraken_main.wobbliness"` |

A fixed address with target/parameter passed as a string payload avoids needing string-formatting nodes just to build a dynamic address, and avoids ordering ambiguity between multiple `Strings` inputs.

**Inbound — a live governance verdict → your graph:**

Add an `OscInput` operator listening on `--osc-out-port` (default 9002). It receives:

| Address | Type | Meaning |
|---|---|---|
| `/pchi/governance/gate_state` | float | `1.0` = ALLOW, `0.5` = ESCALATE, `0.0` = DENY |

`OscInput`'s outputs (`Contents`, `Values`) are float-only — confirmed from its source, there's no string channel — so `gate_state` is sent as a numeric code, not text. Wire it into a comparison node to gate downstream output on a live verdict: for example, force a DMX dimmer or strobe operator to zero when `gate_state < 1.0`, driven by a real signed decision rather than a locally-duplicated safety check.

## What this does not do

This bridge only carries the signal — it does not itself implement any safety behavior. Gating a DMX output on `/pchi/governance/gate_state` is downstream graph wiring, done by whoever builds the show, using whichever Only-Lang rule (`.only-pchi` file) actually defines the limit.

## Verification status

Every claim above about TiXL's OSC wire format is verified against TiXL's actual operator source, not its docs alone. The bridge itself has been tested live, end to end, against a real PCHI Conductor, in both directions — but **not against a running instance of TiXL**, which is a Windows/DirectX GUI application. Testing used real OSC traffic constructed to exactly match what `OscOutput.cs` sends and what `OscInput.cs` expects (confirmed from source), sent and received by standalone `python-osc` clients standing in for TiXL. If you run this against real TiXL and hit a mismatch, please open an issue — the wire format was verified by reading, not by running the real application.
