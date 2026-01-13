//! Agent execution context for middleware integration.
//!
//! This module provides [`AgentContext`], which carries execution state through
//! the agent lifecycle and enables middleware to observe and modify behavior.
//!
//! # Usage
//!
//! The context is automatically created by the agent executor and passed to
//! middleware hooks. Middleware can:
//!
//! - Read the original user input
//! - Access intermediate steps (action/observation pairs)
//! - Track iteration count for loop detection
//! - Store custom metadata for cross-middleware communication
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::core::agents::{AgentContext, AgentMiddleware};
//!
//! struct LoggingMiddleware;
//!
//! impl AgentMiddleware for LoggingMiddleware {
//!     async fn before_plan(&self, ctx: &mut AgentContext) -> Result<()> {
//!         println!("Iteration {}: processing '{}'", ctx.iteration, ctx.input);
//!         Ok(())
//!     }
//! }
//! ```

use super::AgentStep;

/// Context passed to middleware during agent execution
///
/// This struct contains the current state of agent execution and is passed
/// to each middleware hook. Middleware can read and modify the context.
#[derive(Debug, Clone)]
pub struct AgentContext {
    /// The original user input
    pub input: String,

    /// Current intermediate steps
    pub intermediate_steps: Vec<AgentStep>,

    /// Current iteration number (0-indexed)
    pub iteration: usize,

    /// Additional metadata that middleware can use
    pub metadata: std::collections::HashMap<String, String>,
}

impl AgentContext {
    /// Create a new agent context
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            intermediate_steps: Vec::new(),
            iteration: 0,
            metadata: std::collections::HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_string() {
        let ctx = AgentContext::new("test input".to_string());
        assert_eq!(ctx.input, "test input");
        assert!(ctx.intermediate_steps.is_empty());
        assert_eq!(ctx.iteration, 0);
        assert!(ctx.metadata.is_empty());
    }

    #[test]
    fn test_new_with_str() {
        let ctx = AgentContext::new("test input");
        assert_eq!(ctx.input, "test input");
    }

    #[test]
    fn test_clone() {
        let mut ctx = AgentContext::new("input");
        ctx.iteration = 5;
        ctx.metadata.insert("key".to_string(), "value".to_string());

        let cloned = ctx.clone();
        assert_eq!(cloned.input, ctx.input);
        assert_eq!(cloned.iteration, ctx.iteration);
        assert_eq!(cloned.metadata.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_debug() {
        let ctx = AgentContext::new("test");
        let debug_str = format!("{:?}", ctx);
        assert!(debug_str.contains("AgentContext"));
        assert!(debug_str.contains("test"));
    }
}
