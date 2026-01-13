// M-962: Moved clippy allows from module-level to #[cfg(test)] only (see tests module)

//! Audit Logging for Self-Improvement
//!
//! This module provides an immutable audit log for plan approvals, implementations,
//! and other significant actions in the self-improvement system. The audit log
//! ensures accountability and traceability for AI-driven changes.
//!
//! ## Audit Events
//!
//! The following events are logged:
//! - Plan creation, approval, rejection, and implementation
//! - Hypothesis creation and evaluation
//! - Configuration changes to the self-improvement system
//! - Manual overrides and administrative actions
//!
//! ## Storage
//!
//! Audit logs are stored in `.dashflow/introspection/audit/` as append-only
//! JSON Lines files (one entry per line), making them tamper-evident and
//! easy to stream/parse.
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{AuditLog, AuditEvent, AuditAction};
//!
//! let audit = AuditLog::default();
//! audit.log(&AuditEvent::new(
//!     AuditAction::PlanApproved,
//!     "plan-123",
//! ).with_actor("user@example.com")
//!  .with_reason("Passed all validation checks"));
//!
//! // Query audit history
//! let events = audit.query()
//!     .action(AuditAction::PlanApproved)
//!     .since(chrono::Utc::now() - chrono::Duration::days(7))
//!     .execute()?;
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;

/// Actions that can be audited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    // Plan lifecycle
    /// A new improvement plan was created.
    PlanCreated,
    /// A plan was approved for implementation.
    PlanApproved,
    /// A plan was rejected and will not be implemented.
    PlanRejected,
    /// Implementation of a plan has started.
    PlanImplementationStarted,
    /// A plan was successfully implemented.
    PlanImplementationCompleted,
    /// Implementation of a plan failed.
    PlanImplementationFailed,
    /// A plan was superseded by a better plan.
    PlanSuperseded,
    /// A plan was archived (no longer active).
    PlanArchived,

    // Hypothesis lifecycle
    /// A new hypothesis was created.
    HypothesisCreated,
    /// A hypothesis was activated for evaluation.
    HypothesisActivated,
    /// A hypothesis was evaluated.
    HypothesisEvaluated,
    /// A hypothesis was confirmed as correct.
    HypothesisConfirmed,
    /// A hypothesis was refuted as incorrect.
    HypothesisRefuted,
    /// A hypothesis was archived.
    HypothesisArchived,

    // Report lifecycle
    /// A report was generated.
    ReportGenerated,
    /// A report was archived.
    ReportArchived,

    // Configuration changes
    /// System configuration was changed.
    ConfigChanged,
    /// A security or operational policy was changed.
    PolicyChanged,

    // Administrative actions
    /// A manual override was performed.
    ManualOverride,
    /// The system was started.
    SystemStartup,
    /// The system was shut down.
    SystemShutdown,
    /// Storage cleanup was performed.
    StorageCleanup,
    /// An emergency stop was triggered.
    EmergencyStop,

    // Security events
    /// PII or sensitive data was redacted.
    RedactionApplied,
    /// Sensitive data was accessed (for audit trail).
    SensitiveDataAccessed,
    /// An unauthorized access attempt was detected.
    UnauthorizedAccessAttempt,
}

impl AuditAction {
    /// Get the severity level of this action
    #[must_use]
    pub fn severity(&self) -> AuditSeverity {
        match self {
            // High severity - requires attention
            Self::PlanApproved
            | Self::PlanImplementationCompleted
            | Self::ManualOverride
            | Self::EmergencyStop
            | Self::UnauthorizedAccessAttempt => AuditSeverity::High,

            // Medium severity - notable events
            Self::PlanCreated
            | Self::PlanRejected
            | Self::PlanImplementationStarted
            | Self::PlanImplementationFailed
            | Self::HypothesisConfirmed
            | Self::HypothesisRefuted
            | Self::ConfigChanged
            | Self::PolicyChanged
            | Self::SensitiveDataAccessed => AuditSeverity::Medium,

            // Low severity - routine operations
            Self::PlanSuperseded
            | Self::PlanArchived
            | Self::HypothesisCreated
            | Self::HypothesisActivated
            | Self::HypothesisEvaluated
            | Self::HypothesisArchived
            | Self::ReportGenerated
            | Self::ReportArchived
            | Self::SystemStartup
            | Self::SystemShutdown
            | Self::StorageCleanup
            | Self::RedactionApplied => AuditSeverity::Low,
        }
    }
}

