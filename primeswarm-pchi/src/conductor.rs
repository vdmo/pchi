//! PCHI Conductor - Central hub for scene state management

use crate::{ConductorError, SceneState, RuleEngine, TransportLayer};
use pchi_schema::{PCHIMessage, MessageType, Payload};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error};

/// PCHI Conductor - Central state management hub
pub struct PCHIConductor {
    /// Current scene state
    state: Arc<RwLock<SceneState>>,
    
    /// Rule engine for Only-Lang rules
    rule_engine: RuleEngine,
    
    /// Transport layer for sending/receiving messages
    transport: TransportLayer,
    
    /// Equilibrium threshold
    equilibrium_threshold: f64,
}

impl PCHIConductor {
    /// Create a new PCHI Conductor
    pub fn new(equilibrium_threshold: f64) -> Self {
        Self {
            state: Arc::new(RwLock::new(SceneState::new())),
            rule_engine: RuleEngine::new(),
            transport: TransportLayer::new(),
            equilibrium_threshold,
        }
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
        }
        
        // Update equilibrium status
        state.update_equilibrium_status(self.equilibrium_threshold);
        
        // Apply rules
        let rule_result = self.rule_engine.evaluate(&state).await;
        if let Err(e) = rule_result {
            error!("Rule evaluation error: {}", e);
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
    
    /// Get equilibrium status
    pub async fn get_equilibrium_status(&self) -> crate::state::EquilibriumStatus {
        self.state.read().await.equilibrium_status.clone()
    }
    
    /// Start the transport layer
    pub async fn start_transport(&mut self, bind_addr: &str) -> Result<(), ConductorError> {
        self.transport.start(bind_addr).await
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
