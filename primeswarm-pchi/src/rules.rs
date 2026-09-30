//! Rule engine for Only-Lang rules.
//!
//! Previously this held a fixed `Vec<Rule>` built by hand as Rust structs
//! (`load_default_rules()`) and never actually read a `.only` file, despite
//! documentation describing PCHI as loading real Only-Lang rules. It now
//! parses real text via `crate::only_lang`, and every rule that fires
//! produces a signed, hash-chained `GovernanceReceipt`
//! (`crate::governance`) plus a `GovernanceEvent` PCHI message the
//! conductor broadcasts — so "AI agents are safely deployed with
//! mathematical boundaries" is something a caller can independently
//! verify, not just a claim in a README.

use crate::governance::GovernanceLog;
use crate::only_lang::{self, Action, RuleFile};
use crate::state::SceneState;
use crate::ConductorError;
use pchi_schema::{GovernanceEventData, MessageType, PCHIMessage, Payload};

/// Bundled default rules — the real, parseable equivalent of the scene
/// this crate has always shipped as its example. Embedded at compile time
/// so a fresh checkout has working default governance with no extra file
/// to point at.
const DEFAULT_RULES_SRC: &str =
    include_str!("../../pchi-schema/examples/kraken-tentacle.only-pchi");

pub struct RuleEngine {
    rule_file: RuleFile,
}

impl RuleEngine {
    /// Load the bundled default rule set.
    pub fn new() -> Self {
        Self::from_source(DEFAULT_RULES_SRC).expect("bundled default rule file must parse")
    }

