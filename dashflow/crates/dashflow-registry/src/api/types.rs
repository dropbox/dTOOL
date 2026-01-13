//! API Request and Response Types
//!
//! These types define the JSON schema for all API endpoints.

use crate::{
    BugCategory, BugSeverity, Capability, ContributionStatus, EffortEstimate, FixType, ImpactLevel,
    ImprovementCategory, PackageManifest, PackageType, PublicKey, RequestPriority, ReviewVerdict,
    SearchFilters, SearchResult, Signature, TrustLevel,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ============================================================================
// Package API Types
// ============================================================================

/// Request to publish a new package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishRequest {
    /// Package manifest (metadata)
    pub manifest: PackageManifest,
    /// Base64-encoded package tarball
    pub content: String,
    /// Package signature
    pub signature: Signature,
    /// Publisher's public key
    pub public_key: PublicKey,
}

/// Response from publish operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResponse {
    /// Content hash of the published package
    pub hash: String,
    /// Published version
    pub version: String,
    /// Whether signature was verified
    pub signature_verified: bool,
    /// Timestamp of publication
    pub published_at: DateTime<Utc>,
}

/// Request to resolve package name/version to hash
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveRequest {
    /// Package name
    pub name: String,
    /// Version requirement (semver)
    pub version: Option<String>,
}

/// Response from resolve operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolveResponse {
    /// Package name
    pub name: String,
    /// Resolved version
    pub version: String,
    /// Content hash
    pub hash: String,
    /// Download URL (via API server - always available)
    pub download_url: String,
    /// CDN download URL (direct S3/R2 - optional, bypasses API server)
    /// Only populated when CDN downloads are enabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdn_url: Option<String>,
    /// When the CDN URL expires (None for public CDN URLs)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cdn_expires_at: Option<DateTime<Utc>>,
    /// Package signatures
    pub signatures: Vec<SignatureInfo>,
}

/// Signature information for client-side verification (M-225)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// Key ID that signed
    pub key_id: String,
    /// Owner name
    pub owner: String,
    /// Trust level of the key
    pub trust_level: TrustLevel,
    /// When signed
    pub timestamp: DateTime<Utc>,
    /// Hex-encoded signature bytes for client-side verification (M-225)
    /// When present, clients can verify the signature locally without trusting the server.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_bytes: Option<String>,
    /// Hex-encoded public key bytes for client-side verification (M-225)
    /// When present with signature_bytes, enables full client-side verification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_key_bytes: Option<String>,
}

/// Response from package download (metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageResponse {
    /// Content hash
    pub hash: String,
    /// Package manifest
    pub manifest: PackageManifest,
    /// Size in bytes
    pub size: u64,
    /// Download URL
    pub download_url: String,
    /// Available mirrors
    pub mirrors: Vec<String>,
    /// Signatures
    pub signatures: Vec<SignatureInfo>,
    /// Lineage (derivation chain)
    pub lineage: Option<LineageInfo>,
}

/// Lineage chain information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageInfo {
    /// Original package hash (root of derivation)
    pub original_hash: Option<String>,
    /// Chain of derivations
    pub steps: Vec<LineageStepInfo>,
}

/// Single step in lineage chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineageStepInfo {
    /// Hash before this step
    pub from_hash: String,
    /// Hash after this step
    pub to_hash: String,
    /// What transformation was applied
    pub transformation: String,
    /// Who performed the transformation
    pub performed_by: String,
    /// Signature of this step
    pub signature: Option<String>,
}

// ============================================================================
// Search API Types
// ============================================================================

/// Unified search request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchApiRequest {
    /// Natural language query (for semantic search)
    #[serde(default)]
    pub query: Option<String>,
    /// Keywords (for keyword search)
    #[serde(default)]
    pub keywords: Option<Vec<String>>,
    /// Required capabilities
    #[serde(default)]
    pub capabilities: Option<Vec<Capability>>,
    /// Search filters
    #[serde(default)]
    pub filters: Option<SearchApiFilters>,
    /// Maximum results to return
    #[serde(default = "default_limit")]
    pub limit: u32,
    /// Offset for pagination
    #[serde(default)]
    pub offset: u32,
}

