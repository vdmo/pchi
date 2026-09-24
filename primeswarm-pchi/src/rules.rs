//! Rule engine for Only-Lang rules

use crate::{ConductorError, SceneState};

/// Rule engine for evaluating Only-Lang rules
pub struct RuleEngine {
    /// Loaded rules
    rules: Vec<Rule>,
}

/// A single Only-Lang rule
#[derive(Debug, Clone)]
struct Rule {
    /// Rule name
    name: String,
    /// Rule condition (simplified for prototype)
    condition: RuleCondition,
    /// Rule action
    action: RuleAction,
}

/// Rule condition
#[derive(Debug, Clone)]
enum RuleCondition {
    /// Equilibrium check
    Equilibrium { threshold: f64 },
    /// Parameter check
    Parameter { key: String, operator: ComparisonOp, value: f64 },
    /// Musical context check
    MusicalContext { field: MusicalField, operator: ComparisonOp, value: f64 },
    /// Artist position check
    ArtistPosition { axis: Axis, operator: ComparisonOp, value: f64 },
}

/// Comparison operator
#[derive(Debug, Clone)]
enum ComparisonOp {
    LessThan,
    GreaterThan,
    Equal,
    NotEqual,
}

/// Musical context field
#[derive(Debug, Clone)]
enum MusicalField {
    Bpm,
    Beat,
}

/// Axis for position checks
#[derive(Debug, Clone)]
enum Axis {
    X,
    Y,
    Z,
}

/// Rule action
#[derive(Debug, Clone)]
enum RuleAction {
    /// Escalate with message
    Escalate(String),
    /// Deny with message
    Deny(String),
    /// Evolve parameter
    Evolve(String, String),
}

impl RuleEngine {
    /// Create a new rule engine
    pub fn new() -> Self {
        Self {
            rules: Self::load_default_rules(),
        }
    }
    
    /// Load default rules (Kraken tentacle example)
    fn load_default_rules() -> Vec<Rule> {
        vec![
            // Tentacle wobbliness equilibrium rule
            Rule {
                name: "tentacle_equilibrium".to_string(),
                condition: RuleCondition::Equilibrium { threshold: 1e-12 },
                action: RuleAction::Escalate("Tentacle wobbliness violates equilibrium".to_string()),
            },
            // Tentacle curl threshold
            Rule {
                name: "tentacle_curl_limit".to_string(),
                condition: RuleCondition::Parameter {
                    key: "tentacle_system.total_curl".to_string(),
                    operator: ComparisonOp::GreaterThan,
                    value: 4.0,
                },
                action: RuleAction::Deny("Tentacle curl exceeds harmonic threshold".to_string()),
            },
            // Animation speed safety
            Rule {
                name: "animation_speed_safety".to_string(),
                condition: RuleCondition::Parameter {
                    key: "tentacle_system.animation_speed".to_string(),
                    operator: ComparisonOp::GreaterThan,
                    value: 2.0,
                },
                action: RuleAction::Deny("Animation speed exceeds safety limit".to_string()),
            },
            // Coherence gap monitoring
            Rule {
                name: "coherence_gap_monitor".to_string(),
                condition: RuleCondition::Parameter {
                    key: "scene_state.coherence_gap".to_string(),
                    operator: ComparisonOp::GreaterThan,
                    value: 0.5,
                },
                action: RuleAction::Escalate("Scene coherence gap too large".to_string()),
            },
            // Kick drum peak state trigger
            Rule {
                name: "kick_peak_state".to_string(),
                condition: RuleCondition::MusicalContext {
                    field: MusicalField::Bpm,
                    operator: ComparisonOp::GreaterThan,
                    value: 120.0,
                },
                action: RuleAction::Evolve("kraken_main".to_string(), "peak_state".to_string()),
            },
        ]
    }
    
    /// Evaluate all rules against the current state
    pub async fn evaluate(&self, state: &SceneState) -> Result<(), ConductorError> {
        for rule in &self.rules {
            if self.evaluate_condition(&rule.condition, state) {
                self.execute_action(&rule.action)?;
            }
        }
        Ok(())
    }
    
    /// Evaluate a single condition
    fn evaluate_condition(&self, condition: &RuleCondition, state: &SceneState) -> bool {
        match condition {
            RuleCondition::Equilibrium { threshold } => {
                match &state.equilibrium_status {
                    crate::state::EquilibriumStatus::Equilibrium => false,
                    crate::state::EquilibriumStatus::Disequilibrium { residual } => {
                        residual.abs() > *threshold
                    }
                    crate::state::EquilibriumStatus::Unknown => false,
                }
            }
            RuleCondition::Parameter { key, operator, value } => {
                if let Some(param_value) = state.get_control_parameter(key) {
                    self.compare(param_value, *value, operator)
                } else {
                    false
                }
            }
            RuleCondition::MusicalContext { field, operator, value } => {
                if let Some(context) = &state.musical_context {
                    let field_value = match field {
                        MusicalField::Bpm => context.bpm,
                        MusicalField::Beat => context.beat as f64,
                    };
                    self.compare(field_value, *value, operator)
                } else {
                    false
                }
            }
            RuleCondition::ArtistPosition { axis, operator, value } => {
                if let Some(tracking) = &state.artist_tracking {
                    let axis_value = match axis {
                        Axis::X => tracking.position[0],
                        Axis::Y => tracking.position[1],
                        Axis::Z => tracking.position[2],
                    };
                    self.compare(axis_value, *value, operator)
                } else {
                    false
                }
            }
        }
    }
    
    /// Compare two values using an operator
    fn compare(&self, a: f64, b: f64, op: &ComparisonOp) -> bool {
        match op {
            ComparisonOp::LessThan => a < b,
            ComparisonOp::GreaterThan => a > b,
            ComparisonOp::Equal => (a - b).abs() < 1e-9,
            ComparisonOp::NotEqual => (a - b).abs() >= 1e-9,
        }
    }
    
    /// Execute a rule action
    fn execute_action(&self, action: &RuleAction) -> Result<(), ConductorError> {
        match action {
            RuleAction::Escalate(msg) => {
                tracing::warn!("ESCALATE: {}", msg);
                Ok(())
            }
            RuleAction::Deny(msg) => {
                tracing::error!("DENY: {}", msg);
                Err(ConductorError::RuleError(msg.clone()))
            }
            RuleAction::Evolve(target, param) => {
                tracing::info!("EVOLVE: {}.{}", target, param);
                Ok(())
            }
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}
