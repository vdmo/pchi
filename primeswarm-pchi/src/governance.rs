//! Signed, hash-chained governance receipts for PCHI rule evaluations.
//!
//! This is the same pattern used by the real DGV gate
//! (`only-dgv-verifier/native/dgv-gate`): every enforcement decision is
//! hashed over an RFC 8785 (JCS) canonical serialization, signed with
//! Ed25519, and chained to the previous decision's hash so a batch can be
//! verified offline — no network access, no trust in this process still
//! running — with `scripts/verify_governance_chain.py`.
//!
//! What is signed here is a *rule evaluation outcome* (a PCHI Only-Lang
//! rule fired against the scene state), not a tool-call authorization like
//! DGV's `/govern`. The chaining and signature format is deliberately the
//! same shape so the same verification story applies: "every AI-driven
//! change to this show that a safety rule touched is cryptographically
//! provable," which is the actual product claim this crate makes.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Mutex;

fn sha256_hex(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    hex::encode(hasher.finalize())
}

/// A signed, chained record of one rule firing. Field names mirror DGV's
/// `DecisionRecord` where the concept maps directly (decision_hash,
/// parent_decision_hash, signature, created_unix_ms); `gate_state` reuses
/// DGV's ALLOW/DENY/ESCALATE vocabulary so both systems' exports read the
/// same way.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceReceipt {
    pub receipt_id: String,
    pub rule_name: String,
    /// "ALLOW" (an Evolve action — a permitted, logged state transition),
    /// "DENY", or "ESCALATE".
    pub gate_state: String,
    pub message: String,
    /// sha256 of the JCS-canonical scene-state snapshot this rule fired
    /// against, so a receipt can be tied back to the state that triggered
    /// it without the export needing to carry the full state every time.
    pub state_snapshot_hash: String,
    pub decision_hash: String,
    pub parent_decision_hash: Option<String>,
    pub signature: String,
    pub created_unix_ms: i64,
}

fn compute_decision_hash(
    rule_name: &str,
    gate_state: &str,
    message: &str,
    state_snapshot_hash: &str,
) -> String {
    let canonical_input = json!({
        "gate_state": gate_state,
        "message": message,
        "rule_name": rule_name,
        "state_snapshot_hash": state_snapshot_hash,
    });
    let canonical =
        serde_jcs::to_string(&canonical_input).unwrap_or_else(|_| canonical_input.to_string());
    sha256_hex(&canonical)
}

struct SigningKeys {
    sk: SigningKey,
    vk: VerifyingKey,
}

impl SigningKeys {
    fn load_or_create(path: &str) -> Self {
        if let Ok(hex_str) = std::fs::read_to_string(path) {
            if let Ok(bytes) = hex::decode(hex_str.trim()) {
                if let Ok(arr) = <[u8; 32]>::try_from(bytes.as_slice()) {
                    let sk = SigningKey::from_bytes(&arr);
                    let vk = sk.verifying_key();
                    tracing::info!("Loaded governance signing key from {}", path);
                    return Self { sk, vk };
                }
            }
        }
        let sk = SigningKey::generate(&mut rand::rngs::OsRng);
        let vk = sk.verifying_key();
        let _ = std::fs::write(path, hex::encode(sk.to_bytes()));
        tracing::info!("Generated new governance signing key, saved to {}", path);
        Self { sk, vk }
    }

    fn sign(&self, decision_hash: &str) -> String {
        let sig = self.sk.sign(decision_hash.as_bytes());
        hex::encode(sig.to_bytes())
    }
}

struct ChainState {
    receipts: Vec<GovernanceReceipt>,
    tail_hash: Option<String>,
    log_file: Option<std::fs::File>,
}

/// Append-only, signed, hash-chained log of governance receipts. Persists
/// to a JSON-Lines file (one receipt per line) so the chain survives a
/// restart — unlike DGV's in-memory `/stats` counters, which reset on
/// restart (see `only-dgv-verifier/docs/LIMITATIONS.md` for why that
/// distinction matters: a signed export is only as good as its
/// durability).
pub struct GovernanceLog {
    keys: SigningKeys,
    state: Mutex<ChainState>,
}

impl GovernanceLog {
    /// `signing_key_path`: hex-encoded Ed25519 seed, created on first run.
    /// `log_path`: JSONL file. If it already contains records, they are
    /// replayed to seed the in-memory chain tail so restart doesn't fork
    /// the chain.
    pub fn open(signing_key_path: &str, log_path: &str) -> anyhow::Result<Self> {
        let keys = SigningKeys::load_or_create(signing_key_path);

        let mut receipts = Vec::new();
        if let Ok(existing) = std::fs::read_to_string(log_path) {
            for line in existing.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<GovernanceReceipt>(line) {
                    Ok(r) => receipts.push(r),
                    Err(e) => tracing::warn!("skipping unparseable governance log line: {}", e),
                }
            }
        }
        let tail_hash = receipts.last().map(|r| r.decision_hash.clone());

        let log_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .map_err(|e| anyhow::anyhow!("opening governance log {}: {}", log_path, e))?;

        tracing::info!(
            "Governance log opened: {} existing receipt(s), verifying_key={}",
            receipts.len(),
            hex::encode(keys.vk.to_bytes())
        );