fn default_limit() -> u32 {
    20
}

/// Search filters for API
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchApiFilters {
    pub package_type: Option<PackageType>,
    pub min_downloads: Option<u64>,
    #[serde(default)]
    pub verified_only: bool,
    pub min_trust_level: Option<TrustLevel>,
    pub updated_after: Option<DateTime<Utc>>,
    pub namespace: Option<String>,
}

impl From<SearchApiFilters> for SearchFilters {
    fn from(api: SearchApiFilters) -> Self {
        SearchFilters {
            package_type: api.package_type,
            min_downloads: api.min_downloads,
            verified_only: api.verified_only,
            min_trust_level: api.min_trust_level,
            updated_after: api.updated_after,
            namespace: api.namespace,
            exclude_yanked: true, // Default to excluding yanked packages
        }
    }
}

/// Search response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchApiResponse {
    /// Matching packages
    pub results: Vec<SearchResult>,
    /// Total matches (for pagination)
    pub total: u64,
    /// Time taken in milliseconds
    pub took_ms: u64,
    /// Which search methods contributed
    pub sources: SearchSourcesInfo,
}

/// Which search sources contributed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSourcesInfo {
    pub semantic: bool,
    pub keyword: bool,
    pub capability: bool,
}

// ============================================================================
// Contribution API Types
// ============================================================================

/// Bug report submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BugReportRequest {
    /// Package hash or name@version
    pub package: String,
    /// Bug title
    pub title: String,
    /// Bug description
    pub description: String,
    /// Bug category
    pub category: BugCategory,
    /// Bug severity
    pub severity: BugSeverity,
    /// Error messages observed
    #[serde(default)]
    pub error_messages: Vec<String>,
    /// Steps to reproduce
    #[serde(default)]
    pub reproduction_steps: Vec<ReproductionStepRequest>,
    /// Occurrence rate (0-1)
    pub occurrence_rate: Option<f64>,
    /// Sample count for rate calculation
    pub sample_count: Option<u64>,
    /// Suggested fix (optional)
    pub suggested_fix: Option<SuggestedFixRequest>,
    /// Reporter information
    pub reporter: ContributorRequest,
    /// Signature
    pub signature: Signature,
}

/// Reproduction step in bug report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReproductionStepRequest {
    pub action: String,
    pub params: serde_json::Value,
}

/// Suggested fix in bug report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestedFixRequest {
    pub description: String,
    pub confidence: f64,
    pub diff: Option<String>,
}

/// Contributor information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributorRequest {
    /// Application or agent ID
    pub app_id: Uuid,
    /// Name of the contributor
    pub name: String,
    /// Public key for verification
    pub public_key: PublicKey,
    /// Whether contributor is an AI agent
    #[serde(default)]
    pub is_ai: bool,
}

/// Improvement proposal submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementRequest {
    /// Package hash or name@version
    pub package: String,
    /// Proposal title
    pub title: String,
    /// Proposal description
    pub description: String,
    /// Improvement category
    pub category: ImprovementCategory,
    /// Expected impact level
    pub impact_level: ImpactLevel,
    /// Estimated effort
    pub effort_estimate: EffortEstimate,
    /// Rationale for the improvement
    pub rationale: String,
    /// Proposed changes
    #[serde(default)]
    pub proposed_changes: Vec<String>,
    /// Alternative solutions considered
    #[serde(default)]
    pub alternatives: Vec<AlternativeRequest>,
    /// Reporter information
    pub reporter: ContributorRequest,
    /// Signature
    pub signature: Signature,
}

/// Alternative solution in improvement proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternativeRequest {
    pub description: String,
    pub rejection_reason: String,
}

