//! Spacer component - flexible space filler.

use crate::node::{BoxNode, Node};

/// A flexible spacer that fills available space.
#[derive(Debug, Clone)]
pub struct Spacer {
    flex_grow: f32,
}

impl Spacer {
    /// Create a new spacer with default flex grow of 1.
    pub fn new() -> Self {
        Self { flex_grow: 1.0 }
    }

    /// Set flex grow value.
    pub fn flex_grow(mut self, value: f32) -> Self {
        self.flex_grow = value;
        self
    }
}

impl Default for Spacer {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Spacer> for Node {
    fn from(spacer: Spacer) -> Self {
        BoxNode::new().flex_grow(spacer.flex_grow).into()
    }
}
