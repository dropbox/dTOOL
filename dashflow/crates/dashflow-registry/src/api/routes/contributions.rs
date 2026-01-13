//! Contribution Routes
//!
//! Handlers for structured contributions: bug reports, improvements, requests, fixes.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use uuid::Uuid;

use crate::api::{
    types::{
        error_codes, ApiError, BugReportRequest, ConsensusInfo, ContributionResponse, FixRequest,
        ImprovementRequest, PackageRequestApiRequest, ReviewApiResponse, ReviewRequest,
        ValidationResult,
    },
    AppState,
};
use crate::{BugSeverity, ContributionStatus, StoredContribution, StoredReview};

/// Contribution routes
pub fn routes() -> Router<AppState> {
    Router::new()
        // POST /contributions/bug - Submit bug report
        .route("/bug", post(submit_bug_report))
        // POST /contributions/improvement - Submit improvement proposal
        .route("/improvement", post(submit_improvement))
        // POST /contributions/request - Submit package request
        .route("/request", post(submit_package_request))
        // POST /contributions/fix - Submit fix
        .route("/fix", post(submit_fix))
        // GET /contributions/:id - Get contribution status
        .route("/:id", get(get_contribution))
        // POST /contributions/:id/review - Submit review
        .route("/:id/review", post(submit_review))
        // GET /contributions - List contributions
        .route("/", get(list_contributions))
}

/// Submit a structured bug report
async fn submit_bug_report(
    State(state): State<AppState>,
    Json(request): Json<BugReportRequest>,
) -> Result<Json<ContributionResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request
    let mut validation_errors = Vec::new();

    if request.title.is_empty() {
        validation_errors.push("Title is required".to_string());
    }
    if request.description.is_empty() {
        validation_errors.push("Description is required".to_string());
    }

    // Verify signature
    let signature_valid = state
        .trust
        .verify_data_signature(
            request.title.as_bytes(),
            &request.signature,
            &request.reporter.public_key,
        )
        .is_ok();

    if !validation_errors.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                ApiError::new(error_codes::VALIDATION_FAILED, "Validation failed")
                    .with_details(serde_json::json!({ "errors": validation_errors })),
            ),
        ));
    }

    // Generate contribution ID
    let contribution_id = Uuid::new_v4();

    // Calculate estimated review time based on severity
    let estimated_review_hours = match request.severity {
        BugSeverity::Critical => Some(4),
        BugSeverity::High => Some(12),
        BugSeverity::Medium => Some(24),
        BugSeverity::Low => Some(48),
    };

    // Create stored contribution
    let now = Utc::now();
    let stored = StoredContribution {
        id: contribution_id,
        contribution_type: "bug".to_string(),
        package_hash: Some(request.package.clone()),
        title: request.title.clone(),
        description: request.description.clone(),
        status: ContributionStatus::Submitted,
        reporter_public_key: request.reporter.public_key.key_id.clone(),
        reporter_name: Some(request.reporter.name.clone()),
        reporter_app_id: Some(request.reporter.app_id),
        reporter_is_ai: request.reporter.is_ai,
        data: serde_json::json!({
            "severity": format!("{:?}", request.severity),
            "category": format!("{:?}", request.category),
            "occurrence_rate": request.occurrence_rate,
            "sample_count": request.sample_count,
            "error_messages": request.error_messages,
            "reproduction_steps": request.reproduction_steps,
            "suggested_fix": request.suggested_fix,
        }),
        signature: serde_json::to_string(&request.signature).unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    // Store in database
    state
        .contributions
        .store_contribution(&stored)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to store contribution: {}", e),
                )),
            )
        })?;

    Ok(Json(ContributionResponse {
        contribution_id,
        status: ContributionStatus::Submitted,
        validation: ValidationResult {
            schema_valid: validation_errors.is_empty(),
            signature_valid,
            evidence_verifiable: !request.error_messages.is_empty()
                || !request.reproduction_steps.is_empty(),
            errors: validation_errors,
        },
        next_steps: vec![
            "awaiting_consensus_review".to_string(),
            "Will notify when reviewed".to_string(),
        ],
        estimated_review_hours,
    }))
}

