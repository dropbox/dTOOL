//! Stack component - z-axis layering.

use crate::node::{BoxNode, Node};
use crate::style::BorderStyle;

/// Z-axis layering component.
///
/// Stacks children on top of each other, with later children rendered
/// on top of earlier ones. Useful for modals, tooltips, overlays, etc.
///
/// Note: In terminal rendering, true z-axis layering is simulated by
/// rendering layers in order. Later layers overwrite earlier ones at
/// overlapping positions.
///
/// # Example
///
/// ```ignore
/// use inky::prelude::*;
///
/// let stack = Stack::new()
///     .layer(BoxNode::new()
///         .width(20)
///         .height(10)
///         .background(Color::Blue))
///     .layer(BoxNode::new()
///         .width(10)
///         .height(5)
///         .background(Color::Red));
/// ```
#[derive(Debug, Clone)]
pub struct Stack {
    /// Layers (rendered bottom to top).
    layers: Vec<Node>,
    /// Width of the stack.
    width: Option<u16>,
    /// Height of the stack.
    height: Option<u16>,
    /// Border style.
    border: BorderStyle,
}

impl Stack {
    /// Create a new stack.
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            width: None,
            height: None,
            border: BorderStyle::None,
        }
    }

    /// Add a layer to the stack.
    /// Later layers render on top of earlier ones.
    pub fn layer(mut self, node: impl Into<Node>) -> Self {
        self.layers.push(node.into());
        self
    }

    /// Add multiple layers.
    pub fn layers(mut self, nodes: impl IntoIterator<Item = impl Into<Node>>) -> Self {
        self.layers.extend(nodes.into_iter().map(Into::into));
        self
    }

    /// Set the width of the stack.
    pub fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the height of the stack.
    pub fn height(mut self, height: u16) -> Self {
        self.height = Some(height);
        self
    }

    /// Set border style.
    pub fn border(mut self, border: BorderStyle) -> Self {
        self.border = border;
        self
    }

    /// Get the number of layers.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// Get a reference to the layers.
    pub fn get_layers(&self) -> &[Node] {
        &self.layers
    }

    /// Get the topmost layer.
    pub fn top(&self) -> Option<&Node> {
        self.layers.last()
    }

    /// Get the bottommost layer.
    pub fn bottom(&self) -> Option<&Node> {
        self.layers.first()
    }

    /// Insert a layer at a specific index.
    pub fn insert_layer(mut self, index: usize, node: impl Into<Node>) -> Self {
        let idx = index.min(self.layers.len());
        self.layers.insert(idx, node.into());
        self
    }

    /// Remove a layer at a specific index.
    pub fn remove_layer(&mut self, index: usize) -> Option<Node> {
        if index < self.layers.len() {
            Some(self.layers.remove(index))
        } else {
            None
        }
    }

    /// Pop the topmost layer.
    pub fn pop_layer(&mut self) -> Option<Node> {
        self.layers.pop()
    }
}

impl Default for Stack {
    fn default() -> Self {
        Self::new()
    }
}

impl From<Stack> for Node {
    fn from(stack: Stack) -> Self {
        // For terminal rendering, we represent the stack as a box containing
        // all layers. The actual z-ordering is handled during the render phase
        // where later children overwrite earlier ones in overlapping regions.
        //
        // In a real terminal, we can't do true compositing, so we rely on the
        // render order: render base layer first, then overlay layers on top.

        let mut container = BoxNode::new().border(stack.border);

        if let Some(w) = stack.width {
            container = container.width(w);
        }
        if let Some(h) = stack.height {
            container = container.height(h);
        }

        // Add all layers as children
        // The render pipeline should handle them appropriately
        for layer in stack.layers {
            container = container.child(layer);
        }

        container.into()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::node::TextNode;

    #[test]
    fn test_stack_new() {
        let stack = Stack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
    }

    #[test]
    fn test_stack_layers() {
        let stack = Stack::new()
            .layer(TextNode::new("Base"))
            .layer(TextNode::new("Middle"))
            .layer(TextNode::new("Top"));

        assert_eq!(stack.len(), 3);
        assert!(!stack.is_empty());
    }

    #[test]
    fn test_stack_top_bottom() {
        let stack = Stack::new()
            .layer(TextNode::new("Base"))
            .layer(TextNode::new("Top"));

        assert!(stack.bottom().is_some());
        assert!(stack.top().is_some());
    }

    #[test]
    fn test_stack_insert() {
        let stack = Stack::new()
            .layer(TextNode::new("First"))
            .layer(TextNode::new("Third"))
            .insert_layer(1, TextNode::new("Second"));

        assert_eq!(stack.len(), 3);
    }

    #[test]
    fn test_stack_remove() {
        let mut stack = Stack::new()
            .layer(TextNode::new("Base"))
            .layer(TextNode::new("Top"));

        let removed = stack.pop_layer();
        assert!(removed.is_some());
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn test_stack_to_node() {
        let stack = Stack::new()
            .layer(TextNode::new("Layer 1"))
            .layer(TextNode::new("Layer 2"))
            .width(20)
            .height(10);

        let node: Node = stack.into();
        assert!(matches!(&node, Node::Box(_)));
        if let Node::Box(b) = node {
            assert_eq!(b.children.len(), 2);
        }
    }

    #[test]
    fn test_stack_dimensions() {
        let stack = Stack::new()
            .width(30)
            .height(15)
            .border(BorderStyle::Single);

        let node: Node = stack.into();
        assert!(matches!(&node, Node::Box(_)));
        if let Node::Box(b) = node {
            assert_eq!(b.style.border, BorderStyle::Single);
        }
    }
}