/// Severity levels for audit events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    /// Routine operations that don't require attention.
    Low,
    /// Notable events that may warrant review.
    Medium,
    /// Significant events requiring attention.
    High,
}

/// An audit event record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Unique event ID
    pub id: String,

    /// Timestamp of the event (UTC)
    pub timestamp: DateTime<Utc>,

    /// The action that was performed
    pub action: AuditAction,

    /// ID of the resource affected (plan ID, hypothesis ID, etc.)
    pub resource_id: String,

    /// Type of resource (plan, hypothesis, report, config)
    #[serde(default)]
    pub resource_type: String,

    /// Who performed the action (user email, "system", "ai-agent")
    #[serde(default)]
    pub actor: String,

    /// Reason or justification for the action
    #[serde(default)]
    pub reason: Option<String>,

    /// Previous state (for changes)
    #[serde(default)]
    pub previous_state: Option<String>,

    /// New state (for changes)
    #[serde(default)]
    pub new_state: Option<String>,

    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Session or correlation ID for grouping related events
    #[serde(default)]
    pub session_id: Option<String>,

    /// IP address or source of the action
    #[serde(default)]
    pub source: Option<String>,
}

impl AuditEvent {
    /// Create a new audit event
    #[must_use]
    pub fn new(action: AuditAction, resource_id: impl Into<String>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            action,
            resource_id: resource_id.into(),
            resource_type: String::new(),
            actor: "system".to_string(),
            reason: None,
            previous_state: None,
            new_state: None,
            metadata: HashMap::new(),
            session_id: None,
            source: None,
        }
    }

    /// Set the resource type
    #[must_use]
    pub fn with_resource_type(mut self, resource_type: impl Into<String>) -> Self {
        self.resource_type = resource_type.into();
        self
    }

    /// Set the actor (who performed the action)
    #[must_use]
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = actor.into();
        self
    }

    /// Set the reason for the action
    #[must_use]
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Set the previous state
    #[must_use]
    pub fn with_previous_state(mut self, state: impl Into<String>) -> Self {
        self.previous_state = Some(state.into());
        self
    }

    /// Set the new state
    #[must_use]
    pub fn with_new_state(mut self, state: impl Into<String>) -> Self {
        self.new_state = Some(state.into());
        self
    }

    /// Add metadata
    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Set the session ID
    #[must_use]
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set the source
    #[must_use]
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Get the severity of this event
    #[must_use]
    pub fn severity(&self) -> AuditSeverity {
        self.action.severity()
    }

    /// Serialize to JSON line (for append-only storage)
    #[must_use]
    pub fn to_json_line(&self) -> String {
        // M-963: Log warning on serialization failure instead of silent "{}" return
        serde_json::to_string(self).unwrap_or_else(|e| {
            tracing::warn!(
                event_id = %self.id,
                error = %e,
                "Failed to serialize audit event to JSON, returning empty object"
            );
            "{}".to_string()
        })
    }

    /// Parse from JSON line
    ///
    /// # Errors
    ///
    /// Returns error if JSON parsing fails
    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

/// Audit log storage and query interface
pub struct AuditLog {
    /// Base directory for audit logs
    base_dir: PathBuf,

    /// Whether to sync after each write
    sync_on_write: bool,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(".dashflow/introspection/audit")
    }
}

