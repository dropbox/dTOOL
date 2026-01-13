// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Objective definitions for multi-objective optimization.

use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Types of objectives that can be optimized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ObjectiveType {
    /// Quality objective (e.g., accuracy, F1 score)
    /// Higher is better. Values typically in [0.0, 1.0]
    Quality,

    /// Cost objective (e.g., USD per 1000 requests)
    /// Lower is better. Values in USD
    Cost,

    /// Latency objective (e.g., average response time in milliseconds)
    /// Lower is better. Values in milliseconds
    Latency,

    /// Token usage objective (e.g., average tokens per request)
    /// Lower is better. Values in token count
    TokenUsage,
}

impl ObjectiveType {
    /// Returns true if this objective should be maximized (higher is better).
    pub fn is_maximized(&self) -> bool {
        matches!(self, ObjectiveType::Quality)
    }

    /// Returns true if this objective should be minimized (lower is better).
    pub fn is_minimized(&self) -> bool {
        !self.is_maximized()
    }
}

impl fmt::Display for ObjectiveType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ObjectiveType::Quality => write!(f, "quality"),
            ObjectiveType::Cost => write!(f, "cost"),
            ObjectiveType::Latency => write!(f, "latency"),
            ObjectiveType::TokenUsage => write!(f, "token_usage"),
        }
    }
}

/// An objective with its type and weight.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    /// The type of objective
    pub objective_type: ObjectiveType,

    /// Weight for this objective in [0.0, 1.0]
    /// Higher weight means more importance
    pub weight: f64,

    /// Optional name for this objective
    pub name: Option<String>,
}

/// Error type for objective configuration validation.
#[derive(Debug, Clone, PartialEq, Error)]
#[non_exhaustive]
pub enum ObjectiveError {
    /// Weight must be in [0.0, 1.0].
    #[error("Weight must be in [0.0, 1.0], got {weight}")]
    InvalidWeight {
        /// The invalid weight value.
        weight: f64,
    },
}

impl Objective {
    /// Create a new objective with given type and weight.
    ///
    /// # Arguments
    /// * `objective_type` - The type of objective
    /// * `weight` - Weight in [0.0, 1.0] (higher = more important)
    ///
    /// # Panics
    /// Panics if weight is not in [0.0, 1.0]
    // SAFETY: Panicking constructor with documented behavior; use try_new() for fallible version
    #[allow(clippy::expect_used)]
    pub fn new(objective_type: ObjectiveType, weight: f64) -> Self {
        Self::try_new(objective_type, weight).expect("Weight must be in [0.0, 1.0]")
    }

    /// Create a new objective with given type and weight, returning an error if invalid.
    ///
    /// # Arguments
    /// * `objective_type` - The type of objective
    /// * `weight` - Weight in [0.0, 1.0] (higher = more important)
    ///
    /// # Errors
    /// Returns `ObjectiveError::InvalidWeight` if weight is not in [0.0, 1.0].
    pub fn try_new(objective_type: ObjectiveType, weight: f64) -> Result<Self, ObjectiveError> {
        if !(0.0..=1.0).contains(&weight) {
            return Err(ObjectiveError::InvalidWeight { weight });
        }

        Ok(Self {
            objective_type,
            weight,
            name: None,
        })
    }

    /// Set a custom name for this objective.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Get the display name for this objective.
    pub fn display_name(&self) -> String {
        self.name
            .clone()
            .unwrap_or_else(|| self.objective_type.to_string())
    }
}

/// Value of an objective for a particular solution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ObjectiveValue {
    /// The type of objective
    pub objective_type: ObjectiveType,

    /// The measured value
    pub value: f64,
}

impl ObjectiveValue {
    /// Create a new objective value.
    pub fn new(objective_type: ObjectiveType, value: f64) -> Self {
        Self {
            objective_type,
            value,
        }
    }

    /// Compare this value with another for the same objective type.
    /// Returns true if this value is better than the other.
    ///
    /// For maximized objectives (Quality), higher is better.
    /// For minimized objectives (Cost, Latency, TokenUsage), lower is better.
    pub fn is_better_than(&self, other: &Self) -> bool {
        assert_eq!(
            self.objective_type, other.objective_type,
            "Cannot compare different objective types"
        );

        if self.objective_type.is_maximized() {
            self.value > other.value
        } else {
            self.value < other.value
        }
    }