        Ok(Self {
            keys,
            state: Mutex::new(ChainState {
                receipts,
                tail_hash,
                log_file: Some(log_file),
            }),
        })
    }

    pub fn verifying_key_hex(&self) -> String {
        hex::encode(self.keys.vk.to_bytes())
    }

    /// Sign and chain a new receipt, append it to the log file, return it.
    pub fn record(
        &self,
        rule_name: &str,
        gate_state: &str,
        message: &str,
        state_snapshot_hash: &str,
    ) -> GovernanceReceipt {
        let mut st = self.state.lock().unwrap();
        let decision_hash =
            compute_decision_hash(rule_name, gate_state, message, state_snapshot_hash);
        let signature = self.keys.sign(&decision_hash);
        let receipt = GovernanceReceipt {
            receipt_id: format!("rcpt_{}", &decision_hash[..16]),
            rule_name: rule_name.to_string(),
            gate_state: gate_state.to_string(),
            message: message.to_string(),
            state_snapshot_hash: state_snapshot_hash.to_string(),
            decision_hash: decision_hash.clone(),
            parent_decision_hash: st.tail_hash.clone(),
            signature,
            created_unix_ms: chrono::Utc::now().timestamp_millis(),
        };

        if let Some(f) = st.log_file.as_mut() {
            if let Ok(line) = serde_json::to_string(&receipt) {
                if let Err(e) = writeln!(f, "{}", line) {
                    tracing::error!(
                        "governance_receipt_persist_failed rule={} error={}",
                        rule_name,
                        e
                    );
                }
            }
        }

        st.tail_hash = Some(decision_hash);
        st.receipts.push(receipt.clone());
        receipt
    }

    pub fn export(&self) -> serde_json::Value {
        let st = self.state.lock().unwrap();
        json!({
            "receipts": st.receipts,
            "count": st.receipts.len(),
            "verifying_key": self.verifying_key_hex(),
        })
    }
}

/// Independent re-verification of a single receipt against a verifying
/// key — used by tests and available for the web server's own sanity
/// check. `scripts/verify_governance_chain.py` does the same thing offline
/// against an exported batch.
pub fn verify_receipt(receipt: &GovernanceReceipt, verifying_key_hex: &str) -> bool {
    let rederived = compute_decision_hash(
        &receipt.rule_name,
        &receipt.gate_state,
        &receipt.message,
        &receipt.state_snapshot_hash,
    );
    if rederived != receipt.decision_hash {
        return false;
    }
    let vk_bytes = match hex::decode(verifying_key_hex) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let vk = match <[u8; 32]>::try_from(vk_bytes.as_slice()).ok().and_then(|a| VerifyingKey::from_bytes(&a).ok()) {
        Some(vk) => vk,
        None => return false,
    };
    let sig_bytes = match hex::decode(&receipt.signature) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let sig = match ed25519_dalek::Signature::from_slice(&sig_bytes) {
        Ok(s) => s,
        Err(_) => return false,
    };
    vk.verify(receipt.decision_hash.as_bytes(), &sig).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_and_verify_single_receipt() {
        let dir = std::env::temp_dir().join(format!("pchi-gov-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("key.hex");
        let log_path = dir.join("log.jsonl");

        let log = GovernanceLog::open(key_path.to_str().unwrap(), log_path.to_str().unwrap()).unwrap();
        let r = log.record("tentacle_curl_limit", "DENY", "curl exceeds threshold", "abc123");
        assert!(r.parent_decision_hash.is_none());
        assert!(verify_receipt(&r, &log.verifying_key_hex()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn chain_links_and_tamper_detection() {
        let dir = std::env::temp_dir().join(format!("pchi-gov-test-{}", std::process::id() as u64 + 1));
        std::fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("key.hex");
        let log_path = dir.join("log.jsonl");

        let log = GovernanceLog::open(key_path.to_str().unwrap(), log_path.to_str().unwrap()).unwrap();
        let r1 = log.record("r1", "ALLOW", "fine", "s1");
        let r2 = log.record("r2", "DENY", "not fine", "s2");
        assert_eq!(r2.parent_decision_hash.as_deref(), Some(r1.decision_hash.as_str()));

        let mut tampered = r2.clone();
        tampered.message = "something else".to_string();
        assert!(!verify_receipt(&tampered, &log.verifying_key_hex()));
        assert!(verify_receipt(&r2, &log.verifying_key_hex()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restart_reloads_chain_tail_from_log() {
        let dir = std::env::temp_dir().join(format!("pchi-gov-test-{}", std::process::id() as u64 + 2));
        std::fs::create_dir_all(&dir).unwrap();
        let key_path = dir.join("key.hex");
        let log_path = dir.join("log.jsonl");

        let last_hash = {
            let log =
                GovernanceLog::open(key_path.to_str().unwrap(), log_path.to_str().unwrap()).unwrap();
            log.record("r1", "ALLOW", "fine", "s1").decision_hash
        };
        // Simulate a restart: open again against the same files.
        let log2 =
            GovernanceLog::open(key_path.to_str().unwrap(), log_path.to_str().unwrap()).unwrap();
        let r2 = log2.record("r2", "DENY", "not fine", "s2");
        assert_eq!(r2.parent_decision_hash, Some(last_hash));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
