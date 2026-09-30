# Limitations

What the governance system in this repo does and doesn't prove, stated
plainly, in the same spirit as DGV's own `LIMITATIONS.md`
(`only-dgv-verifier/docs/LIMITATIONS.md`) — a claim without its limits
next to it isn't trustworthy, whichever direction it's wrong in.

## 1. The Only-Lang Rule Grammar Is a Documented Subset, Not the Full Language

`primeswarm-pchi/src/only_lang.rs` parses and evaluates real `.only-pchi`
files: `harmony(...)`, `if <condition> then <actions> end`, `and`/`or`
conditions, `escalate`/`deny`/`evolve`/`residual` actions.

- ✅ `pchi-schema/examples/kraken-tentacle.only-pchi` is real, loaded by
  default, and every rule in it is exercised by tests
  (`primeswarm-pchi/src/only_lang.rs`, `src/rules.rs`).
- ❌ `for` loops and `sum(... for i in 0..8)` comprehensions — used in the
  original `pchi-schema/examples/kraken-rules.only` — are **not**
  supported. That file is kept as the target-syntax reference and is
  explicitly labeled as such at its head; loading it with the current
  parser fails with a clear "unsupported top-level statement" error rather
  than silently misparsing it.
- ❌ No parenthesized grouping in conditions — `and`/`or` chains evaluate
  strictly left to right.

**Mitigation today:** write rules within the documented subset (see
`primeswarm-pchi/src/only_lang.rs`'s module doc for the exact grammar).
**Not yet built:** loop/comprehension support.

## 2. A Signed Receipt Proves the Rule Engine Made a Decision, Not That the Decision Was Correct

Every fired rule is signed and chained — that's a cryptographic guarantee
about *what the conductor decided and when*, re-derivable offline. It is
**not** a guarantee that the rule itself encodes the right safety
threshold. A `.only-pchi` file with `deny(...)` set at the wrong number is
just as signable as a correctly-tuned one.

**Mitigation:** rule content review is a separate concern from receipt
verification — see `PRICING.md`'s certified-rule-packs section, which is
exactly where domain review is meant to live. The signed chain proves
enforcement happened as configured; it does not audit the configuration.

## 3. Single-Process Chain, No Multi-Instance Reconciliation

Unlike DGV's gate (which supports peer gossip and Merkle anti-entropy
reconciliation across instances sharing Postgres —
`only-dgv-verifier/docs/LIMITATIONS.md` §1), `primeswarm-pchi`'s
governance log is local to one conductor process, backed by a single JSONL
file. Two conductors (e.g. one per venue in a multi-venue show, the
"Distributed Multi-Venue Shows" application described in
`CAPABILITIES_AND_APPLICATIONS.md`) produce two independent chains with no
built-in reconciliation.

**Mitigation today:** export and archive each conductor's chain
separately; there is no cross-conductor verification story yet.
**Not yet built:** the Cloud Conductor described in `PRICING.md` is the
natural place to add this.

## 4. `/governance/export` Access Control Is Opt-In, Not Default-Closed

`PCHI_GOVERNANCE_KEY` gates the export endpoint when set; it is **open by
default**, deliberately, because this conductor is meant to be
self-hosted on a machine its operator already controls — unlike DGV's
`/decisions/export`, which was found open on a genuinely multi-tenant,
publicly reachable deployment and is now closed by default (see DGV's
`LIMITATIONS.md` §4 for that finding). If you expose a PCHI conductor
beyond localhost, set the key yourself; the software will not do it for
you.

## 5. No Independent Security Review

Nobody outside this project has attempted to break the signing, the
parser, or the export endpoint. The tamper-detection tests in
`primeswarm-pchi/src/governance.rs` and the live test in this change's
commit message cover the failure modes we thought to check, not an
adversarial review.

## 6. FlatBuffers Is a Schema, Not a Wire Format Actually Used

`pchi-schema/pchi.fbs` exists and `flatbuffers` is a dependency of
`pchi-schema`, but nothing in `primeswarm-pchi` actually serializes to it
at runtime — the UDP receive loop and the WebSocket dashboard both use
JSON (`serde_json`). "Multiple Data Formats: JSON and FlatBuffers" in
`pchi-schema/README.md` describes the schema's design, not the
conductor's current behavior.

**Not yet built:** a FlatBuffers encode/decode path actually wired into
`conductor.rs`'s message handling.

## 7. Governance-Event Messages From Other Conductors Are Logged, Not Re-Verified

A `MessageType::GovernanceEvent` received from another source is recorded
as informational (`conductor.rs`) but its embedded `receipt_id` is not
automatically checked against the originating conductor's own export —
that would require fetching that conductor's `/governance/export` and
running the same verification locally. A receiving dashboard that wants
that guarantee has to do it itself today.

**Not yet built:** an automatic cross-conductor receipt-verification
fetch.