/// Submit an improvement proposal
async fn submit_improvement(
    State(state): State<AppState>,
    Json(request): Json<ImprovementRequest>,
) -> Result<Json<ContributionResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request
    let mut validation_errors = Vec::new();

    if request.title.is_empty() {
        validation_errors.push("Title is required".to_string());
    }
    if request.description.is_empty() {
        validation_errors.push("Description is required".to_string());
    }
    if request.rationale.is_empty() {
        validation_errors.push("Rationale is required".to_string());
    }

    // Verify signature
    let signature_valid = state
        .trust
        .verify_data_signature(
            request.title.as_bytes(),
            &request.signature,
            &request.reporter.public_key,
        )
        .is_ok();

    if !validation_errors.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                ApiError::new(error_codes::VALIDATION_FAILED, "Validation failed")
                    .with_details(serde_json::json!({ "errors": validation_errors })),
            ),
        ));
    }

    let contribution_id = Uuid::new_v4();

    // Create stored contribution
    let now = Utc::now();
    let stored = StoredContribution {
        id: contribution_id,
        contribution_type: "improvement".to_string(),
        package_hash: Some(request.package.clone()),
        title: request.title.clone(),
        description: request.description.clone(),
        status: ContributionStatus::Submitted,
        reporter_public_key: request.reporter.public_key.key_id.clone(),
        reporter_name: Some(request.reporter.name.clone()),
        reporter_app_id: Some(request.reporter.app_id),
        reporter_is_ai: request.reporter.is_ai,
        data: serde_json::json!({
            "category": format!("{:?}", request.category),
            "impact_level": format!("{:?}", request.impact_level),
            "effort_estimate": format!("{:?}", request.effort_estimate),
            "rationale": request.rationale,
            "proposed_changes": request.proposed_changes,
            "alternatives": request.alternatives,
        }),
        signature: serde_json::to_string(&request.signature).unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    // Store in database
    state
        .contributions
        .store_contribution(&stored)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to store contribution: {}", e),
                )),
            )
        })?;

    Ok(Json(ContributionResponse {
        contribution_id,
        status: ContributionStatus::Submitted,
        validation: ValidationResult {
            schema_valid: validation_errors.is_empty(),
            signature_valid,
            evidence_verifiable: true,
            errors: validation_errors,
        },
        next_steps: vec!["awaiting_consensus_review".to_string()],
        estimated_review_hours: Some(48),
    }))
}

/// Submit a package request
async fn submit_package_request(
    State(state): State<AppState>,
    Json(request): Json<PackageRequestApiRequest>,
) -> Result<Json<ContributionResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request
    let mut validation_errors = Vec::new();

    if request.title.is_empty() {
        validation_errors.push("Title is required".to_string());
    }
    if request.description.is_empty() {
        validation_errors.push("Description is required".to_string());
    }
    if request.use_cases.is_empty() {
        validation_errors.push("At least one use case is required".to_string());
    }

    // Verify signature
    let signature_valid = state
        .trust
        .verify_data_signature(
            request.title.as_bytes(),
            &request.signature,
            &request.reporter.public_key,
        )
        .is_ok();

    if !validation_errors.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                ApiError::new(error_codes::VALIDATION_FAILED, "Validation failed")
                    .with_details(serde_json::json!({ "errors": validation_errors })),
            ),
        ));
    }

    let contribution_id = Uuid::new_v4();

    // Create stored contribution - package_hash is None for package requests
    let now = Utc::now();
    let stored = StoredContribution {
        id: contribution_id,
        contribution_type: "request".to_string(),
        package_hash: None, // Package requests don't reference existing packages
        title: request.title.clone(),
        description: request.description.clone(),
        status: ContributionStatus::Submitted,
        reporter_public_key: request.reporter.public_key.key_id.clone(),
        reporter_name: Some(request.reporter.name.clone()),
        reporter_app_id: Some(request.reporter.app_id),
        reporter_is_ai: request.reporter.is_ai,
        data: serde_json::json!({
            "use_cases": request.use_cases,
            "priority": format!("{:?}", request.priority),
            "similar_packages": request.similar_packages,
            "required_capabilities": request.required_capabilities,
            "suggested_name": request.suggested_name,
        }),
        signature: serde_json::to_string(&request.signature).unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    // Store in database
    state
        .contributions
        .store_contribution(&stored)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to store contribution: {}", e),
                )),
            )
        })?;

    Ok(Json(ContributionResponse {
        contribution_id,
        status: ContributionStatus::Submitted,
        validation: ValidationResult {
            schema_valid: validation_errors.is_empty(),
            signature_valid,
            evidence_verifiable: true,
            errors: validation_errors,
        },
        next_steps: vec![
            "request_logged".to_string(),
            "Community voting enabled".to_string(),
        ],
        estimated_review_hours: None,
    }))
}

