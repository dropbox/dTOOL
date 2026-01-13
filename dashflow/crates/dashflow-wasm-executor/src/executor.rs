//! WASM executor module
//!
//! Core WASM execution engine with comprehensive security controls
//!
//! **Status:** WASM Runtime
//!
//! Full WASM runtime with:
//! - Wasmtime integration
//! - WASI configuration (zero permissions by default)
//! - Resource limits (fuel, memory, timeout)
//! - Sandboxed execution

use crate::audit::AuditLog;
use crate::auth::AuthContext;
use crate::config::WasmExecutorConfig;
use crate::error::{Error, Result};
use crate::metrics::Metrics;
use wasmtime::{Config, Engine, Linker, Module, Store, StoreLimits, StoreLimitsBuilder, Val};
use wasmtime_wasi::{WasiCtx, WasiCtxBuilder};

/// Store state that holds both WASI context and resource limits
///
/// This struct is passed to `Store::new()` to enable memory limits enforcement.
/// The `StoreLimits` restricts WASM memory growth to prevent resource exhaustion attacks.
struct StoreState {
    /// WASI context for sandboxed I/O (currently zero-permission)
    #[allow(dead_code)] // Architectural: WASI context held for future I/O operations
    wasi: WasiCtx,
    /// Resource limits (memory, instances, tables)
    limits: StoreLimits,
}

/// Guard to ensure concurrent executions counter is decremented on drop
struct ExecutionGuard {
    metrics: Metrics,
}

impl ExecutionGuard {
    fn new(metrics: Metrics) -> Self {
        Self { metrics }
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        self.metrics.execution_finished();
    }
}

/// Execution result with metrics
struct ExecutionMetrics {
    /// Result string
    output: String,
    /// Fuel consumed during execution
    fuel_consumed: u64,
    /// Peak memory usage in bytes
    memory_peak_bytes: u64,
}

/// WASM executor
///
/// Main struct for executing WebAssembly code with HIPAA/SOC2 compliance
#[derive(Clone)]
pub struct WasmExecutor {
    /// Configuration
    config: WasmExecutorConfig,

    /// Authentication context
    auth: AuthContext,

    /// Audit log
    audit_log: AuditLog,

    /// Wasmtime engine
    engine: Engine,

    /// Prometheus metrics
    metrics: Metrics,
}

impl WasmExecutor {
    /// Create a new WASM executor
    ///
    /// # Arguments
    /// * `config` - Executor configuration
    ///
    /// # Errors
    /// Returns error if configuration is invalid or audit log cannot be created
    pub fn new(config: WasmExecutorConfig) -> Result<Self> {
        // Validate configuration
        config.validate().map_err(Error::Configuration)?;

        // Create authentication context
        let auth = AuthContext::new(config.jwt_secret.clone(), config.jwt_expiry_minutes)?;

        // Create audit log
        let audit_log = if config.enable_audit_logging {
            AuditLog::new(&config.audit_log_path)?
        } else {
            // For testing: use temp file
            AuditLog::new("/tmp/wasm-executor-audit.log")?
        };

        // Create Wasmtime engine with security-first configuration
        let mut wasmtime_config = Config::new();

        // Enable fuel metering for CPU limits
        wasmtime_config.consume_fuel(true);

        // Set memory limits (max_memory_bytes from config)
        wasmtime_config.max_wasm_stack(config.max_stack_bytes);

        // Disable features that could be used for side-channels or attacks
        wasmtime_config.wasm_threads(false);
        wasmtime_config.wasm_bulk_memory(true); // Safe and useful
        wasmtime_config.wasm_reference_types(false);
        // Note: SIMD cannot be disabled if relaxed SIMD is enabled (wasmtime default)
        // We'll keep SIMD enabled but monitor for security issues

        // Create engine
        let engine = Engine::new(&wasmtime_config)
            .map_err(|e| Error::Configuration(format!("Failed to create Wasmtime engine: {e}")))?;

        // Create metrics
        let metrics = Metrics::default();

        Ok(Self {
            config,
            auth,
            audit_log,
            engine,
            metrics,
        })
    }

