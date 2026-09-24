//! PCHI v2.0 Schema with PIR Mathematical Invariants
//!
//! This crate provides the PCHI (Peachy) v2.0 protocol schema for scene state communication
//! with embedded PIR (Prime Integer Relations) mathematical invariants for equilibrium and coherence.

pub mod invariants;
pub mod validation;
pub mod transform;

pub use invariants::{PIRInvariants, EquilibriumCheck};
pub use transform::Transform;

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use thiserror::Error;
use std::collections::HashMap;

/// PCHI v2.0 message with PIR invariants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PCHIMessage {
    /// Schema version
    pub version: String,
    
    /// Message type
    #[serde(rename = "type")]
    pub message_type: MessageType,
    
    /// Source identifier
    pub source_id: String,
    
    /// Timestamp in milliseconds
    pub timestamp: f64,
    
    /// PIR mathematical invariants
    pub pir_invariants: invariants::PIRInvariants,
    
    /// Message payload
    pub payload: Payload,
}

/// Message types in the PCHI protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageType {
    /// Scene update with object transforms
    SceneUpdate,
    
    /// Control parameter change
    ControlParameter,
    
    /// Musical context (BPM, beat, etc.)
    MusicalContext,
    
    /// Artist tracking data
    ArtistTracking,
    
    /// System heartbeat
    Heartbeat,

    /// A real DGV governance decision, recorded onto the stage.
    GovernanceEvent,
}

/// Message payload
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "payloadType", content = "data")]
pub enum Payload {
    /// Scene update with objects
    #[serde(rename = "scene_update")]
    SceneUpdate(SceneUpdateData),
    
    /// Control parameter
    #[serde(rename = "control_parameter")]
    ControlParameter(ControlParameterData),
    
    /// Musical context
    #[serde(rename = "musical_context")]
    MusicalContext(MusicalContextData),
    
    /// Artist tracking
    #[serde(rename = "artist_tracking")]
    ArtistTracking(ArtistTrackingData),

    /// DGV governance decision
    #[serde(rename = "governance_event")]
    GovernanceEvent(GovernanceEventData),
}

/// Scene update data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneUpdateData {
    pub objects: Vec<SceneObject>,
}

/// Scene object with transform and custom data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub id: String,
    pub transform: transform::Transform,
    pub custom_data: Option<HashMap<String, serde_json::Value>>,
    pub pir_constraints: Option<ObjectConstraints>,
}

/// Object-level PIR constraints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectConstraints {
    pub harmony_rule: String,
    pub allowed_range: (f64, f64),
}

/// Control parameter data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlParameterData {
    pub target_id: String,
    pub parameter: String,
    pub value: serde_json::Value,
}

/// Musical context data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicalContextData {
    pub bpm: f64,
    pub beat: u32,
    pub kick: bool,
    pub snare: bool,
    pub section: String,
}

/// Artist tracking data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistTrackingData {
    pub artist_id: String,
    pub transform: transform::Transform,
    pub velocity: Option<[f64; 3]>,
}

/// A real DGV governance decision (ALLOW/DENY/DEFER/SILENCE), carried onto
/// the PCHI stage so any connected tool can react to real enforcement
/// outcomes rather than only scene/performance state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceEventData {
    pub agent_id: String,
    pub tool: String,
    pub action: String,
    pub gate_state: String,
    pub receipt_id: String,
}

impl PCHIMessage {
    /// Create a new PCHI message
    pub fn new(
        message_type: MessageType,
        source_id: String,
        payload: Payload,
    ) -> Self {
        Self {
            version: "2.0.0".to_string(),
            message_type,
            source_id,
            timestamp: chrono::Utc::now().timestamp_millis() as f64,
            pir_invariants: invariants::PIRInvariants::default(),
            payload,
        }
    }
    
    /// Validate the message using PIR invariants
    pub fn validate(&self) -> Result<(), validation::ValidationError> {
        validation::validate_message(self)
    }
    
    /// Check if the message maintains equilibrium
    pub fn is_equilibrium(&self) -> bool {
        self.pir_invariants.residual < 1e-12
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_pchi_message_creation() {
        let message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![],
            }),
        );
        
        assert_eq!(message.version, "2.0.0");
        assert_eq!(message.source_id, "test_source");
    }
    
    #[test]
    fn test_equilibrium_check() {
        let mut message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![],
            }),
        );
        
        // Default should be in equilibrium
        assert!(message.is_equilibrium());
        
        // Break equilibrium
        message.pir_invariants.residual = 1.0;
        assert!(!message.is_equilibrium());
    }
}
