#!/usr/bin/env python3
"""Simulates TiXL's OscOutput operator sending a control update.

Not a fake protocol — this sends the exact real OSC shape TiXL's
OscOutput would (fixed address /pchi/control, Values=[float],
Strings=["target_id.parameter"]), confirmed from TiXL's actual source
(Operators/Io/Symbols/lib/io/osc/OscOutput.cs). TiXL itself is a
Windows/.NET GUI editor not installed in this environment, so this
script stands in for it; the bridge receiving this message is the real,
unmodified tixl-pchi-bridge.
"""
import argparse
from pythonosc import udp_client


def main():
    parser = argparse.ArgumentParser(description="Send a control update as if from TiXL's OscOutput")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=9001, help="tixl-pchi-bridge --osc-in-port")
    parser.add_argument("target_id")
    parser.add_argument("parameter")
    parser.add_argument("value", type=float)
    args = parser.parse_args()

    client = udp_client.SimpleUDPClient(args.host, args.port)
    key = f"{args.target_id}.{args.parameter}"
    client.send_message("/pchi/control", [args.value, key])
    print(f"[as-TiXL] /pchi/control {args.value} \"{key}\" -> {args.host}:{args.port}")


if __name__ == "__main__":
    main()
