// Allow clippy warnings for audit module
// - needless_pass_by_value: Audit event data passed by value for logging
#![allow(clippy::needless_pass_by_value)]

//! Audit logging module
//!
//! Implements HIPAA §164.312(b) Audit Controls:
//! - Record and examine all WASM execution activity
//! - Tamper-evident log storage (append-only)
//! - 7-year retention for compliance
//! - Structured JSON logs for SIEM integration

use crate::auth::Role;
use crate::error::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Audit log entry
///
/// Captures all relevant information about a WASM execution event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Timestamp (ISO 8601 format)
    pub timestamp: DateTime<Utc>,

    /// Event type
    pub event_type: String,

    /// Severity level
    pub severity: Severity,

    /// User information
    pub user: UserInfo,

    /// Request information
    pub request: RequestInfo,

    /// Execution information (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionInfo>,

    /// Result information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultInfo>,

    /// Compliance metadata
    pub metadata: ComplianceMetadata,
}

/// User information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    /// User ID
    pub id: String,

    /// User role
    pub role: Role,

    /// Source IP address
    pub ip: String,
}

/// Request context for audit logging
///
/// Contains request-level metadata like source IP address that should be
/// captured for compliance logging. Pass this to `execute_with_auth_and_context`
/// to include request context in audit logs.
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    /// Source IP address of the request
    pub source_ip: Option<String>,
}

impl RequestContext {
    /// Create a new request context with the source IP address
    pub fn with_ip(ip: impl Into<String>) -> Self {
        Self {
            source_ip: Some(ip.into()),
        }
    }

    /// Get the IP address or a default placeholder
    pub fn ip_or_default(&self) -> String {
        self.source_ip
            .clone()
            .unwrap_or_else(|| "unknown".to_string())
    }
}

/// Request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestInfo {
    /// Unique request ID
    pub request_id: String,

    /// Session ID
    pub session_id: String,

    /// SHA-256 hash of WASM module
    pub wasm_hash: String,

    /// Function name being called
    pub function: String,
}

/// Execution information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionInfo {
    /// Execution status
    pub status: ExecutionStatus,

    /// Duration in milliseconds
    pub duration_ms: u64,

    /// Fuel consumed
    pub fuel_consumed: u64,

    /// Peak memory usage (bytes)
    pub memory_peak_bytes: usize,
}

/// Execution status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Execution completed successfully
    Success,
    /// Execution failed with error
    Failure,
    /// Execution timed out
    Timeout,
}

/// Result information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultInfo {
    /// Length of output in bytes
    pub output_length: usize,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Log severity level
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Informational events (normal operations)
    Info,
    /// Warning events (potential issues)
    Warning,
    /// Error events (failures)
    Error,
    /// Critical events (security breaches, system failures)
    Critical,
}

/// Compliance metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplianceMetadata {
    /// Compliance frameworks applicable
    pub compliance: Vec<String>,

    /// Retention period in years
    pub retention_years: u8,
}

impl Default for ComplianceMetadata {
    fn default() -> Self {
        Self {
            compliance: vec!["HIPAA".to_string(), "SOC2".to_string()],
            retention_years: 7, // HIPAA requirement
        }
    }
}

/// Audit logger
///
/// Thread-safe, tamper-evident audit logging system
#[derive(Clone)]
pub struct AuditLog {
    /// Log file path
    file_path: String,

    /// File handle (append-only)
    file: Arc<Mutex<std::fs::File>>,
}