/// Submit a fix
async fn submit_fix(
    State(state): State<AppState>,
    Json(request): Json<FixRequest>,
) -> Result<Json<ContributionResponse>, (StatusCode, Json<ApiError>)> {
    // Validate request
    let mut validation_errors = Vec::new();

    if request.title.is_empty() {
        validation_errors.push("Title is required".to_string());
    }
    if request.diff.is_empty() {
        validation_errors.push("Diff is required".to_string());
    }

    // Verify signature
    let signature_valid = state
        .trust
        .verify_data_signature(
            request.diff.as_bytes(),
            &request.signature,
            &request.reporter.public_key,
        )
        .is_ok();

    // Validate diff format (basic check)
    let diff_valid = request.diff.contains("---") && request.diff.contains("+++");
    if !diff_valid {
        validation_errors.push("Diff must be in unified diff format".to_string());
    }

    if !validation_errors.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                ApiError::new(error_codes::VALIDATION_FAILED, "Validation failed")
                    .with_details(serde_json::json!({ "errors": validation_errors })),
            ),
        ));
    }

    let contribution_id = Uuid::new_v4();

    // Create stored contribution for fix
    let now = Utc::now();
    let stored = StoredContribution {
        id: contribution_id,
        contribution_type: "fix".to_string(),
        package_hash: Some(request.package.clone()),
        title: request.title.clone(),
        description: request.description.clone(),
        status: ContributionStatus::Submitted,
        reporter_public_key: request.reporter.public_key.key_id.clone(),
        reporter_name: Some(request.reporter.name.clone()),
        reporter_app_id: Some(request.reporter.app_id),
        reporter_is_ai: request.reporter.is_ai,
        data: serde_json::json!({
            "diff": request.diff,
            "fix_type": format!("{:?}", request.fix_type),
            "fixes_issues": request.fixes_issues,
            "test_cases": request.test_cases,
        }),
        signature: serde_json::to_string(&request.signature).unwrap_or_default(),
        created_at: now,
        updated_at: now,
    };

    // Store in database
    state
        .contributions
        .store_contribution(&stored)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to store contribution: {}", e),
                )),
            )
        })?;

    Ok(Json(ContributionResponse {
        contribution_id,
        status: ContributionStatus::Submitted,
        validation: ValidationResult {
            schema_valid: validation_errors.is_empty(),
            signature_valid,
            evidence_verifiable: diff_valid,
            errors: validation_errors,
        },
        next_steps: vec![
            "awaiting_code_review".to_string(),
            "Tests will be run automatically".to_string(),
        ],
        estimated_review_hours: Some(24),
    }))
}

/// Get contribution status
async fn get_contribution(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<ContributionDetailResponse>, (StatusCode, Json<ApiError>)> {
    // Look up contribution in database
    let contribution = state
        .contributions
        .get_contribution(id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Database error: {}", e),
                )),
            )
        })?;

    match contribution {
        Some(c) => {
            // Get reviews for this contribution
            let reviews = state.contributions.get_reviews(id).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError::new(
                        error_codes::INTERNAL_ERROR,
                        format!("Failed to get reviews: {}", e),
                    )),
                )
            })?;

            let review_summaries: Vec<ReviewSummary> = reviews
                .into_iter()
                .map(|r| ReviewSummary {
                    reviewer: r
                        .reviewer_name
                        .unwrap_or_else(|| r.reviewer_public_key.clone()),
                    verdict: r.verdict,
                    confidence: r.confidence as f64,
                    created_at: r.created_at,
                })
                .collect();

            Ok(Json(ContributionDetailResponse {
                id: c.id,
                status: c.status,
                contribution_type: c.contribution_type,
                title: c.title,
                created_at: c.created_at,
                reviews: review_summaries,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                error_codes::NOT_FOUND,
                format!("Contribution {} not found", id),
            )),
        )),
    }
}

/// Contribution detail response
#[derive(Debug, serde::Serialize)]
struct ContributionDetailResponse {
    id: Uuid,
    status: ContributionStatus,
    contribution_type: String,
    title: String,
    created_at: chrono::DateTime<chrono::Utc>,
    reviews: Vec<ReviewSummary>,
}

/// Review summary
#[derive(Debug, serde::Serialize)]
struct ReviewSummary {
    reviewer: String,
    verdict: String,
    confidence: f64,
    created_at: chrono::DateTime<chrono::Utc>,
}