/// Package request submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageRequestApiRequest {
    /// Requested package title/name
    pub title: String,
    /// Description of what the package should do
    pub description: String,
    /// Priority level
    pub priority: RequestPriority,
    /// Use cases
    #[serde(default)]
    pub use_cases: Vec<String>,
    /// Required capabilities
    #[serde(default)]
    pub required_capabilities: Vec<Capability>,
    /// Similar existing packages
    #[serde(default)]
    pub similar_packages: Vec<SimilarPackageRequest>,
    /// Suggested package name
    pub suggested_name: Option<String>,
    /// Reporter information
    pub reporter: ContributorRequest,
    /// Signature
    pub signature: Signature,
}

/// Similar package reference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarPackageRequest {
    pub name: String,
    pub why_insufficient: String,
}

/// Fix submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRequest {
    /// Package hash or name@version
    pub package: String,
    /// Fix title
    pub title: String,
    /// Fix description
    pub description: String,
    /// Fix type
    pub fix_type: FixType,
    /// Related bug or issue IDs
    #[serde(default)]
    pub fixes_issues: Vec<Uuid>,
    /// Unified diff of changes
    pub diff: String,
    /// Test cases added/modified
    #[serde(default)]
    pub test_cases: Vec<String>,
    /// Reporter information
    pub reporter: ContributorRequest,
    /// Signature
    pub signature: Signature,
}

/// Contribution submission response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContributionResponse {
    /// Contribution ID
    pub contribution_id: Uuid,
    /// Current status
    pub status: ContributionStatus,
    /// Validation results
    pub validation: ValidationResult,
    /// Next steps
    pub next_steps: Vec<String>,
    /// Estimated review time in hours
    pub estimated_review_hours: Option<u32>,
}

/// Validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub schema_valid: bool,
    pub signature_valid: bool,
    pub evidence_verifiable: bool,
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Review submission request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    /// Contribution ID to review
    pub contribution_id: Uuid,
    /// Review verdict
    pub verdict: ReviewVerdict,
    /// Confidence in verdict (0-1)
    pub confidence: f64,
    /// Comments
    pub comments: Vec<String>,
    /// Concerns raised
    #[serde(default)]
    pub concerns: Vec<ReviewConcernRequest>,
    /// Suggestions for improvement
    #[serde(default)]
    pub suggestions: Vec<String>,
    /// Reviewer information
    pub reviewer: ContributorRequest,
    /// Signature
    pub signature: Signature,
}

/// Review concern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewConcernRequest {
    pub severity: String,
    pub category: String,
    pub description: String,
    pub suggested_resolution: Option<String>,
}

/// Review response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewApiResponse {
    /// Review ID
    pub review_id: Uuid,
    /// Updated contribution status
    pub contribution_status: ContributionStatus,
    /// Consensus information (if multiple reviews)
    pub consensus: Option<ConsensusInfo>,
    /// Recommended action
    pub recommended_action: String,
}

/// Consensus information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsensusInfo {
    pub score: f64,
    pub total_reviews: usize,
    pub approve_count: usize,
    pub reject_count: usize,
}

// ============================================================================
// Trust API Types
// ============================================================================

/// Signature verification request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyRequest {
    /// Content to verify (base64 or hash)
    pub content: String,
    /// Signature to verify
    pub signature: Signature,
    /// Public key to verify against (optional, will look up if not provided)
    pub public_key: Option<PublicKey>,
}

/// Verification response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyResponse {
    pub valid: bool,
    pub key_id: Option<String>,
    pub key_owner: Option<String>,
    pub trust_level: Option<TrustLevel>,
    #[serde(default)]
    pub errors: Vec<String>,
}

/// Trusted keys list response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeysResponse {
    pub keys: Vec<KeyInfo>,
    pub total: usize,
}

/// Key information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyInfo {
    pub key_id: String,
    pub public_key: String,
    pub owner: String,
    pub trust_level: TrustLevel,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

// ============================================================================
// Batch API Types
// ============================================================================

/// Batch resolve request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResolveRequest {
    /// List of package specs to resolve
    pub packages: Vec<ResolveRequest>,
}

/// Batch resolve response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResolveResponse {
    /// Resolved packages
    pub resolved: Vec<BatchResolveResult>,
    /// Packages that failed to resolve
    pub failed: Vec<BatchResolveFailed>,
}

