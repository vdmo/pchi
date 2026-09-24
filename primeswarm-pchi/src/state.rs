//! Scene state management for PCHI Conductor

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Central scene state managed by the PCHI Conductor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneState {
    /// Scene ID
    pub scene_id: Uuid,
    
    /// Current timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    
    /// Object states by ID
    pub objects: HashMap<String, ObjectState>,
    
    /// Musical context
    pub musical_context: Option<MusicalContext>,
    
    /// Artist tracking data
    pub artist_tracking: Option<ArtistTracking>,
    
    /// Control parameters
    pub control_parameters: HashMap<String, f64>,
    
    /// Overall equilibrium status
    pub equilibrium_status: EquilibriumStatus,
}

/// Object state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectState {
    pub id: String,
    pub transform: [f64; 3],  // position x, y, z
    pub custom_data: HashMap<String, serde_json::Value>,
    pub last_update: chrono::DateTime<chrono::Utc>,
}

/// Musical context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicalContext {
    pub bpm: f64,
    pub beat: u32,
    pub kick: bool,
    pub snare: bool,
    pub section: String,
}

/// Artist tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtistTracking {
    pub artist_id: String,
    pub position: [f64; 3],
    pub velocity: Option<[f64; 3]>,
}

/// Equilibrium status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EquilibriumStatus {
    Equilibrium,
    Disequilibrium { residual: f64 },
    Unknown,
}

impl SceneState {
    /// Create a new scene state
    pub fn new() -> Self {
        Self {
            scene_id: Uuid::new_v4(),
            timestamp: chrono::Utc::now(),
            objects: HashMap::new(),
            musical_context: None,
            artist_tracking: None,
            control_parameters: HashMap::new(),
            equilibrium_status: EquilibriumStatus::Unknown,
        }
    }
    
    /// Update object state
    pub fn update_object(&mut self, id: String, transform: [f64; 3]) {
        let now = chrono::Utc::now();
        let state = ObjectState {
            id: id.clone(),
            transform,
            custom_data: HashMap::new(),
            last_update: now,
        };
        self.objects.insert(id, state);
        self.timestamp = now;
    }
    
    /// Update musical context
    pub fn update_musical_context(&mut self, context: MusicalContext) {
        self.musical_context = Some(context);
        self.timestamp = chrono::Utc::now();
    }
    
    /// Update artist tracking
    pub fn update_artist_tracking(&mut self, tracking: ArtistTracking) {
        self.artist_tracking = Some(tracking);
        self.timestamp = chrono::Utc::now();
    }
    
    /// Set control parameter
    pub fn set_control_parameter(&mut self, key: String, value: f64) {
        self.control_parameters.insert(key, value);
        self.timestamp = chrono::Utc::now();
    }
    
    /// Get control parameter
    pub fn get_control_parameter(&self, key: &str) -> Option<f64> {
        self.control_parameters.get(key).copied()
    }
    
    /// Calculate overall equilibrium from current state
    pub fn calculate_equilibrium(&self) -> f64 {
        // Simple equilibrium calculation based on object positions
        if self.objects.is_empty() {
            return 0.0;
        }
        
        let positions: Vec<f64> = self.objects.values()
            .flat_map(|obj| obj.transform.iter())
            .copied()
            .collect();
        
        let mean = positions.iter().sum::<f64>() / positions.len() as f64;
        let variance = positions.iter()
            .map(|v| (v - mean).powi(2))
            .sum::<f64>() / positions.len() as f64;
        
        variance.sqrt()
    }
    
    /// Update equilibrium status
    pub fn update_equilibrium_status(&mut self, threshold: f64) {
        let residual = self.calculate_equilibrium();
        self.equilibrium_status = if residual < threshold {
            EquilibriumStatus::Equilibrium
        } else {
            EquilibriumStatus::Disequilibrium { residual }
        };
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}