impl AuditLog {
    /// Create a new audit log with the given base directory
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
            sync_on_write: true,
        }
    }

    /// Disable sync-on-write for better performance (less durability)
    #[must_use]
    pub fn without_sync(mut self) -> Self {
        self.sync_on_write = false;
        self
    }

    /// Initialize the audit log directory
    ///
    /// # Errors
    ///
    /// Returns error if directory creation fails
    pub fn initialize(&self) -> std::io::Result<()> {
        fs::create_dir_all(&self.base_dir)?;
        Ok(())
    }

    /// Get the current log file path (YYYY-MM-DD.jsonl)
    fn current_log_file(&self) -> PathBuf {
        let date = Utc::now().format("%Y-%m-%d").to_string();
        self.base_dir.join(format!("{date}.jsonl"))
    }

    /// Log an audit event
    ///
    /// # Errors
    ///
    /// Returns error if writing fails
    pub fn log(&self, event: &AuditEvent) -> std::io::Result<()> {
        self.initialize()?;

        let log_file = self.current_log_file();
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        let line = event.to_json_line();
        writeln!(file, "{line}")?;

        if self.sync_on_write {
            file.sync_all()?;
        }

        Ok(())
    }

    /// Create a query builder
    #[must_use]
    pub fn query(&self) -> AuditQueryBuilder {
        AuditQueryBuilder::new(self.base_dir.clone())
    }

    /// Get all events for a specific resource
    ///
    /// # Errors
    ///
    /// Returns error if reading fails
    pub fn events_for_resource(&self, resource_id: &str) -> std::io::Result<Vec<AuditEvent>> {
        self.query().resource_id(resource_id).execute()
    }

    /// Get recent events (last N)
    ///
    /// # Errors
    ///
    /// Returns error if reading fails
    pub fn recent_events(&self, count: usize) -> std::io::Result<Vec<AuditEvent>> {
        self.query().limit(count).execute()
    }

    /// Get events by action type
    ///
    /// # Errors
    ///
    /// Returns error if reading fails
    pub fn events_by_action(&self, action: AuditAction) -> std::io::Result<Vec<AuditEvent>> {
        self.query().action(action).execute()
    }

    /// List all log files
    ///
    /// # Errors
    ///
    /// Returns error if reading directory fails
    pub fn log_files(&self) -> std::io::Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if self.base_dir.exists() {
            for entry in fs::read_dir(&self.base_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "jsonl") {
                    files.push(path);
                }
            }
        }
        files.sort();
        Ok(files)
    }

    /// Get total event count across all log files
    ///
    /// # Errors
    ///
    /// Returns error if reading fails
    pub fn total_event_count(&self) -> std::io::Result<usize> {
        let mut count = 0;
        for file in self.log_files()? {
            let reader = BufReader::new(File::open(file)?);
            count += reader.lines().count();
        }
        Ok(count)
    }
}

/// Builder for audit log queries
pub struct AuditQueryBuilder {
    base_dir: PathBuf,
    action_filter: Option<AuditAction>,
    resource_id_filter: Option<String>,
    actor_filter: Option<String>,
    since_filter: Option<DateTime<Utc>>,
    until_filter: Option<DateTime<Utc>>,
    severity_filter: Option<AuditSeverity>,
    limit: Option<usize>,
}

impl AuditQueryBuilder {
    fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            action_filter: None,
            resource_id_filter: None,
            actor_filter: None,
            since_filter: None,
            until_filter: None,
            severity_filter: None,
            limit: None,
        }
    }

    /// Filter by action type
    #[must_use]
    pub fn action(mut self, action: AuditAction) -> Self {
        self.action_filter = Some(action);
        self
    }

    /// Filter by resource ID
    #[must_use]
    pub fn resource_id(mut self, id: impl Into<String>) -> Self {
        self.resource_id_filter = Some(id.into());
        self
    }

    /// Filter by actor
    #[must_use]
    pub fn actor(mut self, actor: impl Into<String>) -> Self {
        self.actor_filter = Some(actor.into());
        self
    }

    /// Filter events since a timestamp
    #[must_use]
    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since_filter = Some(since);
        self
    }

    /// Filter events until a timestamp
    #[must_use]
    pub fn until(mut self, until: DateTime<Utc>) -> Self {
        self.until_filter = Some(until);
        self
    }

    /// Filter by minimum severity
    #[must_use]
    pub fn min_severity(mut self, severity: AuditSeverity) -> Self {
        self.severity_filter = Some(severity);
        self
    }

    /// Limit number of results
    #[must_use]
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Execute the query
    ///
    /// # Errors
    ///
    /// Returns error if reading files fails
    pub fn execute(self) -> std::io::Result<Vec<AuditEvent>> {
        let mut events = Vec::new();

        if !self.base_dir.exists() {
            return Ok(events);
        }

        // Get all log files, sorted by date (newest first for limit queries)
        let mut log_files: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                log_files.push(path);
            }
        }
        log_files.sort_by(|a, b| b.cmp(a)); // Newest first

        'outer: for log_file in log_files {
            let file = match File::open(&log_file) {
                Ok(f) => f,
                Err(e) => {
                    // M-964: Log file open errors instead of silently skipping
                    tracing::debug!(
                        file = %log_file.display(),
                        error = %e,
                        "Failed to open audit log file, skipping"
                    );
                    continue;
                }
            };
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        // M-964: Log line read errors
                        tracing::debug!(
                            file = %log_file.display(),
                            error = %e,
                            "Failed to read line from audit log, skipping"
                        );
                        continue;
                    }
                };

                let event = match AuditEvent::from_json_line(&line) {
                    Ok(e) => e,
                    Err(e) => {
                        // M-964: Log JSON parse errors
                        tracing::debug!(
                            file = %log_file.display(),
                            error = %e,
                            "Failed to parse audit event JSON, skipping"
                        );
                        continue;
                    }
                };

                // Apply filters
                if let Some(ref action) = self.action_filter {
                    if event.action != *action {
                        continue;
                    }
                }

                if let Some(ref resource_id) = self.resource_id_filter {
                    if event.resource_id != *resource_id {
                        continue;
                    }
                }

                if let Some(ref actor) = self.actor_filter {
                    if event.actor != *actor {
                        continue;
                    }
                }

                if let Some(since) = self.since_filter {
                    if event.timestamp < since {
                        continue;
                    }
                }

                if let Some(until) = self.until_filter {
                    if event.timestamp > until {
                        continue;
                    }
                }

                if let Some(ref severity) = self.severity_filter {
                    if event.severity() < *severity {
                        continue;
                    }
                }

                events.push(event);

                if let Some(limit) = self.limit {
                    if events.len() >= limit {
                        break 'outer;
                    }
                }
            }
        }

        Ok(events)
    }
}

