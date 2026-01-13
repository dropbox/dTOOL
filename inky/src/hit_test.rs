//! Hit testing utilities for mouse coordinate to node mapping.
//!
//! This module provides infrastructure for determining which UI element
//! is at a given screen coordinate, enabling mouse interaction support.
//!
//! # Overview
//!
//! Hit testing traverses the node tree depth-first, checking each node's
//! computed layout rectangle against the mouse position. The deepest
//! (most specific) matching node is returned.
//!
//! # Example
//!
//! ```rust
//! use inky::prelude::*;
//! use inky::hit_test::{HitTester, HitTestResult};
//! use inky::layout::LayoutEngine;
//!
//! // Build your node tree
//! let root = BoxNode::new()
//!     .width(80)
//!     .height(24)
//!     .child(TextNode::new("Click me!"))
//!     .into();
//!
//! // Compute layout
//! let mut engine = LayoutEngine::new();
//! engine.build(&root).unwrap();
//! engine.compute(80, 24).unwrap();
//!
//! // Create hit tester
//! let tester = HitTester::new(&root, &engine);
//!
//! // Test a coordinate
//! if let Some(result) = tester.hit_test(10, 5) {
//!     println!("Hit node: {:?}", result.node_id);
//! }
//! ```

use crate::layout::{Layout, LayoutEngine};
use crate::node::{Node, NodeId};
use smallvec::SmallVec;

/// Result of a hit test, containing the hit node and its layout.
#[derive(Debug, Clone)]
pub struct HitTestResult {
    /// The ID of the node that was hit.
    pub node_id: NodeId,
    /// The computed layout of the hit node.
    pub layout: Layout,
    /// Absolute X coordinate within the hit node (0-based from node's left edge).
    pub local_x: u16,
    /// Absolute Y coordinate within the hit node (0-based from node's top edge).
    pub local_y: u16,
    /// Path from root to the hit node (list of ancestor NodeIds).
    pub path: SmallVec<[NodeId; 8]>,
}

impl HitTestResult {
    /// Check if this result represents a hit within the given node's bounds.
    pub fn is_within(&self, node_id: NodeId) -> bool {
        self.node_id == node_id || self.path.contains(&node_id)
    }
}

/// Hit testing engine for mapping coordinates to nodes.
///
/// Performs depth-first traversal of the node tree to find the deepest
/// node containing the given coordinates.
pub struct HitTester<'a> {
    root: &'a Node,
    engine: &'a LayoutEngine,
}

impl<'a> HitTester<'a> {
    /// Create a new hit tester for the given node tree and layout.
    ///
    /// # Arguments
    /// * `root` - The root of the node tree
    /// * `engine` - The layout engine with computed layouts
    pub fn new(root: &'a Node, engine: &'a LayoutEngine) -> Self {
        Self { root, engine }
    }

    /// Perform a hit test at the given screen coordinates.
    ///
    /// Returns the deepest node containing the point, or `None` if no node
    /// contains the coordinates (e.g., outside the root bounds).
    ///
    /// # Arguments
    /// * `x` - X coordinate in terminal columns (0-based)
    /// * `y` - Y coordinate in terminal rows (0-based)
    pub fn hit_test(&self, x: u16, y: u16) -> Option<HitTestResult> {
        let mut path = SmallVec::new();
        self.hit_test_recursive(self.root, x, y, 0, 0, &mut path)
    }

    /// Perform a hit test and return all nodes that contain the point.
    ///
    /// Returns nodes from root to deepest, which is useful for event
    /// propagation (capture phase goes root→leaf, bubble goes leaf→root).
    ///
    /// # Arguments
    /// * `x` - X coordinate in terminal columns (0-based)
    /// * `y` - Y coordinate in terminal rows (0-based)
    pub fn hit_test_all(&self, x: u16, y: u16) -> Vec<HitTestResult> {
        let mut results = Vec::new();
        self.collect_hits_recursive(self.root, x, y, 0, 0, &mut SmallVec::new(), &mut results);
        results
    }

