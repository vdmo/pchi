# Demo: Multi-Tool Live Show

One real PCHI Conductor. One rule file. Three tools' real, unmodified
bridge/relay code from `tools/`. This demonstrates the actual claim
behind "many tools, one truth": a control update sent through *any one*
tool's bridge is validated and governed once, and every other connected
tool sees the same verdict live — not three separate integrations that
happen to agree by convention.

## What's genuinely live here, and what's simulated

Be precise about this, because it matters:

- **Genuinely live**: the PCHI Conductor (the real `primeswarm-pchi`
  binary), the Only-Lang rule engine, the signed/hash-chained governance
  log, every bridge's real code (`tixl-pchi-bridge`,
  `resolume-pchi-bridge`, `cables-pchi-relay`, unmodified), and the
  cables.gl leg — a real headless Chromium driving the actual, live,
  public `cables.gl` editor and its real `WebSocket_v2` op.
- **Simulated**: TiXL and Resolume Arena are both GUI applications (TiXL
  is Windows/.NET, Resolume is a commercial Windows/Mac app) not
  installed in this environment. `send_as_tixl.py` and
  `send_as_resolume.py` stand in for their editors — but they send the
  *exact real wire shapes* those tools' own OSC operators send
  (confirmed from TiXL's actual source for one, from the real bugs
  fixed against Resolume's actual format in this repo's PR #3 for the
  other), to the real, unmodified bridges. The bridges themselves
  cannot tell the difference between these scripts and the real editors.
- **Resolume's bridge is one-directional.** Unlike the TiXL bridge and
  the cables.gl relay, `resolume-pchi-bridge` has no governance-receive
  leg — it only sends control updates to the Conductor, it doesn't
  listen for verdicts coming back. That's a real, current limitation of
  that bridge (see its source), not something this demo works around,
  which is why there's a `listen_as_tixl.py` but no
  `listen_as_resolume.py`.

## Why every tool converges on `resolume_layer_main.strobe_intensity`

In a real show, each tool addresses things its own way — this is the
whole reason a shared Conductor matters. For this demo, the OSC address
sent "as Resolume" (`/composition/layers/main/video/strobe_intensity`)
happens to produce that exact key because `resolume-pchi-bridge` always
derives `target_id` as `resolume_layer_{layer_id}` — real, unmodified
behavior, not special-cased for this demo. TiXL's bridge has no fixed
naming convention at all (its `Strings` input is a free-form
`"target_id.parameter"` string), so `send_as_tixl.py` simply uses the
same key on purpose. This mirrors exactly what an operator setting up a
real show has to do: agree on shared names across tools, the same way
you'd agree on OSC addresses today — PCHI doesn't invent that agreement
for you, it gives every tool a single, verified place to enforce rules
once that agreement exists.

## Running it

Requirements: Rust toolchain, Python 3 with `python-osc`, `numpy`, and
`websockets` installed (`pip install python-osc numpy websockets`,
already needed by the individual bridges this reuses), Node.js, and
`/snap/bin/chromium` (or pass `--chromium-path` to `cables_leg.js`).

```bash
npm install          # puppeteer-core, for the cables.gl leg
./run_demo.sh
```

It builds the Conductor, starts all four real processes, waits for
startup, then runs two phases:

1. **Safe value.** TiXL raises `resolume_layer_main.strobe_intensity` to
   `0.3`. The rule's `evolve` action fires (every update gets a visible
   ALLOW acknowledgement — see `show-rules.only-pchi`), and a live
   cables.gl browser session — pointed at this demo's
   `cables-pchi-relay`, not at TiXL — shows it arriving in real time.
2. **Violation.** Resolume — a *different* tool's bridge — pushes the
   same object to `0.95`, past the rule's `0.8` threshold. The Conductor
   denies it, signs a receipt, and broadcasts the denial. The same
   cables.gl session sees the same verdict live, proving propagation
   doesn't depend on which tool caused the update.

You'll see **two** governance events for Phase B, not one: the routine
`evolve` acknowledgement every update gets, immediately followed by the
`DENY` the threshold rule adds. That's the real rule-evaluation loop
running every rule in the file in order, not a bug.

### The cables.gl leg's environment sensitivity

Of everything in this demo, the live cables.gl browser leg is the one
genuinely sensitive to the host machine's load. It's a real headless
Chromium rendering the real public cables.gl editor (WebGL, a live
WebSocket) — under heavy *unrelated* CPU contention on the host, the
browser tab can be starved enough that its WebSocket connection to this
demo's relay drops and doesn't reconnect in time for the verdict to
arrive before `run_demo.sh` checks. This was observed directly while
building this demo: three unrelated background processes consuming
500%+ combined CPU on the same machine caused repeated, reproducible
connection drops that disappeared once that load wasn't present. Both
of this relay's WebSocket legs now use a 60s ping timeout (up from the
`websockets` library's 20s default) specifically because of this, and
`cables_leg.js` checks the relay's real `/status` endpoint and
re-triggers the op's connection before giving up — but on a sufficiently
loaded machine, this leg can still fail where the rest of the demo
doesn't.

That's why `run_demo.sh` does **not** let a Phase A/B cables.gl failure
block the rest of the run: the Conductor, the rule engine, the signed
governance chain, and the TiXL governance listener are verified
independently of it, from the real Conductor log and the exported,
independently-verified receipt chain, every run, regardless of what the
browser leg managed to observe. A "FAIL" in the summary's Phase A/B
lines means specifically that the browser didn't see the verdict in
time this run — not that the update wasn't validated, governed, and
signed, which the Conductor log and the governance export right above
the summary show happened regardless.

At the end, the script exports the full signed governance log and runs
`scripts/verify_governance_chain.py` against it — the same independent
check an auditor would run, over everything this run actually did.

## What this doesn't claim

This isn't a benchmark or a production deployment guide. The 2-second
sleeps between starting each process are generous, not tuned; the demo
rule file is deliberately small. What it does prove, concretely: a real
rule, written once, enforced identically regardless of which of three
different tools' bridges a message arrives through, with every decision
independently verifiable afterward.