/// Submit a review
async fn submit_review(
    State(state): State<AppState>,
    Path(contribution_id): Path<Uuid>,
    Json(request): Json<ReviewRequest>,
) -> Result<Json<ReviewApiResponse>, (StatusCode, Json<ApiError>)> {
    // Check contribution exists
    let contribution = state
        .contributions
        .get_contribution(contribution_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Database error: {}", e),
                )),
            )
        })?;

    if contribution.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ApiError::new(
                error_codes::NOT_FOUND,
                format!("Contribution {} not found", contribution_id),
            )),
        ));
    }

    // Verify signature
    let signature_valid = state
        .trust
        .verify_data_signature(
            format!("{:?}", request.verdict).as_bytes(),
            &request.signature,
            &request.reviewer.public_key,
        )
        .is_ok();

    if !signature_valid {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError::new(
                error_codes::SIGNATURE_INVALID,
                "Review signature invalid",
            )),
        ));
    }

    // Generate review ID
    let review_id = Uuid::new_v4();

    // Format verdict as string
    let verdict_str = match request.verdict {
        crate::ReviewVerdict::Approve => "approve",
        crate::ReviewVerdict::ApproveWithSuggestions => "approve_with_suggestions",
        crate::ReviewVerdict::RequestChanges => "request_changes",
        crate::ReviewVerdict::Reject => "reject",
        crate::ReviewVerdict::Abstain => "abstain",
    };

    // Create justification from comments
    let justification = if request.comments.is_empty() {
        None
    } else {
        Some(request.comments.join("\n"))
    };

    // Create stored review
    let stored_review = StoredReview {
        id: review_id,
        contribution_id,
        reviewer_app_id: Some(request.reviewer.app_id),
        reviewer_public_key: request.reviewer.public_key.key_id.clone(),
        reviewer_name: Some(request.reviewer.name.clone()),
        reviewer_is_ai: request.reviewer.is_ai,
        verdict: verdict_str.to_string(),
        confidence: request.confidence as f32,
        justification,
        signature: serde_json::to_string(&request.signature).unwrap_or_default(),
        created_at: Utc::now(),
    };

    // Store review in database
    state
        .contributions
        .store_review(&stored_review)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to store review: {}", e),
                )),
            )
        })?;

    // Update contribution status to UnderReview
    let _ = state
        .contributions
        .update_contribution_status(contribution_id, ContributionStatus::UnderReview)
        .await;

    // Get real consensus from database
    let db_consensus = state
        .contributions
        .get_consensus(contribution_id)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Failed to get consensus: {}", e),
                )),
            )
        })?;

    let consensus = db_consensus.map(|c| ConsensusInfo {
        score: c.avg_confidence as f64,
        total_reviews: c.total_reviews as usize,
        approve_count: c.approve_count as usize,
        reject_count: c.reject_count as usize,
    });

    let recommended_action = match request.verdict {
        crate::ReviewVerdict::Approve => "auto_approve_eligible",
        crate::ReviewVerdict::ApproveWithSuggestions => "notify_maintainer",
        crate::ReviewVerdict::RequestChanges => "request_changes",
        crate::ReviewVerdict::Reject => "reject",
        crate::ReviewVerdict::Abstain => "await_more_reviews",
    };

    Ok(Json(ReviewApiResponse {
        review_id,
        contribution_status: ContributionStatus::UnderReview,
        consensus,
        recommended_action: recommended_action.to_string(),
    }))
}

/// List contributions query params
#[derive(Debug, serde::Deserialize)]
struct ListContributionsParams {
    #[serde(default)]
    package: Option<String>,
    #[serde(default)]
    status: Option<ContributionStatus>,
    #[serde(default = "default_limit")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

fn default_limit() -> u32 {
    20
}

/// List contributions
async fn list_contributions(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<ListContributionsParams>,
) -> Result<Json<ListContributionsResponse>, (StatusCode, Json<ApiError>)> {
    // Query database with filters
    let (contributions, total) = state
        .contributions
        .list_contributions(
            params.package.as_deref(),
            params.status,
            params.limit as usize,
            params.offset as usize,
        )
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiError::new(
                    error_codes::INTERNAL_ERROR,
                    format!("Database error: {}", e),
                )),
            )
        })?;

    // Map to summary format
    let summaries: Vec<ContributionSummary> = contributions
        .into_iter()
        .map(|c| ContributionSummary {
            id: c.id,
            contribution_type: c.contribution_type,
            title: c.title,
            status: c.status,
            package: c.package_hash,
            created_at: c.created_at,
        })
        .collect();

    Ok(Json(ListContributionsResponse {
        contributions: summaries,
        total,
        limit: params.limit,
        offset: params.offset,
    }))
}

/// List contributions response
#[derive(Debug, serde::Serialize)]
struct ListContributionsResponse {
    contributions: Vec<ContributionSummary>,
    total: u64,
    limit: u32,
    offset: u32,
}

/// Contribution summary for listing
#[derive(Debug, serde::Serialize)]
struct ContributionSummary {
    id: Uuid,
    contribution_type: String,
    title: String,
    status: ContributionStatus,
    package: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_diff_validation() {
        let valid_diff = "--- a/file.rs\n+++ b/file.rs\n@@ -1 +1 @@\n-old\n+new";
        assert!(valid_diff.contains("---") && valid_diff.contains("+++"));

        let invalid_diff = "some random text";
        assert!(!(invalid_diff.contains("---") && invalid_diff.contains("+++")));
    }
}