/// Single resolved package in batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResolveResult {
    pub name: String,
    pub version: String,
    pub hash: String,
    pub download_url: String,
}

/// Failed resolution in batch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchResolveFailed {
    pub name: String,
    pub version: Option<String>,
    pub error: String,
}

/// Batch download request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadRequest {
    /// Content hashes to download
    pub hashes: Vec<String>,
}

/// Batch download response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadResponse {
    /// Download URLs for requested packages
    pub downloads: Vec<BatchDownloadUrl>,
}

/// Download URL in batch response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchDownloadUrl {
    pub hash: String,
    pub url: String,
    /// Alternative mirrors
    pub mirrors: Vec<String>,
    /// URL expiration time
    pub expires_at: DateTime<Utc>,
}

// ============================================================================
// Error Response
// ============================================================================

/// Standard API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    /// Error code for programmatic handling
    pub code: String,
    /// Human-readable message
    pub message: String,
    /// Additional details
    #[serde(default)]
    pub details: Option<serde_json::Value>,
    /// Request ID for debugging
    pub request_id: Option<String>,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
            request_id: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }
}

// Common error codes
pub mod error_codes {
    pub const NOT_FOUND: &str = "NOT_FOUND";
    pub const INVALID_REQUEST: &str = "INVALID_REQUEST";
    pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
    pub const SIGNATURE_INVALID: &str = "SIGNATURE_INVALID";
    pub const RATE_LIMITED: &str = "RATE_LIMITED";
    pub const INTERNAL_ERROR: &str = "INTERNAL_ERROR";
    pub const VALIDATION_FAILED: &str = "VALIDATION_FAILED";
    pub const CONFLICT: &str = "CONFLICT";
    pub const HASH_MISMATCH: &str = "HASH_MISMATCH";
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // default_limit tests
    // ========================================================================

    #[test]
    fn test_default_limit() {
        assert_eq!(default_limit(), 20);
    }

    // ========================================================================
    // ApiError tests
    // ========================================================================

    #[test]
    fn test_api_error_new() {
        let err = ApiError::new("NOT_FOUND", "Resource not found");
        assert_eq!(err.code, "NOT_FOUND");
        assert_eq!(err.message, "Resource not found");
        assert!(err.details.is_none());
        assert!(err.request_id.is_none());
    }

    #[test]
    fn test_api_error_new_with_into() {
        let err = ApiError::new(String::from("ERROR"), String::from("message"));
        assert_eq!(err.code, "ERROR");
        assert_eq!(err.message, "message");
    }

    #[test]
    fn test_api_error_with_details() {
        let err = ApiError::new("VALIDATION_FAILED", "Invalid input")
            .with_details(json!({"field": "email", "reason": "invalid format"}));
        assert_eq!(err.code, "VALIDATION_FAILED");
        assert!(err.details.is_some());
        let details = err.details.unwrap();
        assert_eq!(details["field"], "email");
    }

    #[test]
    fn test_api_error_with_request_id() {
        let err = ApiError::new("INTERNAL_ERROR", "Something went wrong")
            .with_request_id("req-12345");
        assert_eq!(err.request_id, Some("req-12345".to_string()));
    }

    #[test]
    fn test_api_error_chained() {
        let err = ApiError::new("ERROR", "msg")
            .with_details(json!({"key": "value"}))
            .with_request_id("req-abc");
        assert_eq!(err.code, "ERROR");
        assert!(err.details.is_some());
        assert_eq!(err.request_id, Some("req-abc".to_string()));
    }

    #[test]
    fn test_api_error_serialize() {
        let err = ApiError::new("NOT_FOUND", "Not found");
        let json_str = serde_json::to_string(&err).unwrap();
        assert!(json_str.contains("NOT_FOUND"));
        assert!(json_str.contains("Not found"));
    }

    #[test]
    fn test_api_error_deserialize() {
        let json_str = r#"{"code":"ERROR","message":"test","details":null,"request_id":null}"#;
        let err: ApiError = serde_json::from_str(json_str).unwrap();
        assert_eq!(err.code, "ERROR");
        assert_eq!(err.message, "test");
    }