    /// Load rules from a `.only-pchi` file on disk.
    pub fn load_from_file(path: &str) -> anyhow::Result<Self> {
        let src = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("reading rule file {}: {}", path, e))?;
        Self::from_source(&src).map_err(|e| anyhow::anyhow!("parsing rule file {}: {}", path, e))
    }

    fn from_source(src: &str) -> Result<Self, only_lang::ParseError> {
        Ok(Self {
            rule_file: only_lang::parse(src)?,
        })
    }

    pub fn harmony_threshold(&self) -> f64 {
        self.rule_file.harmony_threshold
    }

    /// Evaluate every rule against the current state. For each rule whose
    /// condition is true, records a signed receipt in `gov` and builds a
    /// `GovernanceEvent` PCHI message for the conductor to broadcast.
    ///
    /// Returns `(events, deny_reason)` rather than using `Err` for a deny.
    /// An earlier version returned `Err(ConductorError::RuleError(msg))`
    /// directly from here, which discarded the already-built `events` —
    /// every governance event from an evaluation that included a DENY was
    /// silently never broadcast, only ever signed into the log. Caught by
    /// `conductor::tests::governance_events_actually_reach_a_connected_ws_client`,
    /// which connects a real WebSocket client and asserts it actually
    /// receives the event — the DENY was correctly in the chain the whole
    /// time (verifiable via `/governance/export`), just never live.
    /// `process_message` broadcasts every event first, then turns
    /// `deny_reason` into the `Err` it returns to its own caller — so a
    /// denial is still never silently treated as success, and is now also
    /// never silently dropped from telemetry.
    pub fn evaluate(
        &self,
        state: &SceneState,
        gov: &GovernanceLog,
    ) -> Result<(Vec<PCHIMessage>, Option<String>), ConductorError> {
        let state_snapshot_hash = snapshot_hash(state);
        let mut events = Vec::new();
        let mut deny: Option<String> = None;

        for rule in &self.rule_file.rules {
            if !only_lang::eval_condition(&rule.condition, state) {
                continue;
            }
            for action in &rule.actions {
                let (gate_state, message, target) = match action {
                    Action::Escalate(msg) => ("ESCALATE", msg.clone(), None),
                    Action::Deny(msg) => ("DENY", msg.clone(), None),
                    Action::Evolve(target, param) => (
                        "ALLOW",
                        format!("evolve {}.{}", target, param),
                        Some(target.clone()),
                    ),
                    Action::Residual => continue, // parsed, intentionally a no-op
                };

                let receipt = gov.record(&rule.name, gate_state, &message, &state_snapshot_hash);

                match gate_state {
                    "ESCALATE" => tracing::warn!("ESCALATE [{}]: {}", receipt.receipt_id, message),
                    "DENY" => tracing::error!("DENY [{}]: {}", receipt.receipt_id, message),
                    _ => tracing::info!("ALLOW [{}]: {}", receipt.receipt_id, message),
                }

                events.push(PCHIMessage::new(
                    MessageType::GovernanceEvent,
                    "pchi-rule-engine".to_string(),
                    Payload::GovernanceEvent(GovernanceEventData {
                        agent_id: "pchi-rule-engine".to_string(),
                        tool: target.unwrap_or_else(|| "scene_state".to_string()),
                        action: rule.name.clone(),
                        gate_state: gate_state.to_string(),
                        receipt_id: receipt.receipt_id.clone(),
                    }),
                ));

                if gate_state == "DENY" && deny.is_none() {
                    deny = Some(message);
                }
            }
        }

        Ok((events, deny))
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// sha256 of a JCS-canonical serialization of the parts of scene state
/// rules actually read, so the receipt can be tied to what triggered it
/// without re-serializing (and hashing) the full object map on every
/// evaluation.
fn snapshot_hash(state: &SceneState) -> String {
    use sha2::{Digest, Sha256};
    let canonical_input = serde_json::json!({
        "control_parameters": state.control_parameters,
        "equilibrium_status": state.equilibrium_status,
        "musical_context": state.musical_context,
        "artist_tracking": state.artist_tracking,
    });
    let canonical =
        serde_jcs::to_string(&canonical_input).unwrap_or_else(|_| canonical_input.to_string());
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::governance::verify_receipt;

    fn temp_gov() -> (GovernanceLog, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("pchi-rules-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let gov = GovernanceLog::open(
            dir.join("key.hex").to_str().unwrap(),
            dir.join("log.jsonl").to_str().unwrap(),
        )
        .unwrap();
        (gov, dir)
    }

    #[test]
    fn default_rules_load_and_parse() {
        let engine = RuleEngine::new();
        assert!(!engine.rule_file.rules.is_empty());
        assert_eq!(engine.harmony_threshold(), 1e-12);
    }

    #[test]
    fn firing_rule_denies_and_signs_a_receipt() {
        let engine = RuleEngine::new();
        let (gov, dir) = temp_gov();
        let mut state = SceneState::new();
        state.set_control_parameter("tentacle_system.total_curl".to_string(), 10.0);

        let (events, deny_reason) = engine.evaluate(&state, &gov).unwrap();
        assert!(deny_reason.is_some(), "curl over threshold should deny");
        assert!(
            events.iter().any(|e| matches!(&e.payload,
                pchi_schema::Payload::GovernanceEvent(d) if d.gate_state == "DENY")),
            "the DENY must still be in the returned events, not just the deny_reason"
        );

        let export = gov.export();
        let receipts = export["receipts"].as_array().unwrap();
        assert!(!receipts.is_empty());
        let vk = export["verifying_key"].as_str().unwrap();
        let r: crate::governance::GovernanceReceipt =
            serde_json::from_value(receipts[0].clone()).unwrap();
        assert!(verify_receipt(&r, vk));
        // The default rule set's first fired rule may be the equilibrium
        // check (ESCALATE, if equilibrium_status happens to be
        // Disequilibrium) or the curl-threshold rule (DENY) — either is
        // fine; what matters is that a real, verifiable receipt exists.
        assert!(r.gate_state == "ESCALATE" || r.gate_state == "DENY");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn allow_actions_do_not_short_circuit() {
        let engine = RuleEngine::new();
        let (gov, dir) = temp_gov();
        let mut state = SceneState::new();
        state.update_musical_context(crate::state::MusicalContext {
            bpm: 128.0,
            beat: 1,
            kick: true,
            snare: false,
            section: "drop".to_string(),
        });
        // Should not error: no curl/animation_speed/coherence_gap set, so
        // only the equilibrium check (Unknown -> false, doesn't fire) and
        // the bpm/kick evolve rule fire.
        let (events, deny_reason) = engine.evaluate(&state, &gov).unwrap();
        assert!(deny_reason.is_none());
        assert!(events.iter().any(|e| matches!(&e.payload,
            pchi_schema::Payload::GovernanceEvent(d) if d.gate_state == "ALLOW")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
