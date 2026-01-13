//! @dashflow-module
//! @name tracers
//! @category core
//! @status stable
//!
//! Tracer system for DashFlow execution tracking
//!
//! This module provides tracing infrastructure for observability and debugging
//! of DashFlow applications. Tracers extend the callback system with structured
//! run tracking that can be persisted to external systems like LangSmith.

mod base;
mod dashflow;
mod root_listeners;
mod run_collector;
mod stdout;

pub use base::{BaseTracer, RunTree, RunType};
pub use dashflow::DashFlowTracer;
pub use root_listeners::{AsyncListener, Listener, RootListenersTracer};
pub use run_collector::RunCollectorCallbackHandler;
pub use stdout::{ConsoleCallbackHandler, FunctionCallbackHandler};

/// Type alias for backwards compatibility with Python's LangChain API.
///
/// In Python LangChain, `Run` refers to a run tree node representing
/// a single execution step. This alias maps to [`RunTree`] in DashFlow.
pub type Run = RunTree;