    #[test]
    fn test_api_error_debug() {
        let err = ApiError::new("TEST", "test message");
        let debug = format!("{:?}", err);
        assert!(debug.contains("ApiError"));
        assert!(debug.contains("TEST"));
    }

    #[test]
    fn test_api_error_clone() {
        let err = ApiError::new("CLONE", "test").with_request_id("123");
        let cloned = err.clone();
        assert_eq!(err.code, cloned.code);
        assert_eq!(err.request_id, cloned.request_id);
    }

    // ========================================================================
    // SearchApiFilters tests
    // ========================================================================

    #[test]
    fn test_search_api_filters_default() {
        let filters = SearchApiFilters::default();
        assert!(filters.package_type.is_none());
        assert!(filters.min_downloads.is_none());
        assert!(!filters.verified_only);
        assert!(filters.min_trust_level.is_none());
        assert!(filters.updated_after.is_none());
        assert!(filters.namespace.is_none());
    }

    #[test]
    fn test_search_api_filters_from_conversion() {
        let api_filters = SearchApiFilters {
            package_type: Some(PackageType::Node),
            min_downloads: Some(100),
            verified_only: true,
            min_trust_level: None,
            updated_after: None,
            namespace: Some("test".to_string()),
        };
        let filters: SearchFilters = api_filters.into();
        assert_eq!(filters.package_type, Some(PackageType::Node));
        assert_eq!(filters.min_downloads, Some(100));
        assert!(filters.verified_only);
        assert!(filters.exclude_yanked); // Default to true
        assert_eq!(filters.namespace, Some("test".to_string()));
    }

    #[test]
    fn test_search_api_filters_exclude_yanked_default() {
        let api_filters = SearchApiFilters::default();
        let filters: SearchFilters = api_filters.into();
        // exclude_yanked should default to true
        assert!(filters.exclude_yanked);
    }

    // ========================================================================
    // SearchApiRequest tests
    // ========================================================================

