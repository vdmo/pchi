//! PIR (Prime Integer Relations) mathematical invariants for PCHI messages

use serde::{Deserialize, Serialize};

/// PIR mathematical invariants embedded in PCHI messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PIRInvariants {
    /// Equilibrium check results
    pub equilibrium_check: EquilibriumCheck,
    
    /// Coherence gap measure
    pub coherence_gap: f64,
    
    /// Curvature signature from geometric analysis
    pub curvature_signature: String,
    
    /// Residual from arithmetic balance equation
    pub residual: f64,
}

/// Equilibrium check results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EquilibriumCheck {
    /// Residual value (should be ~0 for equilibrium)
    pub residual: f64,
    
    /// Precision threshold
    pub precision: f64,
    
    /// Prouhet-Thue-Morse sign sequence
    pub sign_sequence: Vec<i8>,
}

impl Default for PIRInvariants {
    fn default() -> Self {
        Self {
            equilibrium_check: EquilibriumCheck::default(),
            coherence_gap: 0.0,
            curvature_signature: "0.0, 0.0, 0.0".to_string(),
            residual: 0.0,
        }
    }
}

impl Default for EquilibriumCheck {
    fn default() -> Self {
        Self {
            residual: 0.0,
            precision: 1e-12,
            sign_sequence: vec![1, -1, -1, 1],
        }
    }
}

impl PIRInvariants {
    /// Create new invariants with calculated equilibrium
    pub fn new(sign_sequence: Vec<i8>, values: Vec<f64>) -> Self {
        let residual = calculate_residual(&sign_sequence, &values);
        
        Self {
            equilibrium_check: EquilibriumCheck {
                residual,
                precision: 1e-12,
                sign_sequence,
            },
            coherence_gap: calculate_coherence_gap(&values),
            curvature_signature: calculate_curvature_signature(&values),
            residual,
        }
    }
    
    /// Check if the system is in equilibrium
    pub fn is_equilibrium(&self) -> bool {
        self.equilibrium_check.residual.abs() < self.equilibrium_check.precision
    }
    
    /// Get the equilibrium status as a string
    pub fn equilibrium_status(&self) -> &'static str {
        if self.is_equilibrium() {
            "EQUILIBRIUM"
        } else {
            "DISEQUILIBRIUM"
        }
    }
}

/// Calculate residual from PIR sign sequence and values
fn calculate_residual(sign_sequence: &[i8], values: &[f64]) -> f64 {
    let mut sum = 0.0;
    for (i, &sign) in sign_sequence.iter().enumerate() {
        if i < values.len() {
            sum += sign as f64 * values[i];
        }
    }
    sum.abs()
}

/// Calculate coherence gap measure
fn calculate_coherence_gap(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values.iter()
        .map(|v| (v - mean).powi(2))
        .sum::<f64>() / values.len() as f64;
    
    variance.sqrt()
}

/// Calculate curvature signature from values
fn calculate_curvature_signature(values: &[f64]) -> String {
    if values.len() < 3 {
        return "0.0, 0.0, 0.0".to_string();
    }
    
    // Simplified curvature calculation
    let c1 = (values[1] - values[0]).abs();
    let c2 = (values[2] - values[1]).abs();
    let c3 = ((values[2] - values[0]) / 2.0).abs();
    
    format!("{:.3}, {:.3}, {:.3}", c1, c2, c3)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_equilibrium_calculation() {
        let signs = vec![1, -1, -1, 1];
        let values = vec![10.0, 5.0, 5.0, 10.0];
        
        let invariants = PIRInvariants::new(signs, values);
        
        // 10 - 5 - 5 + 10 = 10, so not in equilibrium
        assert!(!invariants.is_equilibrium());
    }
    
    #[test]
    fn test_perfect_equilibrium() {
        let signs = vec![1, -1, -1, 1];
        let values = vec![10.0, 10.0, 10.0, 10.0];
        
        let invariants = PIRInvariants::new(signs, values);
        
        // 10 - 10 - 10 + 10 = 0, perfect equilibrium
        assert!(invariants.is_equilibrium());
    }
    
    #[test]
    fn test_default_invariants() {
        let invariants = PIRInvariants::default();
        
        assert!(invariants.is_equilibrium());
        assert_eq!(invariants.equilibrium_status(), "EQUILIBRIUM");
    }
}
