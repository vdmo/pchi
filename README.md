# PCHI (Peachy) 2.0 Protocol

Mathematical Governance for Scene State. 

This repository contains all the core libraries, tools, and documentation needed to run and integrate the PCHI protocol for your live shows. 

## Repository Structure

- `pchi-schema/`: The core JSON and FlatBuffers schema definitions, plus the Rust validation and parsing library.
- `primeswarm-pchi/`: The PrimeSwarm Engine (PCHI Conductor). The central state manager that enforces Only-Lang rules and PIR invariants.
- `tools/`: Ready-to-use bridges and plugins for creative software:
  - `resolume-pchi-bridge/`: Python OSC bridge for Resolume Arena.
  - `touchdesigner-pchi-chop/`: Python CHOP for TouchDesigner.
  - `ableton-pchi-max/`: Max for Live device for Ableton Live.
- `docs/`: The full documentation and presentation website. 

For full details, please open `docs/index.html` or check the individual `README.md` files in each sub-directory.
