//! Validation for PCHI messages with PIR invariants

use crate::PCHIMessage;
use thiserror::Error;

/// Validation error types
#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("Invalid version: {0}")]
    InvalidVersion(String),
    
    #[error("Missing required field: {0}")]
    MissingField(String),
    
    #[error("Equilibrium violation: residual {residual} exceeds threshold {threshold}")]
    EquilibriumViolation { residual: f64, threshold: f64 },
    
    #[error("Coherence gap {gap} exceeds threshold {threshold}")]
    CoherenceGapViolation { gap: f64, threshold: f64 },
    
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),
    
    #[error("Transform validation failed: {0}")]
    TransformError(String),
    
    #[error("Payload validation failed: {0}")]
    PayloadError(String),
}

/// Validation configuration
#[derive(Debug, Clone)]
pub struct ValidationConfig {
    /// Maximum allowed residual for equilibrium
    pub max_residual: f64,
    
    /// Maximum allowed coherence gap
    pub max_coherence_gap: f64,
    
    /// Whether to enforce strict timestamp validation
    pub strict_timestamp: bool,
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            max_residual: 1e-12,
            max_coherence_gap: 1.0,
            strict_timestamp: false,
        }
    }
}

/// Validate a PCHI message
pub fn validate_message(message: &PCHIMessage) -> Result<(), ValidationError> {
    validate_message_with_config(message, &ValidationConfig::default())
}

/// Validate a PCHI message with custom configuration
pub fn validate_message_with_config(
    message: &PCHIMessage,
    config: &ValidationConfig,
) -> Result<(), ValidationError> {
    // Validate version
    if message.version != "2.0.0" {
        return Err(ValidationError::InvalidVersion(message.version.clone()));
    }
    
    // Validate source_id
    if message.source_id.is_empty() {
        return Err(ValidationError::MissingField("source_id".to_string()));
    }
    
    // Validate timestamp
    if config.strict_timestamp {
        validate_timestamp(message.timestamp)?;
    }
    
    // Validate PIR invariants
    validate_invariants(message, config)?;
    
    // Validate payload
    validate_payload(message)?;
    
    Ok(())
}

fn validate_timestamp(timestamp: f64) -> Result<(), ValidationError> {
    // Timestamp should be reasonable (between year 2000 and 2100)
    const MIN_TIMESTAMP: f64 = 946_684_800_000.0; // 2000-01-01
    const MAX_TIMESTAMP: f64 = 4_102_444_800_000.0; // 2100-01-01
    
    if !(MIN_TIMESTAMP..=MAX_TIMESTAMP).contains(&timestamp) {
        return Err(ValidationError::InvalidTimestamp(
            format!("timestamp {} out of valid range", timestamp)
        ));
    }
    
    Ok(())
}

/// Equilibrium is a property of a *scene* — a set of related values that a
/// sender constructed to satisfy an invariant together (the Kraken example:
/// 8 tentacles' wobbliness values, deliberately paired so their
/// Prouhet-Thue-Morse-signed sum cancels). It is not a property of one
/// independent fact.
///
/// Before this fix, every message type was checked against the same
/// residual/coherence-gap threshold, including ControlParameter,
/// MusicalContext, ArtistTracking and Heartbeat — messages that report a
/// single value or a single unrelated fact and have no multi-value
/// invariant to satisfy in the first place. A real bridge (Resolume)
/// computed its residual as a Prouhet-Thue-Morse-signed sum over its last
/// 1–4 *arbitrary, unrelated* recently-set parameters — a number with no
/// real meaning — and this check rejected the update outright whenever
/// that incidental window didn't happen to cancel. Tested live: a single
/// layer's opacity change (one value) always failed; matched updates
/// sometimes passed depending purely on send-order coincidence. See
/// docs/LIMITATIONS.md item 7 for the full writeup before this fix.
///
/// Only `SceneUpdate` carries a payload that can genuinely represent
/// multiple related object states a sender constructed to balance — so
/// only `SceneUpdate` is checked here. The conductor already has the
/// correct mechanism for tracking real equilibrium across single
/// incremental updates: it recomputes actual aggregate equilibrium from
/// the real accumulated scene state after every message
/// (`SceneState::update_equilibrium_status`), independent of what any one
/// message claims about itself — and the shipped default rule file already
/// acts on that real value (`if equilibrium > 1e-12 then escalate(...)`
/// in `kraken-tentacle.only-pchi`). This function stops duplicating that
/// check, badly, at the transport layer.
fn validate_invariants(
    message: &PCHIMessage,
    config: &ValidationConfig,
) -> Result<(), ValidationError> {
    if !matches!(message.message_type, crate::MessageType::SceneUpdate) {
        return Ok(());
    }

    // Check equilibrium
    if message.pir_invariants.residual.abs() > config.max_residual {
        return Err(ValidationError::EquilibriumViolation {
            residual: message.pir_invariants.residual,
            threshold: config.max_residual,
        });
    }

    // Check coherence gap
    if message.pir_invariants.coherence_gap > config.max_coherence_gap {
        return Err(ValidationError::CoherenceGapViolation {
            gap: message.pir_invariants.coherence_gap,
            threshold: config.max_coherence_gap,
        });
    }

    Ok(())
}

