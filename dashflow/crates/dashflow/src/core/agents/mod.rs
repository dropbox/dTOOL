// Allow clippy warnings for agent framework
// - expect_used: Agent execution uses expect() for configuration validation
// - clone_on_ref_ptr: Agents clone Arc for shared state
#![allow(clippy::items_after_test_module)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Agent framework for autonomous tool-using LLM systems
//!
//! Agents combine language models with tools to autonomously solve tasks through
//! iterative reasoning and action loops. This module provides the core abstractions
//! for building agents that can:
//!
//! - Reason about what actions to take
//! - Execute tools based on observations
//! - Maintain conversation state across iterations
//! - Make decisions about when to stop
//!
//! # Architecture
//!
//! The agent system is built around several core types:
//!
//! - [`Agent`]: Core trait for agent implementations
//! - [`AgentAction`]: Represents a decision to use a tool
//! - [`AgentFinish`]: Represents a decision to return a final answer
//! - [`AgentStep`]: Records an action and its observation
//! - [`AgentExecutor`]: Runs the agent loop with tools
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow::core::agents::{Agent, AgentExecutor};
//! use dashflow::core::tools::Tool;
//!
//! async fn run_agent() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create tools
//!     let tools: Vec<Box<dyn Tool>> = vec![
//!         // Add your tools here
//!     ];
//!
//!     // Create agent (implementation specific)
//!     // let agent = create_react_agent(model, tools)?;
//!
//!     // Execute agent
//!     // let executor = AgentExecutor::new(agent, tools);
//!     // let result = executor.execute("What is 25 * 4?").await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Agent Types
//!
//! This module provides the foundation for various agent types:
//!
//! - **`ReAct` agents**: Reason and Act in an interleaved manner
//! - **Plan-and-Execute agents**: Plan all steps upfront, then execute
//! - **Conversational agents**: Maintain conversation history
//!
//! The specific agent implementations will be built on top of these abstractions.

// Existing submodules
mod checkpoint;
mod json_chat;
mod memory;
mod middleware;
mod xml;

// Split from legacy `mod.rs` (CQ-37)
mod config;
mod context;
mod executor;
mod openai_functions;
mod openai_tools;
mod react;
mod self_ask_with_search;
mod structured_chat;
mod tool_calling;
mod traits;
mod types;

// Re-exports from existing submodules
pub use checkpoint::{
    AgentCheckpointState, Checkpoint, FileCheckpoint, MemoryCheckpoint, DEFAULT_CHECKPOINTS_LIMIT,
};
pub use json_chat::JsonChatAgent;
pub use memory::{BufferMemory, ConversationBufferWindowMemory, Memory};
#[cfg(any(test, feature = "testing"))]
pub use middleware::ToolEmulatorMiddleware;
pub use middleware::{
    AgentMiddleware, HumanInTheLoopMiddleware, LoggingMiddleware, ModelFallbackMiddleware,
    RateLimitMiddleware, RetryMiddleware, TimeoutMiddleware, ValidationMiddleware,
};
pub use xml::XmlAgent;

// Re-exports from CQ-37 split modules
pub use config::AgentConfigError;
pub use context::AgentContext;
#[allow(deprecated)]
pub use executor::{AgentExecutor, AgentExecutorConfig, AgentExecutorResult};
pub use openai_functions::OpenAIFunctionsAgent;
pub use openai_tools::OpenAIToolsAgent;
pub use react::ReActAgent;
#[allow(deprecated)]
pub use react::{MRKLAgent, ZeroShotAgent};
pub use self_ask_with_search::SelfAskWithSearchAgent;
pub use structured_chat::StructuredChatAgent;
pub use tool_calling::ToolCallingAgent;
pub use traits::Agent;
pub use types::{AgentAction, AgentDecision, AgentFinish, AgentStep};

#[allow(deprecated)]
#[cfg(test)]
mod tests;
