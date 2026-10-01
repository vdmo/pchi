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

## 4. Dashboard Access Control Is Opt-In, Not Default-Closed

**[Improved]** `PCHI_DASHBOARD_PASSWORD`, when set, now gates all three
HTTP surfaces — the dashboard page (`/`), its `/ws` telemetry feed, and
`/governance/export` — behind HTTP Basic Auth (username `operator`),
not just the export endpoint on its own as the earlier
`PCHI_GOVERNANCE_KEY` did. It is still **open by default**, deliberately,
because this conductor is meant to be self-hosted on a machine its
operator already controls — unlike DGV's `/decisions/export`, which was
found open on a genuinely multi-tenant, publicly reachable deployment and
is now closed by default (see DGV's `LIMITATIONS.md` §4 for that
finding). If you expose a PCHI conductor beyond localhost, set the
password yourself; the software will not do it for you.

One real gap: a non-browser WebSocket client connecting to `/ws`
directly (not a page that already loaded `/` and cached credentials)
must send its own `Authorization: Basic ...` header on the handshake —
the plain browser `WebSocket` API has no way to attach one itself, so
this only works automatically for the dashboard's own JS, which inherits
credentials the browser cached from loading the authenticated page
first.

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

## 7. [Fixed] A Single Bridge Update's Equilibrium Check Used to Pass or Fail Arbitrarily

**Status: fixed.** `message.validate()` (`pchi-schema/src/validation.rs`)
used to reject *every* message outright if its own self-reported
`pir_invariants.residual` exceeded `1e-12` — including `ControlParameter`,
`Heartbeat`, `MusicalContext` and `ArtistTracking` messages, which report
one independent fact and never carried a genuine multi-value invariant to
begin with. The Resolume bridge computed that residual as a
Prouhet-Thue-Morse-signed sum over its last 1–4 recently-set parameter
values — a number with no real meaning for unrelated values — so a
single layer's opacity change (one value, signs `[1]`) always failed,
and matched multi-value updates passed or failed depending purely on
which arbitrary window a given message happened to land on at send
time, not on whether the change was safe.

**The fix:** `validate_invariants` now only checks residual/coherence-gap
against `SceneUpdate` messages — the one payload shape that can
genuinely carry multiple related object states a sender constructed to
balance, which is what an "equilibrium" claim actually means. Every
other message type is no longer gated on it. This isn't a loosened
check; it's a correctly-scoped one: the conductor already has the real
mechanism for tracking equilibrium across a sequence of independent
updates — it recomputes actual aggregate equilibrium from the real
accumulated scene state after every message
(`SceneState::update_equilibrium_status`), and the shipped default rule
file already acts on that real value (`if equilibrium > 1e-12 then
escalate(...)` in `kraken-tentacle.only-pchi`). The message-level gate
was duplicating that check, badly, over data that was never a valid
invariant claim in the first place.

Verified live: the exact scenario that used to always fail (a single
layer's opacity change) and the exact scenario that used to pass or fail
essentially at random (11 real control-parameter updates across mixed
parameters) — 25/25 applied, 0 dropped, after the fix, run against the
same real conductor and bridge. `SceneUpdate`'s equilibrium check is
still enforced (regression-tested in
`pchi-schema/src/validation.rs::scene_update_still_rejects_a_real_equilibrium_violation`).

**Also fixed, previously:** two independent bugs that meant *no*
message — passing or failing this check — ever reached the conductor
before that fix: `message_type`/`payload_type` were serialized under
their Python field names instead of the wire names (`type`/`payloadType`)
the schema's `#[serde(rename = ...)]` actually requires, and messages
were sent OSC-wrapped (`osc_client.send_message`) rather than as raw
JSON-over-UDP, which is the only format the conductor's receive loop
parses. Heartbeats additionally omitted `pir_invariants` and `payload`
entirely, both required fields with no `#[serde(default)]`.

## 8. Governance-Event Messages From Other Conductors Are Logged, Not Re-Verified

A `MessageType::GovernanceEvent` received from another source is recorded
as informational (`conductor.rs`) but its embedded `receipt_id` is not
automatically checked against the originating conductor's own export —
that would require fetching that conductor's `/governance/export` and
running the same verification locally. A receiving dashboard that wants
that guarantee has to do it itself today.

**Not yet built:** an automatic cross-conductor receipt-verification
fetch.