fn validate_payload(message: &PCHIMessage) -> Result<(), ValidationError> {
    use crate::Payload;
    
    match &message.payload {
        Payload::SceneUpdate(data) => {
            if data.objects.is_empty() {
                return Err(ValidationError::PayloadError(
                    "Scene update must contain at least one object".to_string()
                ));
            }
            
            for obj in &data.objects {
                if obj.id.is_empty() {
                    return Err(ValidationError::PayloadError(
                        "Object must have a non-empty ID".to_string()
                    ));
                }
            }
        }
        Payload::ControlParameter(data) => {
            if data.target_id.is_empty() {
                return Err(ValidationError::PayloadError(
                    "Control parameter must have a target_id".to_string()
                ));
            }
            if data.parameter.is_empty() {
                return Err(ValidationError::PayloadError(
                    "Control parameter must have a parameter name".to_string()
                ));
            }
        }
        Payload::MusicalContext(data) => {
            if data.bpm <= 0.0 || data.bpm > 300.0 {
                return Err(ValidationError::PayloadError(
                    format!("Invalid BPM: {}", data.bpm)
                ));
            }
        }
        Payload::ArtistTracking(data) => {
            if data.artist_id.is_empty() {
                return Err(ValidationError::PayloadError(
                    "Artist tracking must have an artist_id".to_string()
                ));
            }
        }
        Payload::GovernanceEvent(data) => {
            if data.agent_id.is_empty() || data.gate_state.is_empty() {
                return Err(ValidationError::PayloadError(
                    "Governance event must have an agent_id and gate_state".to_string()
                ));
            }
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessageType, Payload, SceneUpdateData};
    
    #[test]
    fn test_valid_message() {
        let message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![],
            }),
        );
        
        let result = validate_message(&message);
        // Should fail because objects is empty
        assert!(result.is_err());
    }
    
    #[test]
    fn test_equilibrium_violation() {
        let mut message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![],
            }),
        );
        
        message.pir_invariants.residual = 1.0;
        
        let config = ValidationConfig {
            max_residual: 1e-12,
            ..Default::default()
        };
        
        let result = validate_message_with_config(&message, &config);
        assert!(matches!(result, Err(ValidationError::EquilibriumViolation { .. })));
    }
    
    #[test]
    fn test_invalid_version() {
        let mut message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![],
            }),
        );
        
        message.version = "1.0.0".to_string();

        let result = validate_message(&message);
        assert!(matches!(result, Err(ValidationError::InvalidVersion(_))));
    }

    #[test]
    fn scene_update_still_rejects_a_real_equilibrium_violation() {
        // Regression guard: SceneUpdate is exactly the case this check
        // exists for, and must still be enforced.
        let mut message = PCHIMessage::new(
            MessageType::SceneUpdate,
            "test_source".to_string(),
            Payload::SceneUpdate(SceneUpdateData {
                objects: vec![crate::SceneObject {
                    id: "obj1".to_string(),
                    transform: crate::Transform::default(),
                    custom_data: None,
                    pir_constraints: None,
                }],
            }),
        );
        message.pir_invariants.residual = 1.0;
        let result = validate_message(&message);
        assert!(matches!(result, Err(ValidationError::EquilibriumViolation { .. })));
    }

    #[test]
    fn control_parameter_is_not_gated_on_residual() {
        // This is the actual bug: a single control-parameter change was
        // rejected outright whenever its self-reported residual (computed
        // by a real bridge as a PTM-signed sum over an arbitrary window of
        // unrelated recent values) didn't happen to cancel — see
        // docs/LIMITATIONS.md item 7. A single value can never validly
        // "cancel" by construction, so this made most real updates fail
        // essentially at random. ControlParameter has no multi-value
        // invariant to assert in the first place.
        let mut message = PCHIMessage::new(
            MessageType::ControlParameter,
            "resolume_bridge".to_string(),
            Payload::ControlParameter(crate::ControlParameterData {
                target_id: "resolume_layer_1".to_string(),
                parameter: "opacity".to_string(),
                value: serde_json::json!(0.85),
            }),
        );
        message.pir_invariants.residual = 0.85; // exactly what a single-value PTM sum is
        let result = validate_message(&message);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn heartbeat_is_not_gated_on_residual() {
        let mut message = PCHIMessage::new(
            MessageType::Heartbeat,
            "resolume_bridge".to_string(),
            Payload::ControlParameter(crate::ControlParameterData {
                target_id: "resolume_bridge".to_string(),
                parameter: "heartbeat".to_string(),
                value: serde_json::json!(1.0),
            }),
        );
        message.pir_invariants.residual = 0.5;
        let result = validate_message(&message);
        assert!(result.is_ok(), "expected Ok, got {:?}", result);
    }

    #[test]
    fn musical_context_and_artist_tracking_are_not_gated_on_residual() {
        let mut mc = PCHIMessage::new(
            MessageType::MusicalContext,
            "ableton_bridge".to_string(),
            Payload::MusicalContext(crate::MusicalContextData {
                bpm: 128.0,
                beat: 1,
                kick: true,
                snare: false,
                section: "drop".to_string(),
            }),
        );
        mc.pir_invariants.residual = 3.0;
        assert!(validate_message(&mc).is_ok());

        let mut at = PCHIMessage::new(
            MessageType::ArtistTracking,
            "tracker".to_string(),
            Payload::ArtistTracking(crate::ArtistTrackingData {
                artist_id: "artist1".to_string(),
                transform: crate::Transform::default(),
                velocity: None,
            }),
        );
        at.pir_invariants.residual = 3.0;
        assert!(validate_message(&at).is_ok());
    }
}
