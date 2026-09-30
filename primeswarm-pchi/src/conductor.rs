//! PCHI Conductor - Central hub for scene state management

use crate::governance::GovernanceLog;
use crate::{ConductorError, RuleEngine, SceneState, TransportLayer};
use pchi_schema::{MessageType, PCHIMessage, Payload};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// PCHI Conductor - Central state management hub
pub struct PCHIConductor {
    /// Current scene state
    state: Arc<RwLock<SceneState>>,

    /// Rule engine for Only-Lang rules
    rule_engine: RuleEngine,

    /// Signed, hash-chained log of governance receipts (every rule firing)
    governance: Arc<GovernanceLog>,

    /// Transport layer for sending/receiving messages
    transport: TransportLayer,

    /// Equilibrium threshold
    equilibrium_threshold: f64,
}

impl PCHIConductor {
    /// Create a new PCHI Conductor with the bundled default rule set and
    /// governance log/signing key at their default paths
    /// (`pchi_governance_log.jsonl`, `pchi_governance_key.hex` in the
    /// working directory).
    pub fn new(equilibrium_threshold: f64) -> Self {
        Self::with_governance_paths(
            equilibrium_threshold,
            "pchi_governance_key.hex",
            "pchi_governance_log.jsonl",
        )
        .expect("opening the default governance log/signing key")
    }

