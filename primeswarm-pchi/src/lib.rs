//! PCHI Conductor - Central state management with PIR mathematical governance
//!
//! The PCHI Conductor is the central hub that receives PCHI messages from various sources,
//! validates them using PIR invariants, applies Only-Lang rules, and broadcasts commands
//! to receivers.

pub mod conductor;
pub mod state;
pub mod rules;
pub mod transport;
pub mod web_server;

pub use conductor::PCHIConductor;
pub use state::SceneState;
pub use rules::RuleEngine;
pub use transport::TransportLayer;
pub use web_server::{create_router, WebServerState};

use thiserror::Error;

/// PCHI Conductor errors
#[derive(Debug, Error)]
pub enum ConductorError {
    #[error("Validation error: {0}")]
    ValidationError(#[from] pchi_schema::validation::ValidationError),
    
    #[error("State error: {0}")]
    StateError(String),
    
    #[error("Transport error: {0}")]
    TransportError(String),
    
    #[error("Rule engine error: {0}")]
    RuleError(String),
}
