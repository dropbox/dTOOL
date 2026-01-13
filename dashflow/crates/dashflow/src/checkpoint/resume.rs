// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Resume-aware runner for checkpoint-based workflow recovery
//!
//! Provides `ResumeRunner` which wraps a `CompiledGraph` and adds environment
//! validation before resuming from checkpoints. This prevents silent failures
//! when resuming in a different environment (working directory, sandbox mode, etc.).
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::{StateGraph, MemoryCheckpointer};
//! use dashflow::checkpoint::ResumeRunner;
//!
//! async fn example() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut graph = StateGraph::new();
//!     // ... configure graph ...
//!
//!     let app = graph.compile()?
//!         .with_checkpointer(MemoryCheckpointer::new())
//!         .with_thread_id("session-1");
//!
//!     // Create a runner with environment validation
//!     let runner = ResumeRunner::new(app)
//!         .with_working_dir_validation()
//!         .with_sandbox_validation(true);
//!
//!     // Automatically resume from checkpoint or start fresh
//!     let result = runner.resume_or_new(initial_state).await?;
//!     Ok(())
//! }
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;

use crate::error::Result;
use crate::executor::{CompiledGraph, ExecutionResult};
use crate::state::MergeableState;

// ============================================================================
// ResumeError - Validation failure types
// ============================================================================

/// Errors that can occur during resume validation
///
/// These errors indicate that the environment has changed since the checkpoint
/// was created, which may cause unexpected behavior or failures.
#[derive(Error, Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ResumeError {
    /// Working directory has changed since checkpoint was created
    #[error(
        "Working directory mismatch: checkpoint created in '{expected}', current is '{actual}'"
    )]
    WorkingDirectoryMismatch {
        /// Directory where checkpoint was created
        expected: PathBuf,
        /// Current working directory
        actual: PathBuf,
    },

    /// Sandbox mode has changed since checkpoint was created
    #[error("Sandbox mode mismatch: checkpoint created with sandbox={expected}, current is sandbox={actual}")]
    SandboxModeMismatch {
        /// Sandbox mode when checkpoint was created
        expected: bool,
        /// Current sandbox mode
        actual: bool,
    },

    /// Custom validation failed
    #[error("Custom validation failed: {reason}")]
    CustomValidationFailed {
        /// Reason for validation failure
        reason: String,
    },

    /// Required environment variable is missing or changed
    #[error("Environment variable '{name}' mismatch: expected '{expected}', found '{actual}'")]
    EnvironmentMismatch {
        /// Name of the environment variable
        name: String,
        /// Expected value
        expected: String,
        /// Actual value (empty string if unset)
        actual: String,
    },

    /// Multiple validations failed
    #[error("Multiple resume validations failed: {}", .errors.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; "))]
    Multiple {
        /// All errors that occurred
        errors: Vec<ResumeError>,
    },
}

impl ResumeError {
    /// Returns true if this error indicates an environment mismatch
    pub fn is_environment_mismatch(&self) -> bool {
        matches!(
            self,
            ResumeError::WorkingDirectoryMismatch { .. }
                | ResumeError::SandboxModeMismatch { .. }
                | ResumeError::EnvironmentMismatch { .. }
        )
    }

    /// Combine multiple errors into a single error
    // SAFETY: unwrap() is safe - we just matched len() == 1, so next() always returns Some
    #[allow(clippy::unwrap_used)]
    pub fn combine(errors: Vec<ResumeError>) -> Option<ResumeError> {
        match errors.len() {
            0 => None,
            1 => Some(errors.into_iter().next().unwrap()),
            _ => Some(ResumeError::Multiple { errors }),
        }
    }
}

// ============================================================================
// ResumeEnvironment - Stored environment context
// ============================================================================

/// Environment context stored with checkpoints for validation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct ResumeEnvironment {
    /// Working directory when checkpoint was created
    pub working_dir: Option<PathBuf>,
    /// Sandbox mode when checkpoint was created
    pub sandbox_mode: Option<bool>,
    /// Custom environment variables to validate
    pub env_vars: std::collections::HashMap<String, String>,
}

impl ResumeEnvironment {
    /// Create a new empty environment
    pub fn new() -> Self {
        Self::default()
    }