    #[test]
    fn test_search_api_request_default_limit() {
        let json_str = r#"{"query":"test"}"#;
        let req: SearchApiRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.limit, 20); // default_limit()
        assert_eq!(req.offset, 0);
    }

    #[test]
    fn test_search_api_request_custom_limit() {
        let json_str = r#"{"query":"test","limit":50,"offset":10}"#;
        let req: SearchApiRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.limit, 50);
        assert_eq!(req.offset, 10);
    }

    // ========================================================================
    // SearchSourcesInfo tests
    // ========================================================================

    #[test]
    fn test_search_sources_info_serialize() {
        let sources = SearchSourcesInfo {
            semantic: true,
            keyword: false,
            capability: true,
        };
        let json = serde_json::to_value(&sources).unwrap();
        assert_eq!(json["semantic"], true);
        assert_eq!(json["keyword"], false);
        assert_eq!(json["capability"], true);
    }

    // ========================================================================
    // ValidationResult tests
    // ========================================================================

    #[test]
    fn test_validation_result_serialize() {
        let result = ValidationResult {
            schema_valid: true,
            signature_valid: true,
            evidence_verifiable: false,
            errors: vec!["Evidence check failed".to_string()],
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["schema_valid"], true);
        assert_eq!(json["errors"][0], "Evidence check failed");
    }

    #[test]
    fn test_validation_result_default_errors() {
        let json_str = r#"{"schema_valid":true,"signature_valid":true,"evidence_verifiable":true}"#;
        let result: ValidationResult = serde_json::from_str(json_str).unwrap();
        assert!(result.errors.is_empty());
    }

    // ========================================================================
    // ConsensusInfo tests
    // ========================================================================

    #[test]
    fn test_consensus_info_serialize() {
        let consensus = ConsensusInfo {
            score: 0.85,
            total_reviews: 10,
            approve_count: 8,
            reject_count: 2,
        };
        let json = serde_json::to_value(&consensus).unwrap();
        assert_eq!(json["score"], 0.85);
        assert_eq!(json["total_reviews"], 10);
    }

    // ========================================================================
    // Batch API types tests
    // ========================================================================

    #[test]
    fn test_batch_resolve_result_serialize() {
        let result = BatchResolveResult {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            hash: "abc123".to_string(),
            download_url: "https://example.com/pkg.tar.gz".to_string(),
        };
        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["name"], "test-pkg");
        assert_eq!(json["version"], "1.0.0");
    }

    #[test]
    fn test_batch_resolve_failed_serialize() {
        let failed = BatchResolveFailed {
            name: "missing-pkg".to_string(),
            version: Some("2.0.0".to_string()),
            error: "Package not found".to_string(),
        };
        let json = serde_json::to_value(&failed).unwrap();
        assert_eq!(json["name"], "missing-pkg");
        assert_eq!(json["error"], "Package not found");
    }

    // ========================================================================
    // error_codes tests
    // ========================================================================

    #[test]
    fn test_error_codes_values() {
        assert_eq!(error_codes::NOT_FOUND, "NOT_FOUND");
        assert_eq!(error_codes::INVALID_REQUEST, "INVALID_REQUEST");
        assert_eq!(error_codes::UNAUTHORIZED, "UNAUTHORIZED");
        assert_eq!(error_codes::SIGNATURE_INVALID, "SIGNATURE_INVALID");
        assert_eq!(error_codes::RATE_LIMITED, "RATE_LIMITED");
        assert_eq!(error_codes::INTERNAL_ERROR, "INTERNAL_ERROR");
        assert_eq!(error_codes::VALIDATION_FAILED, "VALIDATION_FAILED");
        assert_eq!(error_codes::CONFLICT, "CONFLICT");
        assert_eq!(error_codes::HASH_MISMATCH, "HASH_MISMATCH");
    }

    // ========================================================================
    // SignatureInfo tests
    // ========================================================================

    #[test]
    fn test_signature_info_skip_serializing_none() {
        let info = SignatureInfo {
            key_id: "key-1".to_string(),
            owner: "test-owner".to_string(),
            trust_level: TrustLevel::Medium,
            timestamp: chrono::Utc::now(),
            signature_bytes: None,
            public_key_bytes: None,
        };
        let json = serde_json::to_value(&info).unwrap();
        // signature_bytes and public_key_bytes should be omitted when None
        assert!(!json.as_object().unwrap().contains_key("signature_bytes"));
        assert!(!json.as_object().unwrap().contains_key("public_key_bytes"));
    }

    #[test]
    fn test_signature_info_with_bytes() {
        let info = SignatureInfo {
            key_id: "key-2".to_string(),
            owner: "owner".to_string(),
            trust_level: TrustLevel::High,
            timestamp: chrono::Utc::now(),
            signature_bytes: Some("deadbeef".to_string()),
            public_key_bytes: Some("cafebabe".to_string()),
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["signature_bytes"], "deadbeef");
        assert_eq!(json["public_key_bytes"], "cafebabe");
    }

    // ========================================================================
    // LineageInfo tests
    // ========================================================================

    #[test]
    fn test_lineage_info_serialize() {
        let lineage = LineageInfo {
            original_hash: Some("original-hash".to_string()),
            steps: vec![LineageStepInfo {
                from_hash: "hash1".to_string(),
                to_hash: "hash2".to_string(),
                transformation: "optimize".to_string(),
                performed_by: "optimizer-agent".to_string(),
                signature: Some("sig123".to_string()),
            }],
        };
        let json = serde_json::to_value(&lineage).unwrap();
        assert_eq!(json["original_hash"], "original-hash");
        assert_eq!(json["steps"][0]["transformation"], "optimize");
    }

    // ========================================================================
    // VerifyResponse tests
    // ========================================================================

    #[test]
    fn test_verify_response_default_errors() {
        let json_str = r#"{"valid":true,"key_id":"k1","key_owner":"owner","trust_level":"High"}"#;
        let resp: VerifyResponse = serde_json::from_str(json_str).unwrap();
        assert!(resp.valid);
        assert!(resp.errors.is_empty());
    }
}