    fn hit_test_recursive(
        &self,
        node: &Node,
        x: u16,
        y: u16,
        parent_x: u16,
        parent_y: u16,
        path: &mut SmallVec<[NodeId; 8]>,
    ) -> Option<HitTestResult> {
        // Get this node's layout
        let layout = self.engine.get(node.id())?;

        // Calculate absolute position
        let abs_x = parent_x.saturating_add(layout.x);
        let abs_y = parent_y.saturating_add(layout.y);

        // Check if point is within this node's bounds
        if !Self::point_in_rect(x, y, abs_x, abs_y, layout.width, layout.height) {
            return None;
        }

        // Point is within this node - add to path
        path.push(node.id());

        // Try children first (depth-first, deepest hit wins)
        // Iterate in reverse order so later children (drawn on top) are tested first
        for child in node.children().iter().rev() {
            if let Some(result) = self.hit_test_recursive(child, x, y, abs_x, abs_y, path) {
                return Some(result);
            }
        }

        // No child was hit, but this node was - return this node
        Some(HitTestResult {
            node_id: node.id(),
            layout: Layout::new(abs_x, abs_y, layout.width, layout.height),
            local_x: x.saturating_sub(abs_x),
            local_y: y.saturating_sub(abs_y),
            path: path.clone(),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_hits_recursive(
        &self,
        node: &Node,
        x: u16,
        y: u16,
        parent_x: u16,
        parent_y: u16,
        path: &mut SmallVec<[NodeId; 8]>,
        results: &mut Vec<HitTestResult>,
    ) {
        // Get this node's layout
        let Some(layout) = self.engine.get(node.id()) else {
            return;
        };

        // Calculate absolute position
        let abs_x = parent_x.saturating_add(layout.x);
        let abs_y = parent_y.saturating_add(layout.y);

        // Check if point is within this node's bounds
        if !Self::point_in_rect(x, y, abs_x, abs_y, layout.width, layout.height) {
            return;
        }

        // Point is within this node
        path.push(node.id());

        // Add this node to results
        results.push(HitTestResult {
            node_id: node.id(),
            layout: Layout::new(abs_x, abs_y, layout.width, layout.height),
            local_x: x.saturating_sub(abs_x),
            local_y: y.saturating_sub(abs_y),
            path: path.clone(),
        });

        // Recurse into children
        for child in node.children().iter() {
            self.collect_hits_recursive(child, x, y, abs_x, abs_y, path, results);
        }

        path.pop();
    }

    #[inline]
    fn point_in_rect(px: u16, py: u16, x: u16, y: u16, width: u16, height: u16) -> bool {
        px >= x && px < x.saturating_add(width) && py >= y && py < y.saturating_add(height)
    }
}

/// Trait for nodes that can respond to mouse events.
///
/// Implement this trait on custom widgets that need mouse interaction.
pub trait MouseTarget {
    /// Check if this node handles mouse events at the given local coordinates.
    ///
    /// Default implementation returns `true`, meaning the node accepts all
    /// mouse events within its bounds.
    fn accepts_mouse(&self, _local_x: u16, _local_y: u16) -> bool {
        true
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::node::BoxNode;
    use crate::style::FlexDirection;

    fn setup_simple_tree() -> (Node, LayoutEngine) {
        // Create a simple tree:
        // Root (80x24)
        // ├── Child1 (40x12)
        // └── Child2 (40x12)
        let root = BoxNode::new()
            .width(80)
            .height(24)
            .flex_direction(FlexDirection::Row)
            .child(BoxNode::new().width(40).height(12))
            .child(BoxNode::new().width(40).height(12))
            .into();

        let mut engine = LayoutEngine::new();
        engine.build(&root).unwrap();
        engine.compute(80, 24).unwrap();

        (root, engine)
    }

    fn setup_nested_tree() -> (Node, LayoutEngine) {
        // Create a nested tree:
        // Root (80x24)
        // └── Outer (60x20)
        //     └── Inner (30x10)
        let root = BoxNode::new()
            .width(80)
            .height(24)
            .child(
                BoxNode::new()
                    .width(60)
                    .height(20)
                    .child(BoxNode::new().width(30).height(10)),
            )
            .into();

        let mut engine = LayoutEngine::new();
        engine.build(&root).unwrap();
        engine.compute(80, 24).unwrap();

        (root, engine)
    }

    #[test]
    fn test_hit_test_root() {
        let (root, engine) = setup_simple_tree();
        let tester = HitTester::new(&root, &engine);

        // Hit in root
        let result = tester.hit_test(0, 0);
        assert!(result.is_some());
    }

    #[test]
    fn test_hit_test_child() {
        let (root, engine) = setup_simple_tree();
        let tester = HitTester::new(&root, &engine);

        // Hit in first child (left side)
        let result = tester.hit_test(10, 5).unwrap();
        // Should hit the first child, not the root
        assert_ne!(result.node_id, root.id());
        assert!(result.path.len() >= 2);
    }

    #[test]
    fn test_hit_test_outside() {
        let (root, engine) = setup_simple_tree();
        let tester = HitTester::new(&root, &engine);

        // Hit outside root bounds
        let result = tester.hit_test(100, 100);
        assert!(result.is_none());
    }

    #[test]
    fn test_hit_test_local_coordinates() {
        let (root, engine) = setup_simple_tree();
        let tester = HitTester::new(&root, &engine);

        // Hit at (5, 3) - should be local (5, 3) in root
        let result = tester.hit_test(5, 3).unwrap();
        assert_eq!(result.local_x, 5);
        assert_eq!(result.local_y, 3);
    }

    #[test]
    fn test_hit_test_nested() {
        let (root, engine) = setup_nested_tree();
        let tester = HitTester::new(&root, &engine);

        // Hit in the innermost box
        let result = tester.hit_test(5, 5).unwrap();
        // Path should have 3 nodes: root -> outer -> inner
        assert_eq!(result.path.len(), 3);
    }

    #[test]
    fn test_hit_test_all() {
        let (root, engine) = setup_nested_tree();
        let tester = HitTester::new(&root, &engine);

        // Get all hits at a point in the inner box
        let results = tester.hit_test_all(5, 5);
        // Should have 3 results: root, outer, inner
        assert_eq!(results.len(), 3);
        // First result should be root
        assert_eq!(results[0].node_id, root.id());
    }

    #[test]
    fn test_hit_test_result_is_within() {
        let (root, engine) = setup_nested_tree();
        let tester = HitTester::new(&root, &engine);

        let result = tester.hit_test(5, 5).unwrap();

        // Should be within root (in path)
        assert!(result.is_within(root.id()));
    }

    #[test]
    fn test_hit_test_edge_cases() {
        let (root, engine) = setup_simple_tree();
        let tester = HitTester::new(&root, &engine);

        // Test at exact boundary (0,0) - should hit
        assert!(tester.hit_test(0, 0).is_some());

        // Test at right edge - should hit (width-1)
        assert!(tester.hit_test(79, 0).is_some());

        // Test past right edge - should not hit
        assert!(tester.hit_test(80, 0).is_none());

        // Test at bottom edge - should hit (height-1)
        assert!(tester.hit_test(0, 23).is_some());

        // Test past bottom edge - should not hit
        assert!(tester.hit_test(0, 24).is_none());
    }

    #[test]
    fn test_empty_tree_hit_test() {
        let root = BoxNode::new().width(80).height(24).into();
        let mut engine = LayoutEngine::new();
        engine.build(&root).unwrap();
        engine.compute(80, 24).unwrap();

        let tester = HitTester::new(&root, &engine);

        // Should hit the root even with no children
        let result = tester.hit_test(10, 10).unwrap();
        assert_eq!(result.node_id, root.id());
    }
}