    /// Returns true if this value dominates the other (is better or equal on this objective).
    pub fn dominates(&self, other: &Self) -> bool {
        assert_eq!(
            self.objective_type, other.objective_type,
            "Cannot compare different objective types"
        );

        if self.objective_type.is_maximized() {
            self.value >= other.value
        } else {
            self.value <= other.value
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_objective_type_maximized() {
        assert!(ObjectiveType::Quality.is_maximized());
        assert!(!ObjectiveType::Cost.is_maximized());
        assert!(!ObjectiveType::Latency.is_maximized());
        assert!(!ObjectiveType::TokenUsage.is_maximized());
    }

    #[test]
    fn test_objective_type_minimized() {
        assert!(!ObjectiveType::Quality.is_minimized());
        assert!(ObjectiveType::Cost.is_minimized());
        assert!(ObjectiveType::Latency.is_minimized());
        assert!(ObjectiveType::TokenUsage.is_minimized());
    }

    #[test]
    fn test_objective_new() {
        let obj = Objective::new(ObjectiveType::Quality, 0.7);
        assert_eq!(obj.objective_type, ObjectiveType::Quality);
        assert_eq!(obj.weight, 0.7);
        assert_eq!(obj.name, None);
    }

    #[test]
    fn test_objective_try_new_valid() {
        let result = Objective::try_new(ObjectiveType::Quality, 0.7);
        assert!(result.is_ok());
        let obj = result.unwrap();
        assert_eq!(obj.objective_type, ObjectiveType::Quality);
        assert_eq!(obj.weight, 0.7);
    }

    #[test]
    fn test_objective_try_new_invalid_weight() {
        let result = Objective::try_new(ObjectiveType::Quality, 1.5);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ObjectiveError::InvalidWeight { weight } if weight == 1.5
        ));
    }

    #[test]
    fn test_objective_try_new_invalid_weight_negative() {
        let result = Objective::try_new(ObjectiveType::Quality, -0.1);
        assert!(result.is_err());
    }

    #[test]
    fn test_objective_with_name() {
        let obj = Objective::new(ObjectiveType::Quality, 0.7).with_name("accuracy");
        assert_eq!(obj.name, Some("accuracy".to_string()));
        assert_eq!(obj.display_name(), "accuracy");
    }

    #[test]
    fn test_objective_display_name_default() {
        let obj = Objective::new(ObjectiveType::Cost, 0.3);
        assert_eq!(obj.display_name(), "cost");
    }

    #[test]
    fn test_objective_value_is_better_quality() {
        let v1 = ObjectiveValue::new(ObjectiveType::Quality, 0.9);
        let v2 = ObjectiveValue::new(ObjectiveType::Quality, 0.8);
        assert!(v1.is_better_than(&v2));
        assert!(!v2.is_better_than(&v1));
    }

    #[test]
    fn test_objective_value_is_better_cost() {
        let v1 = ObjectiveValue::new(ObjectiveType::Cost, 0.01);
        let v2 = ObjectiveValue::new(ObjectiveType::Cost, 0.02);
        assert!(v1.is_better_than(&v2));
        assert!(!v2.is_better_than(&v1));
    }

    #[test]
    fn test_objective_value_dominates() {
        let v1 = ObjectiveValue::new(ObjectiveType::Quality, 0.9);
        let v2 = ObjectiveValue::new(ObjectiveType::Quality, 0.8);
        let v3 = ObjectiveValue::new(ObjectiveType::Quality, 0.9);

        assert!(v1.dominates(&v2));
        assert!(v1.dominates(&v3)); // Equal values dominate
        assert!(!v2.dominates(&v1));
    }

    #[test]
    fn test_objective_value_dominates_cost() {
        let v1 = ObjectiveValue::new(ObjectiveType::Cost, 0.01);
        let v2 = ObjectiveValue::new(ObjectiveType::Cost, 0.02);
        let v3 = ObjectiveValue::new(ObjectiveType::Cost, 0.01);

        assert!(v1.dominates(&v2));
        assert!(v1.dominates(&v3)); // Equal values dominate
        assert!(!v2.dominates(&v1));
    }
}