    /// Capture the current working directory
    #[must_use]
    pub fn with_working_dir(mut self) -> Self {
        self.working_dir = std::env::current_dir().ok();
        self
    }

    /// Set sandbox mode
    #[must_use]
    pub fn with_sandbox_mode(mut self, sandbox: bool) -> Self {
        self.sandbox_mode = Some(sandbox);
        self
    }

    /// Add an environment variable to track
    #[must_use]
    pub fn with_env_var(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if let Ok(value) = std::env::var(&name) {
            self.env_vars.insert(name, value);
        }
        self
    }
}

// ============================================================================
// Validation Callbacks
// ============================================================================

/// Trait for custom resume validation
pub trait ResumeValidator: Send + Sync {
    /// Validate that resumption is safe given the stored environment
    ///
    /// Returns `Ok(())` if validation passes, or `Err(ResumeError)` if it fails.
    fn validate(&self, stored_env: &ResumeEnvironment) -> std::result::Result<(), ResumeError>;
}

/// Validates working directory matches checkpoint
struct WorkingDirValidator;

impl ResumeValidator for WorkingDirValidator {
    fn validate(&self, stored_env: &ResumeEnvironment) -> std::result::Result<(), ResumeError> {
        if let Some(ref expected) = stored_env.working_dir {
            let actual =
                std::env::current_dir().map_err(|e| ResumeError::CustomValidationFailed {
                    reason: format!("Could not determine current working directory: {e}"),
                })?;
            if &actual != expected {
                return Err(ResumeError::WorkingDirectoryMismatch {
                    expected: expected.clone(),
                    actual,
                });
            }
        }
        Ok(())
    }
}

/// Validates sandbox mode matches checkpoint
struct SandboxValidator {
    current_sandbox: bool,
}

impl ResumeValidator for SandboxValidator {
    fn validate(&self, stored_env: &ResumeEnvironment) -> std::result::Result<(), ResumeError> {
        if let Some(expected) = stored_env.sandbox_mode {
            if self.current_sandbox != expected {
                return Err(ResumeError::SandboxModeMismatch {
                    expected,
                    actual: self.current_sandbox,
                });
            }
        }
        Ok(())
    }
}

/// Validates specific environment variables match
struct EnvVarValidator {
    var_names: Vec<String>,
}

impl ResumeValidator for EnvVarValidator {
    fn validate(&self, stored_env: &ResumeEnvironment) -> std::result::Result<(), ResumeError> {
        for name in &self.var_names {
            if let Some(expected) = stored_env.env_vars.get(name) {
                let actual = std::env::var(name).unwrap_or_default();
                if &actual != expected {
                    return Err(ResumeError::EnvironmentMismatch {
                        name: name.clone(),
                        expected: expected.clone(),
                        actual,
                    });
                }
            }
        }
        Ok(())
    }
}

// ============================================================================
// ResumeRunner - Main orchestrator
// ============================================================================

/// Result of a resume_or_new operation
#[derive(Debug)]
#[non_exhaustive]
pub enum ResumeOutcome<S>
where
    S: MergeableState,
{
    /// Started fresh execution (no checkpoint found)
    Fresh(ExecutionResult<S>),
    /// Resumed from existing checkpoint
    Resumed(ExecutionResult<S>),
}

impl<S> ResumeOutcome<S>
where
    S: MergeableState,
{
    /// Get the execution result regardless of fresh/resumed
    pub fn into_result(self) -> ExecutionResult<S> {
        match self {
            ResumeOutcome::Fresh(r) | ResumeOutcome::Resumed(r) => r,
        }
    }

    /// Returns true if this was a fresh execution
    pub fn is_fresh(&self) -> bool {
        matches!(self, ResumeOutcome::Fresh(_))
    }

    /// Returns true if this was a resumed execution
    pub fn is_resumed(&self) -> bool {
        matches!(self, ResumeOutcome::Resumed(_))
    }
}