    /// Create a conductor with the bundled default rules but explicit
    /// governance file paths — used by tests and by anyone who wants the
    /// signed log somewhere other than the working directory.
    pub fn with_governance_paths(
        equilibrium_threshold: f64,
        signing_key_path: &str,
        governance_log_path: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            state: Arc::new(RwLock::new(SceneState::new())),
            rule_engine: RuleEngine::new(),
            governance: Arc::new(GovernanceLog::open(signing_key_path, governance_log_path)?),
            transport: TransportLayer::new(),
            equilibrium_threshold,
        })
    }

    /// Create a conductor loading rules from a `.only-pchi` file rather
    /// than the bundled default set.
    pub fn with_rules_file(
        equilibrium_threshold: f64,
        rules_path: &str,
        signing_key_path: &str,
        governance_log_path: &str,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            state: Arc::new(RwLock::new(SceneState::new())),
            rule_engine: RuleEngine::load_from_file(rules_path)?,
            governance: Arc::new(GovernanceLog::open(signing_key_path, governance_log_path)?),
            transport: TransportLayer::new(),
            equilibrium_threshold,
        })
    }

    /// Process an incoming PCHI message
    pub async fn process_message(&self, message: PCHIMessage) -> Result<(), ConductorError> {
        // Validate the message
        message.validate()?;

        // Check equilibrium
        if !message.is_equilibrium() {
            warn!("Message not in equilibrium: residual = {}", message.pir_invariants.residual);
        }

        // Update state based on message type
        let mut state = self.state.write().await;

        match message.message_type {
            MessageType::SceneUpdate => {
                if let Payload::SceneUpdate(data) = message.payload {
                    let obj_count = data.objects.len();
                    for obj in &data.objects {
                        state.update_object(obj.id.clone(), obj.transform.position);
                    }
                    info!("Updated scene with {} objects", obj_count);
                }
            }
            MessageType::ControlParameter => {
                if let Payload::ControlParameter(data) = message.payload {
                    let key = format!("{}.{}", data.target_id, data.parameter);
                    if let Some(value) = data.value.as_f64() {
                        state.set_control_parameter(key.clone(), value);
                        info!("Set control parameter: {} = {}", key, value);
                    }
                }
            }
            MessageType::MusicalContext => {
                if let Payload::MusicalContext(data) = message.payload {
                    let context = crate::state::MusicalContext {
                        bpm: data.bpm,
                        beat: data.beat,
                        kick: data.kick,
                        snare: data.snare,
                        section: data.section,
                    };
                    state.update_musical_context(context);
                    info!("Updated musical context: BPM = {}", data.bpm);
                }
            }
            MessageType::ArtistTracking => {
                if let Payload::ArtistTracking(data) = message.payload {
                    let artist_id = data.artist_id.clone();
                    let tracking = crate::state::ArtistTracking {
                        artist_id: data.artist_id,
                        position: data.transform.position,
                        velocity: data.velocity,
                    };
                    state.update_artist_tracking(tracking);
                    info!("Updated artist tracking: {}", artist_id);
                }
            }
            MessageType::Heartbeat => {
                info!("Received heartbeat from {}", message.source_id);
            }
            MessageType::GovernanceEvent => {
                // A governance decision made *elsewhere* (another
                // conductor, a DGV-fronted tool) reported onto this stage.
                // The conductor doesn't re-derive or re-sign it — that
                // would let a forwarder claim authorship of someone else's
                // decision — it just makes the event visible; a receiving
                // dashboard/log can independently verify the embedded
                // receipt_id against the origin's own governance export.
                if let Payload::GovernanceEvent(data) = &message.payload {
                    info!(
                        "GovernanceEvent from {}: {} on {} -> {} (receipt {})",
                        message.source_id, data.action, data.tool, data.gate_state, data.receipt_id
                    );
                }
            }
        }

        // Update equilibrium status
        state.update_equilibrium_status(self.equilibrium_threshold);

        // Apply rules. A prior version of this method swallowed rule
        // errors (`if let Err(e) = ...; error!(...); Ok(())` — always
        // returned Ok regardless of a DENY), so a caller had no way to
        // learn a denial actually happened. Every fired rule is signed
        // into the governance log before this returns, denial or not.
        let events = self.rule_engine.evaluate(&state, &self.governance)?;
        drop(state);

        for event in events {
            if let Ok(bytes) = serde_json::to_vec(&event) {
                if let Err(e) = self.transport.broadcast_websocket(&bytes).await {
                    error!("Failed to broadcast governance event: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Get current scene state
    pub async fn get_state(&self) -> SceneState {
        self.state.read().await.clone()
    }

    /// Get Arc reference to scene state (for web server)
    pub fn get_state_arc(&self) -> Arc<RwLock<SceneState>> {
        Arc::clone(&self.state)
    }

    /// Get Arc reference to the governance log (for web server export route)
    pub fn get_governance_arc(&self) -> Arc<GovernanceLog> {
        Arc::clone(&self.governance)
    }

    /// Get equilibrium status
    pub async fn get_equilibrium_status(&self) -> crate::state::EquilibriumStatus {
        self.state.read().await.equilibrium_status.clone()
    }

    /// Start the transport layer
    pub async fn start_transport(&mut self, bind_addr: &str) -> Result<(), ConductorError> {
        self.transport.start(bind_addr).await
    }

    /// Receive PCHI messages over UDP and feed them into `process_message`
    /// in a loop until the socket errors or is stopped. `start_transport`
    /// only opens the sockets — until this ran, nothing anything sent to
    /// `bind_addr` (the Resolume/TouchDesigner/Ableton bridges' actual
    /// send target) was ever validated, governed, or applied. Requires
    /// `Arc<Self>` so it can be `tokio::spawn`ed alongside the web server.
    ///
    /// Wire format: JSON (`serde_json`), matching what `web_server.rs`'s
    /// WebSocket path already uses — `pchi.fbs` describes a FlatBuffers
    /// schema, but nothing in this crate actually serializes to it at
    /// runtime, so claiming binary framing here would be another
    /// documented-but-not-built gap.
    pub async fn run_udp_receive_loop(self: Arc<Self>) {
        let mut buf = vec![0u8; 65536];
        loop {
            let (len, addr) = match self.transport.receive(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    error!("UDP receive loop stopping: {}", e);
                    return;
                }
            };
            match serde_json::from_slice::<PCHIMessage>(&buf[..len]) {
                Ok(message) => {
                    if let Err(e) = self.process_message(message).await {
                        warn!("process_message error from {}: {}", addr, e);
                    }
                }
                Err(e) => {
                    warn!("Dropping unparseable UDP packet from {}: {}", addr, e);
                }
            }
        }
    }

    /// Stop the transport layer
    pub async fn stop_transport(&mut self) -> Result<(), ConductorError> {
        self.transport.stop().await
    }
}

impl Default for PCHIConductor {
    fn default() -> Self {
        Self::new(1e-12)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pchi_schema::ControlParameterData;

    async fn test_conductor() -> (PCHIConductor, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("pchi-conductor-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let c = PCHIConductor::with_governance_paths(
            1e-12,
            dir.join("key.hex").to_str().unwrap(),
            dir.join("log.jsonl").to_str().unwrap(),
        )
        .unwrap();
        (c, dir)
    }

    #[tokio::test]
    async fn denying_rule_surfaces_as_an_error_not_swallowed() {
        let (conductor, dir) = test_conductor().await;
        let msg = PCHIMessage::new(
            MessageType::ControlParameter,
            "test".to_string(),
            Payload::ControlParameter(ControlParameterData {
                target_id: "tentacle_system".to_string(),
                parameter: "total_curl".to_string(),
                value: serde_json::json!(10.0),
            }),
        );
        let result = conductor.process_message(msg).await;
        assert!(result.is_err(), "a DENY-worthy state must not be swallowed as Ok");

        let export = conductor.get_governance_arc().export();
        assert!(export["count"].as_u64().unwrap() >= 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn governance_event_message_type_does_not_error() {
        let (conductor, dir) = test_conductor().await;
        let msg = PCHIMessage::new(
            MessageType::GovernanceEvent,
            "other-conductor".to_string(),
            Payload::GovernanceEvent(pchi_schema::GovernanceEventData {
                agent_id: "a1".to_string(),
                tool: "strobe".to_string(),
                action: "strobe_intensity_limit".to_string(),
                gate_state: "DENY".to_string(),
                receipt_id: "rcpt_deadbeef".to_string(),
            }),
        );
        assert!(conductor.process_message(msg).await.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