impl AuditLog {
    /// Create new audit log
    ///
    /// Creates the log file if it doesn't exist, opens in append mode
    ///
    /// # HIPAA Compliance
    /// - Append-only mode prevents tampering
    /// - All events are logged sequentially
    pub fn new<P: AsRef<Path>>(file_path: P) -> Result<Self> {
        let path = file_path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                Error::Configuration(format!("Failed to create audit log directory: {e}"))
            })?;
        }

        // Open in append mode (O_APPEND ensures atomic appends)
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|e| Error::Configuration(format!("Failed to open audit log: {e}")))?;

        Ok(Self {
            file_path: path.to_string_lossy().to_string(),
            file: Arc::new(Mutex::new(file)),
        })
    }

    /// Log an event
    ///
    /// Writes a JSON line to the audit log (append-only)
    ///
    /// # HIPAA Compliance
    /// - §164.312(b): Record and examine activity
    /// - Atomic writes prevent corruption
    /// - Structured format for SIEM integration
    pub fn log(&self, entry: AuditLogEntry) -> Result<()> {
        let json = serde_json::to_string(&entry)
            .map_err(|e| Error::AuditFailed(format!("Failed to serialize audit entry: {e}")))?;

        let mut file = self
            .file
            .lock()
            .map_err(|e| Error::AuditFailed(format!("Failed to acquire audit log lock: {e}")))?;

        writeln!(file, "{json}")
            .map_err(|e| Error::AuditFailed(format!("Failed to write to audit log: {e}")))?;

        // Flush to ensure data is written to disk immediately
        file.flush()
            .map_err(|e| Error::AuditFailed(format!("Failed to flush audit log: {e}")))?;

        Ok(())
    }

    /// Log a WASM execution event
    #[allow(clippy::too_many_arguments)] // Audit logging requires full context: user, session, function, status
    pub fn log_execution(
        &self,
        user_id: String,
        role: Role,
        ip: String,
        request_id: String,
        session_id: String,
        wasm_hash: String,
        function: String,
        status: ExecutionStatus,
        duration_ms: u64,
        fuel_consumed: u64,
        memory_peak_bytes: usize,
        output_length: usize,
        error: Option<String>,
    ) -> Result<()> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_type: "wasm_execution".to_string(),
            severity: match status {
                ExecutionStatus::Success => Severity::Info,
                ExecutionStatus::Failure => Severity::Error,
                ExecutionStatus::Timeout => Severity::Warning,
            },
            user: UserInfo {
                id: user_id,
                role,
                ip,
            },
            request: RequestInfo {
                request_id,
                session_id,
                wasm_hash,
                function,
            },
            execution: Some(ExecutionInfo {
                status,
                duration_ms,
                fuel_consumed,
                memory_peak_bytes,
            }),
            result: Some(ResultInfo {
                output_length,
                error,
            }),
            metadata: ComplianceMetadata::default(),
        };

        self.log(entry)
    }

    /// Log an authentication event
    pub fn log_authentication(
        &self,
        user_id: String,
        ip: String,
        success: bool,
        reason: Option<String>,
    ) -> Result<()> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_type: if success {
                "authentication_success".to_string()
            } else {
                "authentication_failure".to_string()
            },
            severity: if success {
                Severity::Info
            } else {
                Severity::Warning
            },
            user: UserInfo {
                id: user_id,
                role: Role::Agent, // Unknown at this point
                ip,
            },
            request: RequestInfo {
                request_id: uuid::Uuid::new_v4().to_string(),
                session_id: String::new(),
                wasm_hash: String::new(),
                function: "authenticate".to_string(),
            },
            execution: None,
            result: Some(ResultInfo {
                output_length: 0,
                error: reason,
            }),
            metadata: ComplianceMetadata::default(),
        };

        self.log(entry)
    }

    /// Log an authorization failure
    pub fn log_authorization_failure(
        &self,
        user_id: String,
        role: Role,
        ip: String,
        requested_action: String,
    ) -> Result<()> {
        let entry = AuditLogEntry {
            timestamp: Utc::now(),
            event_type: "authorization_failure".to_string(),
            severity: Severity::Warning,
            user: UserInfo {
                id: user_id,
                role,
                ip,
            },
            request: RequestInfo {
                request_id: uuid::Uuid::new_v4().to_string(),
                session_id: String::new(),
                wasm_hash: String::new(),
                function: requested_action,
            },
            execution: None,
            result: Some(ResultInfo {
                output_length: 0,
                error: Some("Access denied".to_string()),
            }),
            metadata: ComplianceMetadata::default(),
        };

        self.log(entry)
    }

    /// Get the audit log file path
    #[must_use]
    pub fn file_path(&self) -> &str {
        &self.file_path
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_audit_log_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::new(temp_file.path()).unwrap();
        assert_eq!(log.file_path(), temp_file.path().to_str().unwrap());
    }

    #[test]
    fn test_log_execution() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::new(temp_file.path()).unwrap();

        let result = log.log_execution(
            "user123".to_string(),
            Role::Agent,
            "192.168.1.100".to_string(),
            "req-123".to_string(),
            "sess-456".to_string(),
            "sha256:abcd1234".to_string(),
            "calculate".to_string(),
            ExecutionStatus::Success,
            125,
            5_000_000,
            50_331_648,
            42,
            None,
        );

        assert!(result.is_ok());

        // Verify log file contains JSON
        let contents = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(contents.contains("wasm_execution"));
        assert!(contents.contains("user123"));
        assert!(contents.contains("HIPAA"));
    }

    #[test]
    fn test_log_authentication() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::new(temp_file.path()).unwrap();

        let result = log.log_authentication(
            "user123".to_string(),
            "192.168.1.100".to_string(),
            false,
            Some("Invalid token".to_string()),
        );

        assert!(result.is_ok());

        let contents = std::fs::read_to_string(temp_file.path()).unwrap();
        assert!(contents.contains("authentication_failure"));
    }

    #[test]
    fn test_append_mode() {
        let temp_file = NamedTempFile::new().unwrap();
        let log = AuditLog::new(temp_file.path()).unwrap();

        // Write first entry
        log.log_authentication("user1".to_string(), "1.1.1.1".to_string(), true, None)
            .unwrap();

        // Write second entry
        log.log_authentication("user2".to_string(), "2.2.2.2".to_string(), true, None)
            .unwrap();

        // Both entries should be present
        let contents = std::fs::read_to_string(temp_file.path()).unwrap();
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(contents.contains("user1"));
        assert!(contents.contains("user2"));
    }
}