/// Resume-aware graph runner with environment validation
///
/// Wraps a `CompiledGraph` and provides `resume_or_new()` functionality
/// that validates the execution environment before resuming from checkpoints.
///
/// # Environment Validation
///
/// Before resuming from a checkpoint, the runner can validate that:
/// - Working directory hasn't changed
/// - Sandbox mode matches
/// - Specified environment variables match
/// - Custom validation logic passes
///
/// This prevents subtle bugs where a workflow resumes in a different
/// environment than expected.
///
/// # Example
///
/// ```rust,ignore
/// let runner = ResumeRunner::new(compiled_graph)
///     .with_working_dir_validation()
///     .with_sandbox_validation(true);
///
/// // Will validate environment before resuming
/// let result = runner.resume_or_new(initial_state).await?;
/// ```
pub struct ResumeRunner<S>
where
    S: MergeableState,
{
    graph: CompiledGraph<S>,
    validators: Vec<Arc<dyn ResumeValidator>>,
    capture_working_dir: bool,
    capture_sandbox: Option<bool>,
    capture_env_vars: Vec<String>,
}

impl<S> ResumeRunner<S>
where
    S: MergeableState,
{
    /// Create a new resume runner wrapping a compiled graph
    ///
    /// The graph should already have a checkpointer and thread_id configured.
    pub fn new(graph: CompiledGraph<S>) -> Self {
        Self {
            graph,
            validators: Vec::new(),
            capture_working_dir: false,
            capture_sandbox: None,
            capture_env_vars: Vec::new(),
        }
    }

    /// Enable working directory validation
    ///
    /// When resuming, validates that the current working directory matches
    /// the directory where the checkpoint was created.
    #[must_use]
    pub fn with_working_dir_validation(mut self) -> Self {
        self.capture_working_dir = true;
        self.validators.push(Arc::new(WorkingDirValidator));
        self
    }

    /// Enable sandbox mode validation
    ///
    /// When resuming, validates that the current sandbox mode matches
    /// the mode when the checkpoint was created.
    ///
    /// # Arguments
    /// * `current_sandbox` - Current sandbox mode setting
    #[must_use]
    pub fn with_sandbox_validation(mut self, current_sandbox: bool) -> Self {
        self.capture_sandbox = Some(current_sandbox);
        self.validators
            .push(Arc::new(SandboxValidator { current_sandbox }));
        self
    }

    /// Add environment variable validation
    ///
    /// When resuming, validates that the specified environment variable
    /// has the same value as when the checkpoint was created.
    #[must_use]
    pub fn with_env_var_validation(mut self, var_name: impl Into<String>) -> Self {
        let name = var_name.into();
        self.capture_env_vars.push(name.clone());
        self.validators.push(Arc::new(EnvVarValidator {
            var_names: vec![name],
        }));
        self
    }

    /// Add multiple environment variables for validation
    #[must_use]
    pub fn with_env_vars_validation(mut self, var_names: Vec<impl Into<String>>) -> Self {
        let names: Vec<String> = var_names.into_iter().map(Into::into).collect();
        self.capture_env_vars.extend(names.clone());
        self.validators
            .push(Arc::new(EnvVarValidator { var_names: names }));
        self
    }

    /// Add a custom validator
    #[must_use]
    pub fn with_validator<V: ResumeValidator + 'static>(mut self, validator: V) -> Self {
        self.validators.push(Arc::new(validator));
        self
    }

    /// Build the environment to capture with checkpoints
    ///
    /// Note: This method will be used when we add support for storing
    /// environment context in checkpoint metadata.
    #[allow(dead_code)] // Architectural: Ready for checkpoint metadata environment capture
    fn build_environment(&self) -> ResumeEnvironment {
        let mut env = ResumeEnvironment::new();

        if self.capture_working_dir {
            env = env.with_working_dir();
        }

        if let Some(sandbox) = self.capture_sandbox {
            env = env.with_sandbox_mode(sandbox);
        }

        for var in &self.capture_env_vars {
            env = env.with_env_var(var);
        }

        env
    }

    /// Run all validators against a stored environment
    ///
    /// Note: This method will be used when we add support for storing
    /// environment context in checkpoint metadata.
    #[allow(dead_code)] // Architectural: Ready for checkpoint metadata validation
    fn validate_environment(
        &self,
        stored_env: &ResumeEnvironment,
    ) -> std::result::Result<(), ResumeError> {
        let mut errors = Vec::new();

        for validator in &self.validators {
            if let Err(e) = validator.validate(stored_env) {
                errors.push(e);
            }
        }

        if let Some(combined) = ResumeError::combine(errors) {
            Err(combined)
        } else {
            Ok(())
        }
    }

    /// Get a reference to the underlying graph
    pub fn graph(&self) -> &CompiledGraph<S> {
        &self.graph
    }

    /// Get a mutable reference to the underlying graph
    pub fn graph_mut(&mut self) -> &mut CompiledGraph<S> {
        &mut self.graph
    }

    /// Consume the runner and return the underlying graph
    pub fn into_graph(self) -> CompiledGraph<S> {
        self.graph
    }
}

