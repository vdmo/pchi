#!/usr/bin/env python3
"""Simulates TiXL's OscInput operator receiving live governance verdicts.

Listens on the real tixl-pchi-bridge's --osc-out-port for the real
/pchi/governance/gate_state float it emits (1.0/0.5/0.0 =
ALLOW/ESCALATE/DENY, confirmed float-only from OscInput's real source —
see tixl-pchi-bridge's own docstring) and prints each one as it arrives,
standing in for TiXL's own OscInput operator since TiXL's GUI editor
isn't installed in this environment.
"""
import argparse
import time
from pythonosc.dispatcher import Dispatcher
from pythonosc.osc_server import BlockingOSCUDPServer

GATE_NAMES = {1.0: "ALLOW", 0.5: "ESCALATE", 0.0: "DENY"}


def handle_gate_state(address, *args):
    code = args[0]
    name = GATE_NAMES.get(code, f"unknown({code})")
    print(f"[as-TiXL OscInput] {time.strftime('%H:%M:%S')} {address} = {code} ({name})")


def main():
    parser = argparse.ArgumentParser(description="Listen for governance verdicts as if from TiXL's OscInput")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9002, help="tixl-pchi-bridge --osc-out-port")
    args = parser.parse_args()

    dispatcher = Dispatcher()
    dispatcher.map("/pchi/governance/gate_state", handle_gate_state)
    server = BlockingOSCUDPServer((args.host, args.port), dispatcher)
    print(f"[as-TiXL OscInput] listening on {args.host}:{args.port}")
    server.serve_forever()


if __name__ == "__main__":
    main()
