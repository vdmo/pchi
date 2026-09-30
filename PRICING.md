# PCHI: what's free, what's paid, and why

PCHI has never had a business model — no pricing, no hosted service, no
paid tier. This is the first one, written the same day the governance
claim it depends on (see below) went from documentation to real, tested
code.

## The rule, stated once

**Self-hosting this software is free forever, without restriction.**
Schema, conductor, rule engine, signed governance log, bridges — all of it,
Apache-2.0/MIT (see `LICENSE`), no key required, no phone-home, no feature
gated behind payment for someone running their own copy on their own
machine. `primeswarm-pchi`'s `/governance/export` route supports an
optional `PCHI_GOVERNANCE_KEY` — that's a *security* control for anyone
exposing a conductor beyond localhost, not a paywall; it defaults open.

What's paid is work someone has to actually do on your behalf, not
software we could give you for free and chose not to:

## 1. Cloud Conductor (not yet built)

A hosted, multi-venue PCHI Conductor — you point bridges at it instead of
running your own. Real infrastructure cost (uptime, multi-tenant
isolation, backups of everyone's signing keys and governance logs), so a
real thing to charge for. Status: on the roadmap in
`pchi-schema/CAPABILITIES_AND_APPLICATIONS.md`, unbuilt. Do not sell this
until it exists.

## 2. Certified rule packs (not yet built)

Every venue runs the same basic categories of physical risk: strobe
intensity and flash-rate limits for photosensitive epilepsy, pyrotechnics
clearance and cooldown windows, moving-rig proximity to performers. A
`.only-pchi` file encoding one of those correctly, reviewed against the
relevant standard (e.g. a strobe pack checked against IEC 61966 guidance,
a pyro pack checked against NFPA 1126), is real domain expertise with real
liability attached to getting it wrong — legitimately a paid product, sold
as a file plus a signed attestation that a named reviewer checked it. The
`.only-pchi` format itself (this repo) stays free; a *specific, vetted*
rule pack is what's paid for.

## 3. Compliance reporting and verification-as-a-service (not yet built)

`/governance/export` gives you raw signed JSON. An insurer or regulator
doesn't want raw JSON — they want a readable report ("this show ran with
N rule evaluations, 0 unresolved DENYs, chain and signatures independently
verified") plus, if they don't want to trust your own verification, an
independent party re-running `verify_governance_chain.py` and attesting to
the result. Both are services on top of data the free tier already
produces, not access to the data itself.

## What this is not

Not an "open core with artificial limits" model — nothing in the free
tier is deliberately crippled to push you toward point 1–3. A hobbyist
running one conductor for one show gets the exact same enforcement,
signing, and export as anyone else. Point 1–3 exist because they're real
work (hosting, expert review, an independent attestation), not because
we made the free version worse.

## Honesty check

None of 1–3 exist yet — no signup, no checkout, no service. This document
is a real commitment about what will and won't be paywalled once they do,
written so it can be checked later against what actually ships, the same
way `docs/LIMITATIONS.md` documents what the governance system does and
doesn't prove today.