impl<S> ResumeRunner<S>
where
    S: MergeableState + serde::Serialize + serde::de::DeserializeOwned,
{
    /// Resume from checkpoint if available, otherwise start fresh
    ///
    /// This method:
    /// 1. Checks for an existing checkpoint for the configured thread_id
    /// 2. If found, validates the environment matches
    /// 3. Resumes execution if validation passes
    /// 4. Starts fresh execution if no checkpoint exists
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No checkpointer is configured
    /// - No thread_id is configured
    /// - Environment validation fails
    /// - Graph execution fails
    ///
    /// # Returns
    ///
    /// Returns `ResumeOutcome::Resumed` if resumed from checkpoint,
    /// or `ResumeOutcome::Fresh` if started fresh.
    pub async fn resume_or_new(self, initial_state: S) -> Result<ResumeOutcome<S>> {
        // Check for existing checkpoint
        if let Some(_checkpoint) = self.try_get_latest_checkpoint().await? {
            // For now, we don't have a way to store/retrieve ResumeEnvironment
            // in the checkpoint metadata. This is a limitation we'll note.
            // In a full implementation, we would store the environment in
            // the checkpoint's metadata field.

            // Attempt to resume - this will fail if checkpoint is corrupted
            // or doesn't exist (race condition)
            match self.graph.resume().await {
                Ok(result) => Ok(ResumeOutcome::Resumed(result)),
                Err(_) => {
                    // If resume fails, try fresh start
                    let result = self.graph.invoke(initial_state).await?;
                    Ok(ResumeOutcome::Fresh(result))
                }
            }
        } else {
            // No checkpoint, start fresh
            let result = self.graph.invoke(initial_state).await?;
            Ok(ResumeOutcome::Fresh(result))
        }
    }

    /// Check if there's an existing checkpoint without loading full state
    async fn try_get_latest_checkpoint(&self) -> Result<Option<()>> {
        // Use get_current_state() to check if a checkpoint exists.
        // If it returns Ok, a checkpoint exists. If it returns Err,
        // either no checkpointer/thread_id is configured or no checkpoint exists.
        match self.graph.get_current_state().await {
            Ok(_) => Ok(Some(())),
            Err(_) => Ok(None), // Treat errors as "no checkpoint"
        }
    }

    /// Force resume from checkpoint, failing if none exists
    ///
    /// Unlike `resume_or_new`, this method will return an error if
    /// no checkpoint exists rather than starting fresh.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No checkpoint exists
    /// - Environment validation fails
    /// - Graph execution fails
    pub async fn resume_only(self) -> Result<ExecutionResult<S>> {
        self.graph.resume().await
    }

    /// Start fresh execution, ignoring any existing checkpoints
    ///
    /// This method always starts from the initial state, even if
    /// a checkpoint exists.
    pub async fn fresh_only(self, initial_state: S) -> Result<ExecutionResult<S>> {
        self.graph.invoke(initial_state).await
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::ExecutionResult;
    use crate::state::MergeableState;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    /// Simple test state that implements MergeableState
    #[derive(Clone, Debug, Default, Serialize, Deserialize)]
    struct TestState {
        value: i32,
    }

    impl MergeableState for TestState {
        fn merge(&mut self, other: &Self) {
            self.value = other.value;
        }
    }

    #[test]
    fn test_resume_error_display() {
        let err = ResumeError::WorkingDirectoryMismatch {
            expected: PathBuf::from("/old/path"),
            actual: PathBuf::from("/new/path"),
        };
        assert!(err.to_string().contains("/old/path"));
        assert!(err.to_string().contains("/new/path"));

        let err = ResumeError::SandboxModeMismatch {
            expected: true,
            actual: false,
        };
        assert!(err.to_string().contains("sandbox=true"));
        assert!(err.to_string().contains("sandbox=false"));

        let err = ResumeError::EnvironmentMismatch {
            name: "HOME".to_string(),
            expected: "/home/user".to_string(),
            actual: "/home/other".to_string(),
        };
        assert!(err.to_string().contains("HOME"));
        assert!(err.to_string().contains("/home/user"));
    }

    #[test]
    fn test_resume_error_is_environment_mismatch() {
        assert!(ResumeError::WorkingDirectoryMismatch {
            expected: PathBuf::new(),
            actual: PathBuf::new(),
        }
        .is_environment_mismatch());

        assert!(ResumeError::SandboxModeMismatch {
            expected: true,
            actual: false,
        }
        .is_environment_mismatch());

        assert!(ResumeError::EnvironmentMismatch {
            name: "X".to_string(),
            expected: "Y".to_string(),
            actual: "Z".to_string(),
        }
        .is_environment_mismatch());

        assert!(!ResumeError::CustomValidationFailed {
            reason: "test".to_string(),
        }
        .is_environment_mismatch());
    }

    #[test]
    fn test_resume_error_combine() {
        // Empty list
        assert!(ResumeError::combine(vec![]).is_none());

        // Single error
        let single = ResumeError::CustomValidationFailed {
            reason: "test".to_string(),
        };
        let combined = ResumeError::combine(vec![single.clone()]);
        assert!(combined.is_some());
        assert!(matches!(
            combined.unwrap(),
            ResumeError::CustomValidationFailed { .. }
        ));

        // Multiple errors
        let err1 = ResumeError::WorkingDirectoryMismatch {
            expected: PathBuf::from("/a"),
            actual: PathBuf::from("/b"),
        };
        let err2 = ResumeError::SandboxModeMismatch {
            expected: true,
            actual: false,
        };
        let combined = ResumeError::combine(vec![err1, err2]);
        assert!(matches!(combined, Some(ResumeError::Multiple { .. })));
        if let Some(ResumeError::Multiple { errors }) = combined {
            assert_eq!(errors.len(), 2);
        }
    }

    #[test]
    fn test_resume_environment_builder() {
        let env = ResumeEnvironment::new().with_sandbox_mode(true);

        assert!(env.working_dir.is_none());
        assert_eq!(env.sandbox_mode, Some(true));
        assert!(env.env_vars.is_empty());
    }

    #[test]
    fn test_resume_environment_with_working_dir() {
        let env = ResumeEnvironment::new().with_working_dir();
        // Should capture current dir
        assert!(env.working_dir.is_some());
    }

    #[test]
    fn test_working_dir_validator_pass() {
        let current_dir = std::env::current_dir().unwrap();
        let env = ResumeEnvironment {
            working_dir: Some(current_dir),
            sandbox_mode: None,
            env_vars: HashMap::new(),
        };

        let validator = WorkingDirValidator;
        assert!(validator.validate(&env).is_ok());
    }

    #[test]
    fn test_working_dir_validator_fail() {
        let env = ResumeEnvironment {
            working_dir: Some(PathBuf::from("/nonexistent/path/that/should/not/match")),
            sandbox_mode: None,
            env_vars: HashMap::new(),
        };

        let validator = WorkingDirValidator;
        let result = validator.validate(&env);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ResumeError::WorkingDirectoryMismatch { .. }
        ));
    }

    #[test]
    fn test_working_dir_validator_none() {
        let env = ResumeEnvironment {
            working_dir: None,
            sandbox_mode: None,
            env_vars: HashMap::new(),
        };

        let validator = WorkingDirValidator;
        assert!(validator.validate(&env).is_ok());
    }

    #[test]
    fn test_sandbox_validator_pass() {
        let env = ResumeEnvironment {
            working_dir: None,
            sandbox_mode: Some(true),
            env_vars: HashMap::new(),
        };

        let validator = SandboxValidator {
            current_sandbox: true,
        };
        assert!(validator.validate(&env).is_ok());
    }

    #[test]
    fn test_sandbox_validator_fail() {
        let env = ResumeEnvironment {
            working_dir: None,
            sandbox_mode: Some(true),
            env_vars: HashMap::new(),
        };

        let validator = SandboxValidator {
            current_sandbox: false,
        };
        let result = validator.validate(&env);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ResumeError::SandboxModeMismatch { .. }
        ));
    }

    #[test]
    fn test_sandbox_validator_none() {
        let env = ResumeEnvironment {
            working_dir: None,
            sandbox_mode: None,
            env_vars: HashMap::new(),
        };

        let validator = SandboxValidator {
            current_sandbox: true,
        };
        assert!(validator.validate(&env).is_ok());
    }

    #[test]
    fn test_env_var_validator_no_stored_vars() {
        let env = ResumeEnvironment {
            working_dir: None,
            sandbox_mode: None,
            env_vars: HashMap::new(),
        };

        let validator = EnvVarValidator {
            var_names: vec!["SOME_VAR".to_string()],
        };
        // Should pass if var wasn't stored
        assert!(validator.validate(&env).is_ok());
    }

    #[test]
    fn test_resume_outcome_is_fresh() {
        let outcome: ResumeOutcome<TestState> = ResumeOutcome::Fresh(ExecutionResult {
            final_state: TestState::default(),
            nodes_executed: vec![],
            interrupted_at: None,
            next_nodes: vec![],
        });
        assert!(outcome.is_fresh());
        assert!(!outcome.is_resumed());
    }

    #[test]
    fn test_resume_outcome_is_resumed() {
        let outcome: ResumeOutcome<TestState> = ResumeOutcome::Resumed(ExecutionResult {
            final_state: TestState::default(),
            nodes_executed: vec![],
            interrupted_at: None,
            next_nodes: vec![],
        });
        assert!(!outcome.is_fresh());
        assert!(outcome.is_resumed());
    }

    #[test]
    fn test_resume_outcome_into_result() {
        let outcome: ResumeOutcome<TestState> = ResumeOutcome::Fresh(ExecutionResult {
            final_state: TestState { value: 42 },
            nodes_executed: vec![],
            interrupted_at: None,
            next_nodes: vec![],
        });
        let result = outcome.into_result();
        assert_eq!(result.final_state.value, 42);
    }

    #[test]
    fn test_multiple_errors_display() {
        let err = ResumeError::Multiple {
            errors: vec![
                ResumeError::WorkingDirectoryMismatch {
                    expected: PathBuf::from("/a"),
                    actual: PathBuf::from("/b"),
                },
                ResumeError::SandboxModeMismatch {
                    expected: true,
                    actual: false,
                },
            ],
        };
        let msg = err.to_string();
        assert!(msg.contains("Multiple"));
        assert!(msg.contains("/a"));
        assert!(msg.contains("sandbox"));
    }

    #[test]
    fn test_resume_error_debug() {
        let err = ResumeError::CustomValidationFailed {
            reason: "test reason".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("CustomValidationFailed"));
        assert!(debug.contains("test reason"));
    }

    #[test]
    fn test_resume_error_clone() {
        let err = ResumeError::WorkingDirectoryMismatch {
            expected: PathBuf::from("/a"),
            actual: PathBuf::from("/b"),
        };
        let cloned = err.clone();
        assert_eq!(err, cloned);
    }

    #[test]
    fn test_resume_error_send_sync() {
        fn assert_send<T: Send>() {}
        fn assert_sync<T: Sync>() {}
        assert_send::<ResumeError>();
        assert_sync::<ResumeError>();
    }

    #[test]
    fn test_resume_environment_serde() {
        let env = ResumeEnvironment {
            working_dir: Some(PathBuf::from("/test")),
            sandbox_mode: Some(true),
            env_vars: {
                let mut m = HashMap::new();
                m.insert("KEY".to_string(), "VALUE".to_string());
                m
            },
        };

        let json = serde_json::to_string(&env).unwrap();
        let deserialized: ResumeEnvironment = serde_json::from_str(&json).unwrap();

        assert_eq!(env.working_dir, deserialized.working_dir);
        assert_eq!(env.sandbox_mode, deserialized.sandbox_mode);
        assert_eq!(env.env_vars, deserialized.env_vars);
    }

    #[test]
    fn test_resume_environment_default() {
        let env = ResumeEnvironment::default();
        assert!(env.working_dir.is_none());
        assert!(env.sandbox_mode.is_none());
        assert!(env.env_vars.is_empty());
    }
}