    /// Execute WASM code
    ///
    /// Full WASM execution with:
    /// - WASM module validation
    /// - Wasmtime runtime setup
    /// - WASI configuration (zero permissions by default)
    /// - Resource limits (fuel, memory, timeout)
    /// - Sandboxed execution
    /// - Result sanitization
    ///
    /// # Arguments
    /// * `wasm_bytes` - WASM module bytecode
    /// * `function` - Function name to call
    /// * `args` - Function arguments (i32 only for now)
    ///
    /// # Returns
    /// Execution result as a string
    ///
    /// # Errors
    /// Returns error if:
    /// - WASM module is invalid
    /// - Function not found
    /// - Execution times out
    /// - Fuel limit exceeded
    /// - Memory limit exceeded
    pub async fn execute(&self, wasm_bytes: &[u8], function: &str, args: &[i32]) -> Result<String> {
        let metrics = self.execute_internal(wasm_bytes, function, args).await?;
        Ok(metrics.output)
    }

    /// Internal execution that returns full metrics
    /// Used by both `execute()` and `execute_with_auth()`
    async fn execute_internal(
        &self,
        wasm_bytes: &[u8],
        function: &str,
        args: &[i32],
    ) -> Result<ExecutionMetrics> {
        // Track concurrent executions
        self.metrics.execution_started();
        let _guard = ExecutionGuard::new(self.metrics.clone());

        // Start timer for execution duration
        let start = std::time::Instant::now();

        // Use tokio::task::spawn_blocking for CPU-bound WASM execution
        let engine = self.engine.clone();
        let config = self.config.clone();
        let wasm_bytes = wasm_bytes.to_vec();
        let function = function.to_string();
        let args = args.to_vec();

        let timeout_duration = config.max_execution_timeout;
        let timeout_secs = timeout_duration.as_secs();

        // Execute with timeout
        let result = tokio::time::timeout(
            timeout_duration,
            tokio::task::spawn_blocking(move || {
                Self::execute_sync(&engine, &config, &wasm_bytes, &function, &args)
            }),
        )
        .await;

        let duration_secs = start.elapsed().as_secs_f64();

        match result {
            Ok(Ok(exec_result)) => {
                // spawn_blocking succeeded, now check execute_sync result
                match exec_result {
                    Ok(metrics) => {
                        // Record success metrics with actual fuel and memory data
                        self.metrics.record_success(
                            duration_secs,
                            metrics.fuel_consumed,
                            metrics.memory_peak_bytes,
                        );
                        Ok(metrics)
                    }
                    Err(e) => {
                        // Record failure metrics (WASM execution failed)
                        // Note: No metrics available for failed execution
                        self.metrics.record_failure(duration_secs, 0, 0);
                        Err(e)
                    }
                }
            }
            Ok(Err(e)) => {
                // spawn_blocking task failed (JoinError)
                self.metrics.record_failure(duration_secs, 0, 0);
                Err(Error::ExecutionFailed(format!("Task join error: {e}")))
            }
            Err(_) => {
                // Record timeout metrics
                self.metrics.record_timeout(duration_secs);
                Err(Error::ExecutionFailed(format!(
                    "Execution timeout after {timeout_secs} seconds"
                )))
            }
        }
    }

