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

    /// Atomically write this state to `path` as JSON: serialize, write to
    /// a sibling temp file, then rename into place. The rename is what
    /// makes it atomic — a crash mid-write leaves the temp file
    /// incomplete but never touches `path` itself, so the file there is
    /// always either the previous complete snapshot or the new one,
    /// never a half-written one.
    pub fn save_snapshot(&self, path: &str) -> std::io::Result<()> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let tmp_path = format!("{path}.tmp");
        std::fs::write(&tmp_path, json)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Load a previously saved snapshot, if one exists and parses.
    /// Returns `None` rather than an error on a missing or corrupt file:
    /// scene state is a resilience convenience, not the safety-critical
    /// record (that's the governance log, which does fail hard on
    /// corruption — see `governance.rs`) — a bad snapshot should log a
    /// warning and start fresh, not keep the conductor from coming up.
    pub fn load_snapshot(path: &str) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        match serde_json::from_slice(&bytes) {
            Ok(state) => Some(state),
            Err(e) => {
                tracing::warn!(
                    "Scene state snapshot at {} is unreadable ({}), starting fresh",
                    path,
                    e
                );
                None
            }
        }
    }
}

impl Default for SceneState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("pchi-state-test-{}-{}", name, uuid::Uuid::new_v4()))
    }

    #[test]
    fn save_then_load_round_trips_real_values() {
        let path = temp_path("roundtrip");
        let mut state = SceneState::new();
        state.set_control_parameter("tentacle_system.total_curl".to_string(), 2.5);
        state.update_musical_context(MusicalContext {
            bpm: 128.0,
            beat: 3,
            kick: true,
            snare: false,
            section: "drop".to_string(),
        });

        state.save_snapshot(path.to_str().unwrap()).unwrap();
        let loaded = SceneState::load_snapshot(path.to_str().unwrap()).unwrap();

        assert_eq!(loaded.get_control_parameter("tentacle_system.total_curl"), Some(2.5));
        assert_eq!(loaded.musical_context.unwrap().beat, 3);
        assert_eq!(loaded.scene_id, state.scene_id, "identity, not just values, must survive a round trip");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn save_leaves_no_temp_file_behind() {
        let path = temp_path("no-temp-litter");
        SceneState::new().save_snapshot(path.to_str().unwrap()).unwrap();
        assert!(path.exists());
        assert!(!std::path::Path::new(&format!("{}.tmp", path.to_str().unwrap())).exists());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn load_missing_file_returns_none_not_an_error() {
        let path = temp_path("does-not-exist");
        assert!(SceneState::load_snapshot(path.to_str().unwrap()).is_none());
    }

    #[test]
    fn load_corrupt_file_returns_none_instead_of_panicking() {
        let path = temp_path("corrupt");
        std::fs::write(&path, b"not valid json at all").unwrap();
        assert!(SceneState::load_snapshot(path.to_str().unwrap()).is_none());
        let _ = std::fs::remove_file(&path);
    }
}
