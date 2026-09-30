#!/usr/bin/env python3
"""Offline verifier for a PCHI governance-receipt chain export.

Adapted from DGV's `verify_decision_chain.py`
(only-dgv-verifier/scripts/verify_decision_chain.py) — same three checks,
same reasoning, applied to `GET /governance/export` instead of DGV's
`GET /decisions/export`. Needs no network access and no dependency on the
conductor still running: only the exported JSON and the embedded
verifying_key are used.

For each receipt, checks:

  1. Hash chain contiguous  — this record's parent_decision_hash equals the
     previous record's decision_hash (the very first record's parent must
     be null).
  2. Payload hash correct   — decision_hash is a correct RFC 8785 (JCS)
     canonical re-derivation of {gate_state, message, rule_name,
     state_snapshot_hash}, matching primeswarm-pchi's own
     compute_decision_hash (src/governance.rs).
  3. Signature valid        — the Ed25519 signature verifies against the
     conductor's verifying_key over decision_hash's raw bytes.

Usage:
    pip install pynacl
    curl -s http://127.0.0.1:3000/governance/export > export.json
    python scripts/verify_governance_chain.py export.json

Exit code 0 if every receipt passes all three checks; 1 otherwise, with the
first failing receipt and which check failed printed to stderr.
"""
import argparse
import hashlib
import json
import sys

try:
    from nacl.signing import VerifyKey
    from nacl.exceptions import BadSignatureError
except ImportError:
    print("Missing dependency: pip install pynacl", file=sys.stderr)
    sys.exit(2)


def jcs_canonicalize(value):
    """Minimal RFC 8785 (JCS) canonical JSON: sorted object keys, no
    insignificant whitespace. Matches serde_jcs's behavior for the JSON
    shapes primeswarm-pchi signs."""
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False)


def compute_decision_hash(rule_name, gate_state, message, state_snapshot_hash):
    canonical_input = {
        "gate_state": gate_state,
        "message": message,
        "rule_name": rule_name,
        "state_snapshot_hash": state_snapshot_hash,
    }
    canonical = jcs_canonicalize(canonical_input)
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def verify_chain(export):
    receipts = export["receipts"]
    verify_key = VerifyKey(bytes.fromhex(export["verifying_key"]))
    errors = []
    prev_hash = None

    for i, r in enumerate(receipts):
        rederived = compute_decision_hash(
            r["rule_name"], r["gate_state"], r["message"], r["state_snapshot_hash"]
        )
        payload_ok = rederived == r["decision_hash"]

        try:
            # primeswarm-pchi signs the *hex string* of decision_hash as
            # UTF-8 text (Rust's `sk.sign(decision_hash.as_bytes())` on a
            # String), not the 32 raw bytes the hex decodes to.
            verify_key.verify(
                r["decision_hash"].encode("utf-8"),
                bytes.fromhex(r["signature"]),
            )
            sig_ok = True
        except BadSignatureError:
            sig_ok = False

        chain_ok = r.get("parent_decision_hash") == prev_hash

        status = "OK" if (payload_ok and sig_ok and chain_ok) else "FAIL"
        print(
            f"  [{i}] {r['receipt_id']}  chain={'OK' if chain_ok else 'BROKEN'}  "
            f"hash={'OK' if payload_ok else 'MISMATCH'}  sig={'OK' if sig_ok else 'INVALID'}  -> {status}"
        )
        if status == "FAIL":
            errors.append((i, r["receipt_id"], chain_ok, payload_ok, sig_ok))

        prev_hash = r["decision_hash"]

    return errors


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("export_file", help="JSON file from GET /governance/export")
    args = ap.parse_args()

    with open(args.export_file) as f:
        export = json.load(f)

    print(f"Loaded {len(export['receipts'])} receipt(s) from {args.export_file}\n")
    errors = verify_chain(export)

    print()
    if errors:
        print(f"❌ {len(errors)} record(s) failed verification:", file=sys.stderr)
        for i, receipt_id, chain_ok, payload_ok, sig_ok in errors:
            reasons = []
            if not chain_ok:
                reasons.append("chain link broken")
            if not payload_ok:
                reasons.append("decision_hash does not match re-derived canonical hash")
            if not sig_ok:
                reasons.append("Ed25519 signature invalid")
            print(f"  [{i}] {receipt_id}: {', '.join(reasons)}", file=sys.stderr)
        sys.exit(1)
    else:
        print("✅ All receipts verified — chain contiguous, hashes correct, signatures valid.")
        sys.exit(0)


if __name__ == "__main__":
    main()