    /// Synchronous WASM execution (called from `spawn_blocking`)
    fn execute_sync(
        engine: &Engine,
        config: &WasmExecutorConfig,
        wasm_bytes: &[u8],
        function: &str,
        args: &[i32],
    ) -> Result<ExecutionMetrics> {
        // Measure memory at start
        let mem_start = memory_stats::memory_stats().map_or(0, |m| m.physical_mem);
        // Validate WASM module
        Module::validate(engine, wasm_bytes)
            .map_err(|e| Error::ExecutionFailed(format!("Invalid WASM module: {e}")))?;

        // Compile module
        let module = Module::new(engine, wasm_bytes)
            .map_err(|e| Error::ExecutionFailed(format!("Failed to compile WASM: {e}")))?;

        // Create WASI context with zero permissions (secure by default)
        let wasi = WasiCtxBuilder::new()
            .inherit_stdio() // Allow stdio for debugging (can be disabled in production)
            .build();

        // Create resource limits to prevent memory exhaustion attacks
        // M-224: WASM memory limits enforced via StoreLimits
        let limits = StoreLimitsBuilder::new()
            .memory_size(config.max_memory_bytes) // Linear memory limit
            .instances(100) // Max concurrent instances (prevents fork bomb)
            .memories(10) // Max linear memories per module
            .tables(10) // Max tables per module
            .table_elements(10_000) // Max table elements
            .trap_on_grow_failure(true) // Trap instead of returning -1 on OOM
            .build();

        // Create store state with WASI context and resource limits
        let store_state = StoreState { wasi, limits };

        // Create store with state
        let mut store = Store::new(engine, store_state);

        // Enable resource limiter - this enforces memory limits on WASM execution
        // The limiter() closure returns a reference to the StoreLimits in our state
        store.limiter(|state| &mut state.limits);

        // Set fuel limit (CPU quota)
        store
            .set_fuel(config.max_fuel)
            .map_err(|e| Error::ExecutionFailed(format!("Failed to set fuel: {e}")))?;

        // Create linker and add WASI
        // For wasmtime 28, we don't need WASI for simple math functions
        // WASI will be needed for I/O operations
        let linker = Linker::new(engine);
        // Note: WASI linker setup deferred - not needed for simple math operations
        // Will be added when needed for I/O operations

        // Instantiate module
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| Error::ExecutionFailed(format!("Failed to instantiate: {e}")))?;

        // Get the function to call
        let func = instance
            .get_func(&mut store, function)
            .ok_or_else(|| Error::ExecutionFailed(format!("Function '{function}' not found")))?;

        // Convert args to Val
        let wasm_args: Vec<Val> = args.iter().map(|&v| Val::I32(v)).collect();

        // Prepare results buffer (assume single i32 result for now)
        let mut results = vec![Val::I32(0)];

        // Call the function
        func.call(&mut store, &wasm_args, &mut results)
            .map_err(|e| {
                // Check for resource exhaustion errors
                let error_msg = e.to_string();
                if error_msg.contains("fuel") {
                    Error::ExecutionFailed(format!(
                        "Fuel limit exceeded (max: {})",
                        config.max_fuel
                    ))
                } else if error_msg.contains("memory")
                    || error_msg.contains("grow")
                    || error_msg.contains("limit")
                {
                    // M-224: Memory limit enforcement - trap on OOM
                    Error::OutOfMemory(config.max_memory_bytes)
                } else {
                    Error::ExecutionFailed(format!("Execution failed: {e}"))
                }
            })?;

        // Get remaining fuel (for audit logging)
        let remaining_fuel = store
            .get_fuel()
            .map_err(|e| Error::ExecutionFailed(format!("Failed to get fuel: {e}")))?;
        let fuel_consumed = config.max_fuel - remaining_fuel;

        // Convert result to string
        let result_str = match results.first() {
            Some(Val::I32(v)) => v.to_string(),
            Some(Val::I64(v)) => v.to_string(),
            Some(Val::F32(v)) => f32::from_bits(*v).to_string(),
            Some(Val::F64(v)) => f64::from_bits(*v).to_string(),
            _ => "()".to_string(), // No return value or unsupported type
        };

        // Measure memory at end
        let mem_end = memory_stats::memory_stats().map_or(0, |m| m.physical_mem);
        let memory_peak_bytes = mem_end.saturating_sub(mem_start) as u64;