/// Helper trait for logging audit events from storage operations
pub trait Auditable {
    /// Get the resource type for audit logging
    fn audit_resource_type(&self) -> &'static str;

    /// Get the resource ID for audit logging
    fn audit_resource_id(&self) -> &str;
}

/// Statistics about the audit log
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditStats {
    /// Total number of events
    pub total_events: usize,

    /// Events by action type
    pub by_action: HashMap<String, usize>,

    /// Events by severity
    pub by_severity: HashMap<String, usize>,

    /// Number of log files
    pub log_file_count: usize,

    /// Total size in bytes
    pub total_size_bytes: u64,

    /// Oldest event timestamp
    pub oldest_event: Option<DateTime<Utc>>,

    /// Newest event timestamp
    pub newest_event: Option<DateTime<Utc>>,
}

impl AuditLog {
    /// Get statistics about the audit log
    ///
    /// # Errors
    ///
    /// Returns error if reading fails
    pub fn stats(&self) -> std::io::Result<AuditStats> {
        let mut stats = AuditStats::default();

        let log_files = self.log_files()?;
        stats.log_file_count = log_files.len();

        for file in &log_files {
            stats.total_size_bytes += fs::metadata(file)?.len();

            let f = File::open(file)?;
            let reader = BufReader::new(f);

            for line in reader.lines() {
                let line = line?;
                if let Ok(event) = AuditEvent::from_json_line(&line) {
                    stats.total_events += 1;

                    let action_key = format!("{:?}", event.action);
                    *stats.by_action.entry(action_key).or_insert(0) += 1;

                    let severity_key = format!("{:?}", event.severity());
                    *stats.by_severity.entry(severity_key).or_insert(0) += 1;

                    // M-965: Use map_or for cleaner pattern without unwrap()
                    if stats
                        .oldest_event
                        .map_or(true, |oldest| event.timestamp < oldest)
                    {
                        stats.oldest_event = Some(event.timestamp);
                    }
                    if stats
                        .newest_event
                        .map_or(true, |newest| event.timestamp > newest)
                    {
                        stats.newest_event = Some(event.timestamp);
                    }
                }
            }
        }

        Ok(stats)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#[allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_audit_log() -> (AuditLog, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let audit = AuditLog::new(temp_dir.path().join("audit"));
        audit.initialize().unwrap();
        (audit, temp_dir)
    }

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(AuditAction::PlanCreated, "plan-123")
            .with_resource_type("plan")
            .with_actor("user@example.com")
            .with_reason("Initial plan creation");

        assert_eq!(event.action, AuditAction::PlanCreated);
        assert_eq!(event.resource_id, "plan-123");
        assert_eq!(event.actor, "user@example.com");
        assert_eq!(event.reason, Some("Initial plan creation".to_string()));
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::new(AuditAction::PlanApproved, "plan-456")
            .with_actor("admin")
            .with_metadata("validation_score", serde_json::json!(0.95));

        let json = event.to_json_line();
        let parsed = AuditEvent::from_json_line(&json).unwrap();

