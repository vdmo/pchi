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

## Building

```bash
cargo build   # workspace root — builds pchi-schema and primeswarm-pchi
cargo test    # 40 tests: schema validation, the Only-Lang parser/evaluator,
              # signed-receipt chaining and tamper detection, end-to-end
              # rule denial through the conductor, dashboard auth
```

## Governance — what's real

`primeswarm-pchi` loads real `.only-pchi` rule files (not hardcoded Rust
structs — see `primeswarm-pchi/src/only_lang.rs`), evaluates them against
every message received over its UDP transport (not just a startup demo
message), and signs + hash-chains every fired rule into an append-only log
(`primeswarm-pchi/src/governance.rs`), independently verifiable offline:

```bash
cargo run -p primeswarm-pchi &
curl http://127.0.0.1:3000/governance/export > export.json
python3 scripts/verify_governance_chain.py export.json
```

A fresh checkout runs the bundled default rule set
(`pchi-schema/examples/kraken-tentacle.only-pchi`). Point it at a
different `.only-pchi` file for a real venue/show instead — no recompile
needed:

```bash
cargo run -p primeswarm-pchi -- --rules ./my-venue-rules.only-pchi
# or: PCHI_RULES_FILE=./my-venue-rules.only-pchi cargo run -p primeswarm-pchi
```

The dashboard, its `/ws` feed, and `/governance/export` are open by
default (self-hosted on a machine its operator already controls). Set
`PCHI_DASHBOARD_PASSWORD` to require HTTP Basic Auth (username
`operator`) before exposing a conductor beyond localhost:

```bash
PCHI_DASHBOARD_PASSWORD=change-me cargo run -p primeswarm-pchi
curl -u operator:change-me http://127.0.0.1:3000/governance/export
```

See `PRICING.md` for what's free (all of the above, unrestricted,
self-hosted) versus what's a real paid service, and `docs/LIMITATIONS.md`
for what this does and doesn't yet prove.

For full details, please open `docs/index.html` or check the individual `README.md` files in each sub-directory.
