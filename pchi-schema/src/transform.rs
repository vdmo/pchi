//! 3D transform representation for PCHI scene objects

use serde::{Deserialize, Serialize};

/// 3D transform with position, rotation, and scale
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub position: [f64; 3],
    pub rotation: [f64; 4], // Quaternion (x, y, z, w)
    pub scale: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0, 1.0], // Identity quaternion
            scale: [1.0, 1.0, 1.0],
        }
    }
}

impl Transform {
    /// Create a new transform
    pub fn new(position: [f64; 3], rotation: [f64; 4], scale: [f64; 3]) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }
    
    /// Create a transform with only position
    pub fn from_position(position: [f64; 3]) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }
    
    /// Get the distance to another transform
    pub fn distance_to(&self, other: &Transform) -> f64 {
        let dx = self.position[0] - other.position[0];
        let dy = self.position[1] - other.position[1];
        let dz = self.position[2] - other.position[2];
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
    
    /// Check if the transform is valid (no NaN or infinite values)
    pub fn is_valid(&self) -> bool {
        self.position.iter().all(|&v| v.is_finite()) &&
        self.rotation.iter().all(|&v| v.is_finite()) &&
        self.scale.iter().all(|&v| v.is_finite() && v > 0.0)
    }
    
    /// Normalize the rotation quaternion
    pub fn normalize_rotation(&mut self) {
        let len = (self.rotation[0].powi(2) + 
                   self.rotation[1].powi(2) + 
                   self.rotation[2].powi(2) + 
                   self.rotation[3].powi(2)).sqrt();
        
        if len > 0.0 {
            self.rotation[0] /= len;
            self.rotation[1] /= len;
            self.rotation[2] /= len;
            self.rotation[3] /= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_transform() {
        let transform = Transform::default();
        
        assert_eq!(transform.position, [0.0, 0.0, 0.0]);
        assert_eq!(transform.rotation, [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(transform.scale, [1.0, 1.0, 1.0]);
    }
    
    #[test]
    fn test_distance_calculation() {
        let t1 = Transform::from_position([0.0, 0.0, 0.0]);
        let t2 = Transform::from_position([3.0, 4.0, 0.0]);
        
        let distance = t1.distance_to(&t2);
        assert!((distance - 5.0).abs() < 1e-9);
    }
    
    #[test]
    fn test_transform_validation() {
        let mut transform = Transform::default();
        assert!(transform.is_valid());
        
        // Invalid scale (negative)
        transform.scale = [1.0, -1.0, 1.0];
        assert!(!transform.is_valid());
        
        // Invalid position (NaN)
        transform.position = [f64::NAN, 0.0, 0.0];
        assert!(!transform.is_valid());
    }
    
    #[test]
    fn test_rotation_normalization() {
        let mut transform = Transform::new(
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        );
        
        transform.normalize_rotation();
        
        let len = (transform.rotation[0].powi(2) + 
                   transform.rotation[1].powi(2) + 
                   transform.rotation[2].powi(2) + 
                   transform.rotation[3].powi(2)).sqrt();
        
        assert!((len - 1.0).abs() < 1e-9);
    }
}
