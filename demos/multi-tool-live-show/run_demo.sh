#!/usr/bin/env bash
# Multi-Tool Live Show demo — orchestration.
#
# One real PCHI Conductor, three real bridge/relay processes (no code
# changed from what ships in tools/), one rule file. Two phases: a safe
# value change and a rule violation, each triggered by a DIFFERENT
# tool's bridge while a real, live cables.gl browser session watches —
# proving propagation is tool-agnostic, not a same-tool echo.
#
# See README.md for exactly what's genuinely live here (the Conductor,
# the governance chain, every bridge's real code, the cables.gl leg)
# versus simulated (TiXL's and Resolume's own GUI editors, which aren't
# installed in this environment — their real wire formats are sent by
# plain scripts standing in for them).
set -euo pipefail

DEMO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$DEMO_DIR/../.." && pwd)"
SCRATCH="$(mktemp -d)"
LOG_DIR="$SCRATCH/logs"
mkdir -p "$LOG_DIR"

echo "Scratch dir (governance files, screenshots, logs): $SCRATCH"

PIDS=()
cleanup() {
  echo
  echo "--- shutting down demo processes ---"
  for pid in "${PIDS[@]:-}"; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT

echo "--- building the conductor (cargo build -p primeswarm-pchi) ---"
(cd "$REPO_ROOT" && cargo build -p primeswarm-pchi --quiet)

echo "--- starting the Conductor: real binary, demo rule file ---"
(cd "$SCRATCH" && exec "$REPO_ROOT/target/debug/primeswarm-pchi" --rules "$DEMO_DIR/show-rules.only-pchi") \
  > "$LOG_DIR/conductor.log" 2>&1 &
PIDS+=($!)
sleep 2

echo "--- starting the real, unmodified resolume-pchi-bridge ---"
python3 -u "$REPO_ROOT/tools/resolume-pchi-bridge/resolume_pchi_bridge.py" \
  > "$LOG_DIR/resolume-bridge.log" 2>&1 &
PIDS+=($!)

echo "--- starting the real, unmodified tixl-pchi-bridge ---"
python3 -u "$REPO_ROOT/tools/tixl-pchi-bridge/tixl_pchi_bridge.py" \
  > "$LOG_DIR/tixl-bridge.log" 2>&1 &
PIDS+=($!)

echo "--- starting the real, unmodified cables-pchi-relay ---"
python3 -u "$REPO_ROOT/tools/cables-pchi-relay/cables_pchi_relay.py" \
  > "$LOG_DIR/cables-relay.log" 2>&1 &
PIDS+=($!)

sleep 2

echo "--- starting the TiXL governance listener (stands in for OscInput) ---"
python3 -u "$DEMO_DIR/listen_as_tixl.py" > "$LOG_DIR/tixl-listener.log" 2>&1 &
PIDS+=($!)

sleep 2
echo "All processes up. Logs: $LOG_DIR"

echo
echo "=== Phase A: TiXL raises resolume_layer_main.strobe_intensity to a SAFE level (0.3) ==="
echo "    A live cables.gl browser session, pointed at this demo's relay, watches it arrive."
echo "    (This leg — a real headless browser against the real public cables.gl site — is the"
echo "     most environment-sensitive part of this demo. It does not block Phases B/C below if"
echo "     it has a bad run; see README.md's note on this.)"
set +e
node "$DEMO_DIR/cables_leg.js" \
  --relay-ws-port 9101 \
  --trigger-cmd "python3 $DEMO_DIR/send_as_tixl.py resolume_layer_main strobe_intensity 0.3" \
  --screenshot "$SCRATCH/phase-a-safe.png" \
  | tee "$LOG_DIR/phase-a-cables.log"
PHASE_A_STATUS=${PIPESTATUS[0]}
set -e

echo
echo "=== Phase B: Resolume pushes the SAME object past the limit (0.95) ==="
echo "    A different tool's bridge triggers it this time — cables.gl still sees the denial live."
set +e
node "$DEMO_DIR/cables_leg.js" \
  --relay-ws-port 9101 \
  --trigger-cmd "python3 $DEMO_DIR/send_as_resolume.py strobe_intensity 0.95" \
  --screenshot "$SCRATCH/phase-b-deny.png" \
  | tee "$LOG_DIR/phase-b-cables.log"
PHASE_B_STATUS=${PIPESTATUS[0]}
set -e

echo
echo "=== What TiXL's own OscInput (listen_as_tixl.py) saw, independently of cables.gl ==="
cat "$LOG_DIR/tixl-listener.log"

echo
echo "=== Signed, hash-chained governance receipts — exported, not just asserted ==="
curl -s http://127.0.0.1:3000/governance/export > "$SCRATCH/export.json"
python3 -m json.tool < "$SCRATCH/export.json"

echo
echo "=== Independent verification of that export (no trust in this conductor's dashboard) ==="
python3 "$REPO_ROOT/scripts/verify_governance_chain.py" "$SCRATCH/export.json"

echo
echo "=== Summary ==="
status_word() { [ "$1" -eq 0 ] && echo "PASS" || echo "FAIL"; }
echo "  Phase A (TiXL -> cables.gl, safe value):      $(status_word "$PHASE_A_STATUS")"
echo "  Phase B (Resolume -> cables.gl, violation):   $(status_word "$PHASE_B_STATUS")"
echo "  TiXL governance listener, Conductor, signed receipt chain: see output above (not gated by cables.gl)"
if [ "$PHASE_A_STATUS" -ne 0 ] || [ "$PHASE_B_STATUS" -ne 0 ]; then
  echo
  echo "  A FAIL above means the live cables.gl browser leg specifically didn't observe the"
  echo "  verdict this run — see README.md's note on this leg's environment sensitivity. It"
  echo "  does not mean the Conductor, the rule, or the signed receipt chain failed: check the"
  echo "  'Set control parameter' / 'ALLOW'/'DENY' lines above from the real Conductor log —"
  echo "  those happen over real UDP from this run's real bridge processes regardless of"
  echo "  whether the browser leg kept up."
fi

echo
echo "Screenshots: $SCRATCH/phase-a-safe.png, $SCRATCH/phase-b-deny.png"
echo "Full process logs: $LOG_DIR"