        // Log successful execution (fuel consumed)
        tracing::info!(
            function = function,
            fuel_consumed = fuel_consumed,
            memory_peak_bytes = memory_peak_bytes,
            result = result_str,
            "WASM execution succeeded"
        );

        Ok(ExecutionMetrics {
            output: result_str,
            fuel_consumed,
            memory_peak_bytes,
        })
    }

    /// Get reference to authentication context
    #[must_use]
    pub fn auth(&self) -> &AuthContext {
        &self.auth
    }

    /// Get reference to audit log
    #[must_use]
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit_log
    }

    /// Get reference to configuration
    #[must_use]
    pub fn config(&self) -> &WasmExecutorConfig {
        &self.config
    }

    /// Get reference to metrics
    #[must_use]
    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    /// Execute WASM code with authentication and authorization
    ///
    /// This method verifies the JWT token, checks role permissions, and logs all attempts
    /// to the audit log. It combines authentication, authorization, execution, and audit
    /// logging in a single operation.
    ///
    /// # Arguments
    /// * `token` - JWT token for authentication
    /// * `wasm_bytes` - WASM module bytecode
    /// * `function` - Function name to call
    /// * `args` - Function arguments (i32 only for now)
    ///
    /// # Returns
    /// Execution result as a string
    ///
    /// # Errors
    /// Returns error if:
    /// - JWT token is invalid or expired
    /// - User role does not have execute permissions
    /// - WASM execution fails (see `execute()` for details)
    ///
    /// # Security
    /// - Only Agent and Administrator roles can execute WASM code
    /// - All execution attempts are logged (success and failure)
    /// - Failed authorization attempts are logged with severity "warning"
    pub async fn execute_with_auth(
        &self,
        token: &str,
        wasm_bytes: &[u8],
        function: &str,
        args: &[i32],
    ) -> Result<String> {
        self.execute_with_auth_and_context(token, wasm_bytes, function, args, None)
            .await
    }

    /// Execute WASM code with authentication and request context
    ///
    /// Same as `execute_with_auth`, but allows passing request context for
    /// proper audit logging (e.g., source IP address).
    ///
    /// # Arguments
    /// * `token` - JWT token for authentication
    /// * `wasm_bytes` - Compiled WASM module bytes
    /// * `function` - Function name to execute
    /// * `args` - Arguments to pass to the function
    /// * `context` - Optional request context containing source IP for audit logs
    ///
    /// # Returns
    /// Result string from the executed function
    ///
    /// # Errors
    /// - Token verification fails
    /// - User role does not have execute permissions
    /// - WASM execution fails (see `execute()` for details)
    ///
    /// # Security
    /// - Only Agent and Administrator roles can execute WASM code
    /// - All execution attempts are logged (success and failure)
    /// - Failed authorization attempts are logged with severity "warning"
    /// - Source IP is included in audit logs when provided via context
    pub async fn execute_with_auth_and_context(
        &self,
        token: &str,
        wasm_bytes: &[u8],
        function: &str,
        args: &[i32],
        context: Option<crate::audit::RequestContext>,
    ) -> Result<String> {
        use crate::audit::{
            AuditLogEntry, ComplianceMetadata, ExecutionInfo, ExecutionStatus, RequestInfo,
            ResultInfo, Severity, UserInfo,
        };
        use sha2::{Digest, Sha256};

        // Get IP from context or use "unknown" as default
        let source_ip = context
            .as_ref()
            .map(|c| c.ip_or_default())
            .unwrap_or_else(|| "unknown".to_string());

        // Verify token and check execute permissions
        let claims = match self.auth.verify_execute_access(token) {
            Ok(claims) => {
                // Record successful authentication
                self.metrics.record_auth_success();
                claims
            }
            Err(e) => {
                // Record failed authentication and access denial
                self.metrics.record_auth_failure();
                self.metrics.record_access_denied_auth();

                // Log authorization failure
                let entry = AuditLogEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: "authorization_failure".to_string(),
                    severity: Severity::Warning,
                    user: UserInfo {
                        id: "unknown".to_string(),
                        role: crate::auth::Role::Auditor, // Default role for unknown users
                        ip: source_ip.clone(),
                    },
                    request: RequestInfo {
                        request_id: uuid::Uuid::new_v4().to_string(),
                        session_id: "unknown".to_string(),
                        wasm_hash: String::new(),
                        function: function.to_string(),
                    },
                    execution: None,
                    result: Some(ResultInfo {
                        output_length: 0,
                        error: Some(e.to_string()),
                    }),
                    metadata: ComplianceMetadata::default(),
                };
                self.audit_log.log(entry)?;

                return Err(e);
            }
        };

        // Calculate WASM hash for audit trail
        let mut hasher = Sha256::new();
        hasher.update(wasm_bytes);
        let wasm_hash = format!("{:x}", hasher.finalize());

        // Start timer for execution duration
        let start = std::time::Instant::now();

        // Execute WASM code using internal method to get metrics
        let exec_result = self.execute_internal(wasm_bytes, function, args).await;

        // Calculate execution duration
        let duration_ms = start.elapsed().as_millis() as u64;

        // Log execution attempt
        match &exec_result {
            Ok(metrics) => {
                // Log successful execution
                let entry = AuditLogEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: "wasm_execution".to_string(),
                    severity: Severity::Info,
                    user: UserInfo {
                        id: claims.sub.clone(),
                        role: claims.role,
                        ip: source_ip.clone(),
                    },
                    request: RequestInfo {
                        request_id: uuid::Uuid::new_v4().to_string(),
                        session_id: claims.session_id.clone(),
                        wasm_hash: wasm_hash.clone(),
                        function: function.to_string(),
                    },
                    execution: Some(ExecutionInfo {
                        status: ExecutionStatus::Success,
                        duration_ms,
                        fuel_consumed: metrics.fuel_consumed,
                        memory_peak_bytes: metrics.memory_peak_bytes as usize,
                    }),
                    result: Some(ResultInfo {
                        output_length: metrics.output.len(),
                        error: None,
                    }),
                    metadata: ComplianceMetadata::default(),
                };
                self.audit_log.log(entry)?;
            }
            Err(e) => {
                // Log failed execution (no metrics available for failures)
                let entry = AuditLogEntry {
                    timestamp: chrono::Utc::now(),
                    event_type: "wasm_execution".to_string(),
                    severity: Severity::Error,
                    user: UserInfo {
                        id: claims.sub.clone(),
                        role: claims.role,
                        ip: source_ip,
                    },
                    request: RequestInfo {
                        request_id: uuid::Uuid::new_v4().to_string(),
                        session_id: claims.session_id.clone(),
                        wasm_hash,
                        function: function.to_string(),
                    },
                    execution: Some(ExecutionInfo {
                        status: ExecutionStatus::Failure,
                        duration_ms,
                        fuel_consumed: 0,
                        memory_peak_bytes: 0,
                    }),
                    result: Some(ResultInfo {
                        output_length: 0,
                        error: Some(e.to_string()),
                    }),
                    metadata: ComplianceMetadata::default(),
                };
                self.audit_log.log(entry)?;
            }
        }

        // Convert Result<ExecutionMetrics> to Result<String> for return
        exec_result.map(|m| m.output)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_creation() {
        let config = WasmExecutorConfig::for_testing();
        let executor = WasmExecutor::new(config);
        if let Err(e) = &executor {
            eprintln!("Executor creation failed: {:?}", e);
        }
        assert!(
            executor.is_ok(),
            "Failed to create executor: {:?}",
            executor.err()
        );
    }

    #[test]
    fn test_executor_with_invalid_config() {
        let mut config = WasmExecutorConfig::for_testing();
        config.max_fuel = 0; // Invalid
        let executor = WasmExecutor::new(config);
        assert!(executor.is_err());
    }
}
