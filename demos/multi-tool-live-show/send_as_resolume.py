#!/usr/bin/env python3
"""Simulates Resolume Arena sending its real composition OSC.

Sends the real OSC address shape Resolume Arena itself sends
(/composition/layers/{layer_id}/video/{parameter}) to the real,
unmodified resolume-pchi-bridge. Resolume Arena is a commercial
Windows/Mac GUI app not installed in this environment, so this script
stands in for it. Layer id "main" is chosen deliberately — see this
demo's README for why, and note resolume-pchi-bridge is control-only:
it has no governance-receive leg to listen on the way the TiXL bridge
and cables.gl relay do, so there's no "listen_as_resolume.py" here.
"""
import argparse
from pythonosc import udp_client


def main():
    parser = argparse.ArgumentParser(description="Send a control update as if from Resolume Arena")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=7000, help="resolume-pchi-bridge --resolume-port")
    parser.add_argument("--layer", default="main")
    parser.add_argument("parameter")
    parser.add_argument("value", type=float)
    args = parser.parse_args()

    client = udp_client.SimpleUDPClient(args.host, args.port)
    address = f"/composition/layers/{args.layer}/video/{args.parameter}"
    client.send_message(address, args.value)
    print(f"[as-Resolume] {address} {args.value} -> {args.host}:{args.port}")


if __name__ == "__main__":
    main()
