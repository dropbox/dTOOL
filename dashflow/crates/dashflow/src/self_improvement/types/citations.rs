// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]
// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Citation types for traceability and evidence linking.
//!
//! This module provides types for referencing source data (execution traces,
//! git commits, logs, etc.) to maintain traceability in introspection reports.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::common::ModelIdentifier;

// =============================================================================
// Citation - Reference to Source Data
// =============================================================================

/// Reference to source data for traceability.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Citation {
    /// Citation identifier (e.g., "\[1\]", "\[trace-abc123\]")
    pub id: String,

    /// Type of source
    pub source_type: CitationSource,

    /// Human-readable description
    pub description: String,

    /// How to retrieve the source data
    pub retrieval: CitationRetrieval,
}

impl Citation {
    /// Create a citation to an execution trace
    #[must_use]
    pub fn trace(thread_id: impl Into<String>) -> Self {
        let thread_id = thread_id.into();
        Self {
            id: format!("trace-{}", &thread_id[..8.min(thread_id.len())]),
            source_type: CitationSource::ExecutionTrace {
                thread_id: thread_id.clone(),
                timestamp: Utc::now(),
            },
            description: format!("ExecutionTrace thread_id={thread_id}"),
            retrieval: CitationRetrieval::TraceStorage {
                query: format!("thread_id = '{thread_id}'"),
            },
        }
    }

    /// Create a citation to an aggregation query
    #[must_use]
    pub fn aggregation(query: impl Into<String>, summary: impl Into<String>) -> Self {
        let query = query.into();
        let summary = summary.into();
        Self {
            id: format!("agg-{}", &Uuid::new_v4().as_simple().to_string()[..8]),
            source_type: CitationSource::Aggregation {
                query: query.clone(),
                result_summary: summary.clone(),
            },
            description: format!("Aggregation: {summary}"),
            retrieval: CitationRetrieval::Inline { data: summary },
        }
    }

    /// Create a citation to a previous report
    #[must_use]
    pub fn report(report_id: Uuid) -> Self {
        Self {
            id: format!("report-{}", &report_id.to_string()[..8]),
            source_type: CitationSource::IntrospectionReport { report_id },
            description: format!("Previous report {report_id}"),
            retrieval: CitationRetrieval::File {
                path: format!(".dashflow/introspection/reports/{report_id}.json"),
            },
        }
    }

    /// Create a citation to a git commit
    #[must_use]
    pub fn commit(hash: impl Into<String>, message: impl Into<String>) -> Self {
        let hash = hash.into();
        let message = message.into();
        Self {
            id: format!("commit-{}", &hash[..7.min(hash.len())]),
            source_type: CitationSource::GitCommit {
                hash: hash.clone(),
                message_summary: message.clone(),
            },
            description: format!("Commit {}: {}", &hash[..7.min(hash.len())], message),
            retrieval: CitationRetrieval::Git {
                command: format!("git show {hash}"),
            },
        }
    }
}

// =============================================================================
// CitationSource - Type of Citation Source
// =============================================================================

/// Type of citation source.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum CitationSource {
    /// Reference to an ExecutionTrace
    ExecutionTrace {
        /// DashStream thread identifier for the trace.
        thread_id: String,
        /// When the trace was captured/observed.
        timestamp: DateTime<Utc>,
    },

    /// Aggregated statistics
    Aggregation {
        /// Query used to compute the aggregation (SQL/PromQL/etc.).
        query: String,
        /// Human-readable summary of the query result.
        result_summary: String,
    },

    /// Previous introspection report
    IntrospectionReport {
        /// Stable identifier for the stored report.
        report_id: Uuid,
    },

    /// Git commit
    GitCommit {
        /// Commit hash (full or short).
        hash: String,
        /// One-line commit message (or other short summary).
        message_summary: String,
    },

    /// External AI review
    ModelReview {
        /// Which model provided the review.
        model: ModelIdentifier,
        /// When the review was produced.
        timestamp: DateTime<Utc>,
    },

    /// Log file
    LogFile {
        /// Filesystem path to the log file.
        path: String,
        /// Optional inclusive line range to highlight in the log file.
        line_range: Option<(usize, usize)>,
    },
}

// =============================================================================
// CitationRetrieval - How to Retrieve Citation Data
// =============================================================================

/// How to retrieve citation source data.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub enum CitationRetrieval {
    /// Retrieve from ExecutionTrace storage
    TraceStorage {
        /// Query used to locate the trace (backend-specific).
        query: String,
    },

    /// Retrieve from git
    Git {
        /// Shell command that prints the relevant content (e.g. `git show <hash>`).
        command: String,
    },

    /// Retrieve from file
    File {
        /// Filesystem path to read.
        path: String,
    },

    /// Inline data (for small citations)
    Inline {
        /// Inlined payload (kept small; suitable for summaries).
        data: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_citation_trace() {
        let citation = Citation::trace("thread-abc123");
        assert!(citation.id.starts_with("trace-"));
        assert!(matches!(
            citation.source_type,
            CitationSource::ExecutionTrace { .. }
        ));
    }

    #[test]
    fn test_citation_commit() {
        let citation = Citation::commit("abc1234567", "Add feature");
        assert!(citation.id.starts_with("commit-"));
        assert!(matches!(
            citation.source_type,
            CitationSource::GitCommit { .. }
        ));
    }

    #[test]
    fn test_citation_report() {
        let report_id = Uuid::new_v4();
        let citation = Citation::report(report_id);
        assert!(citation.id.starts_with("report-"));
        assert!(matches!(
            citation.source_type,
            CitationSource::IntrospectionReport { .. }
        ));
    }

    #[test]
    fn test_citation_aggregation() {
        let citation = Citation::aggregation("SELECT * FROM traces", "47 executions analyzed");
        assert!(citation.id.starts_with("agg-"));
        assert!(matches!(
            citation.source_type,
            CitationSource::Aggregation { .. }
        ));
    }
}
