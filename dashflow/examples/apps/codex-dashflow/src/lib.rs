//! Codex DashFlow - AI-powered code generation and understanding
//!
//! A port of OpenAI Codex to the DashFlow platform with full observability
//! and telemetry integration.
//!
//! ## Features
//!
//! | Feature | Description |
//! |---------|-------------|
//! | Code Generation | Generate code from natural language |
//! | Code Explanation | Explain what code does in plain English |
//! | Refactoring | Suggest improvements to existing code |
//! | Test Generation | Generate unit tests for functions |
//! | Documentation | Generate docstrings and comments |
//!
//! ## Usage
//!
//! ```bash
//! # Generate code from description
//! cargo run -p codex-dashflow -- generate "a function that calculates fibonacci"
//!
//! # Explain code
//! cargo run -p codex-dashflow -- explain --file src/lib.rs --symbol "my_function"
//!
//! # Generate tests
//! cargo run -p codex-dashflow -- test --file src/lib.rs --function "process_data"
//! ```

pub mod agent;
pub mod apply;
pub mod chat;
pub mod config;
pub mod docs_generator;
pub mod explainer;
pub mod generator;
pub mod mcp_server;
pub mod refactor;
pub mod session;
pub mod test_generator;

pub use agent::{
    create_coding_agent, default_agent_state, run_chat_loop, run_chat_loop_with_session,
    run_chat_loop_with_session_streaming, run_single_query, run_single_query_streaming,
    run_single_query_streaming_with_state, run_single_query_with_state,
};
pub use chat::{run_chat, ChatConfig};
pub use config::CodexConfig;
pub use docs_generator::DocsGenerator;
pub use explainer::CodeExplainer;
pub use generator::CodeGenerator;
pub use mcp_server::McpStdioServer;
pub use refactor::RefactorSuggester;
pub use session::{
    default_session_path, load_or_create_session, load_session, save_session, Session,
};
pub use test_generator::TestGenerator;

// Apply module exports
pub use apply::{generate_unified_diff, git_apply, git_diff, git_diff_staged, ApplyResult};