        assert_eq!(parsed.action, AuditAction::PlanApproved);
        assert_eq!(parsed.resource_id, "plan-456");
        assert_eq!(parsed.metadata.get("validation_score").unwrap(), &0.95);
    }

    #[test]
    fn test_audit_log_write_and_read() {
        let (audit, _temp_dir) = temp_audit_log();

        // Write some events
        audit
            .log(&AuditEvent::new(AuditAction::PlanCreated, "plan-1").with_actor("user1"))
            .unwrap();
        audit
            .log(&AuditEvent::new(AuditAction::PlanApproved, "plan-1").with_actor("admin"))
            .unwrap();
        audit
            .log(&AuditEvent::new(AuditAction::PlanCreated, "plan-2").with_actor("user2"))
            .unwrap();

        // Query all events
        let all_events = audit.query().execute().unwrap();
        assert_eq!(all_events.len(), 3);

        // Query by resource
        let plan1_events = audit.events_for_resource("plan-1").unwrap();
        assert_eq!(plan1_events.len(), 2);

        // Query by action
        let created_events = audit.events_by_action(AuditAction::PlanCreated).unwrap();
        assert_eq!(created_events.len(), 2);

        // Query by actor
        let admin_events = audit.query().actor("admin").execute().unwrap();
        assert_eq!(admin_events.len(), 1);
    }

    #[test]
    fn test_audit_log_limit() {
        let (audit, _temp_dir) = temp_audit_log();

        for i in 0..10 {
            audit
                .log(&AuditEvent::new(
                    AuditAction::PlanCreated,
                    format!("plan-{}", i),
                ))
                .unwrap();
        }

        let recent = audit.recent_events(5).unwrap();
        assert_eq!(recent.len(), 5);
    }

    #[test]
    fn test_audit_severity() {
        assert_eq!(AuditAction::PlanApproved.severity(), AuditSeverity::High);
        assert_eq!(AuditAction::PlanCreated.severity(), AuditSeverity::Medium);
        assert_eq!(AuditAction::ReportArchived.severity(), AuditSeverity::Low);
    }

    #[test]
    fn test_audit_severity_filter() {
        let (audit, _temp_dir) = temp_audit_log();

        audit
            .log(&AuditEvent::new(AuditAction::PlanApproved, "plan-1"))
            .unwrap(); // High
        audit
            .log(&AuditEvent::new(AuditAction::PlanCreated, "plan-2"))
            .unwrap(); // Medium
        audit
            .log(&AuditEvent::new(AuditAction::ReportArchived, "report-1"))
            .unwrap(); // Low

        let high_events = audit
            .query()
            .min_severity(AuditSeverity::High)
            .execute()
            .unwrap();
        assert_eq!(high_events.len(), 1);

        let medium_plus = audit
            .query()
            .min_severity(AuditSeverity::Medium)
            .execute()
            .unwrap();
        assert_eq!(medium_plus.len(), 2);
    }

    #[test]
    fn test_audit_stats() {
        let (audit, _temp_dir) = temp_audit_log();

        audit
            .log(&AuditEvent::new(AuditAction::PlanCreated, "plan-1"))
            .unwrap();
        audit
            .log(&AuditEvent::new(AuditAction::PlanApproved, "plan-1"))
            .unwrap();

        let stats = audit.stats().unwrap();
        assert_eq!(stats.total_events, 2);
        assert_eq!(stats.log_file_count, 1);
        assert!(stats.by_action.contains_key("PlanCreated"));
        assert!(stats.by_action.contains_key("PlanApproved"));
    }

    #[test]
    fn test_audit_event_with_state() {
        let event = AuditEvent::new(AuditAction::ConfigChanged, "config-1")
            .with_previous_state("debug=false")
            .with_new_state("debug=true")
            .with_reason("Enable debugging for investigation");

        assert_eq!(event.previous_state, Some("debug=false".to_string()));
        assert_eq!(event.new_state, Some("debug=true".to_string()));
    }

    #[test]
    fn test_audit_log_files() {
        let (audit, _temp_dir) = temp_audit_log();

        audit
            .log(&AuditEvent::new(AuditAction::SystemStartup, "system"))
            .unwrap();

        let files = audit.log_files().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .ends_with(".jsonl"));
    }

    #[test]
    fn test_audit_total_count() {
        let (audit, _temp_dir) = temp_audit_log();

        for i in 0..5 {
            audit
                .log(&AuditEvent::new(
                    AuditAction::HypothesisCreated,
                    format!("hyp-{}", i),
                ))
                .unwrap();
        }

        assert_eq!(audit.total_event_count().unwrap(), 5);
    }

    #[test]
    fn test_empty_audit_log() {
        let (audit, _temp_dir) = temp_audit_log();

        let events = audit.query().execute().unwrap();
        assert!(events.is_empty());

        let count = audit.total_event_count().unwrap();
        assert_eq!(count, 0);
    }
}
