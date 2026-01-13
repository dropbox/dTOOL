// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Multi-Model Consensus System for Self-Improvement
//!
//! This module implements the Self-Improvement roadmap - obtaining
//! multiple perspectives on improvement proposals by consulting different AI models.
//!
//! The key insight is that different AI models (Claude, GPT, Gemini) have different
//! biases and blind spots. By consulting multiple models and synthesizing their
//! reviews, we can achieve more robust validation of improvement proposals.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────┐
//! │ ImprovementProposal │
//! └──────────┬──────────┘
//!            │
//!            ▼
//! ┌─────────────────────┐
//! │  ConsensusBuilder   │──┬──▶ AnthropicReviewer
//! └─────────────────────┘  │
//!                          ├──▶ OpenAIReviewer
//!                          │
//!                          └──▶ GoogleReviewer
//!            │
//!            ▼
//! ┌─────────────────────┐
//! │  ConsensusResult    │
//! └─────────────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow::self_improvement::{
//!     ConsensusBuilder, ImprovementProposal, ReviewRequest,
//!     AnthropicReviewer, OpenAIReviewer,
//! };
//!
//! // Create reviewers (requires API keys)
//! let anthropic = AnthropicReviewer::from_env()?;
//! let openai = OpenAIReviewer::from_env()?;
//!
//! // Build consensus
//! let consensus = ConsensusBuilder::new()
//!     .add_reviewer(Box::new(anthropic))
//!     .add_reviewer(Box::new(openai))
//!     .build_consensus(&proposals)
//!     .await?;
//!
//! println!("Consensus score: {}", consensus.consensus_score);
//! println!("Validated: {}", consensus.validated);
//! ```
//!
//! # API Key Detection
//!
//! Reviewers automatically activate when their API keys are present:
//! - `ANTHROPIC_API_KEY` → Claude models
//! - `OPENAI_API_KEY` → GPT models
//! - `GOOGLE_API_KEY` → Gemini models
//!
//! If no keys are present, consensus is silently skipped.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::types::{
    Assessment, ConsensusResult, Critique, CritiqueSeverity, Disagreement, ImprovementProposal,
    ModelIdentifier, ModelReview,
};
use crate::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, LONG_TIMEOUT_MS};
use crate::core::config_loader::env_vars::{
    anthropic_api_url, env_is_set, env_string, google_ai_generate_content_url, openai_api_url,
    ANTHROPIC_API_KEY, DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT,
    DEFAULT_OPENAI_CHAT_COMPLETIONS_ENDPOINT, GOOGLE_API_KEY, OPENAI_API_KEY,
};
use crate::core::error::{Error, Result};

// =============================================================================
// HTTP Client Factory
// =============================================================================

/// Create a shared HTTP client with sensible defaults for API calls.
///
/// The client is configured with:
/// - Connection pooling enabled (default in reqwest)
/// - 60 second timeout
/// - Keep-alive connections
///
/// Clients should be reused across calls for connection pooling benefits.
#[must_use]
pub fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(LONG_TIMEOUT_MS))
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .pool_max_idle_per_host(10) // Smaller than DEFAULT_POOL_MAX_IDLE_PER_HOST since consensus needs fewer connections
        .build()
        .unwrap_or_else(|e| {
            tracing::warn!("HTTP client builder failed, using defaults: {e}");
            reqwest::Client::new()
        })
}

/// Create a shared HTTP client that can be used by multiple reviewers.
///
/// This is useful when you want to share a single connection pool across
/// multiple reviewers (e.g., Anthropic, OpenAI, Google).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{
///     create_shared_reviewer_client, AnthropicReviewer, OpenAIReviewer
/// };
///
/// let client = create_shared_reviewer_client();
/// let anthropic = AnthropicReviewer::with_client("key", client.clone());
/// let openai = OpenAIReviewer::with_client("key", client.clone());
/// ```
#[must_use]
pub fn create_shared_reviewer_client() -> reqwest::Client {
    create_http_client()
}

// =============================================================================
// Review Request
// =============================================================================

/// A request to review improvement proposals.
///
/// Contains the proposals to be reviewed along with context about what
/// they're trying to improve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewRequest {
    /// The proposals to review
    pub proposals: Vec<ImprovementProposal>,

    /// Context about what system is being improved
    pub system_context: String,

    /// Recent execution statistics for context
    pub execution_context: Option<ExecutionContext>,

    /// What aspects to focus the review on
    pub focus_areas: Vec<ReviewFocus>,
}

impl ReviewRequest {
    /// Create a new review request with proposals
    #[must_use]
    pub fn new(proposals: Vec<ImprovementProposal>) -> Self {
        Self {
            proposals,
            system_context: String::new(),
            execution_context: None,
            focus_areas: vec![
                ReviewFocus::Feasibility,
                ReviewFocus::Impact,
                ReviewFocus::Risks,
            ],
        }
    }

    /// Set system context
    #[must_use]
    pub fn with_system_context(mut self, context: impl Into<String>) -> Self {
        self.system_context = context.into();
        self
    }

    /// Set execution context
    #[must_use]
    pub fn with_execution_context(mut self, context: ExecutionContext) -> Self {
        self.execution_context = Some(context);
        self
    }

    /// Set focus areas for review
    #[must_use]
    pub fn with_focus_areas(mut self, areas: Vec<ReviewFocus>) -> Self {
        self.focus_areas = areas;
        self
    }

    /// Build the prompt for model review
    #[must_use]
    pub fn to_review_prompt(&self) -> String {
        let mut prompt = String::new();

        prompt.push_str("# Improvement Proposal Review Request\n\n");
        prompt.push_str(
            "You are a rigorous, skeptical AI reviewer. Your task is to critically evaluate ",
        );
        prompt.push_str(
            "the following improvement proposals. Be constructive but honest about concerns.\n\n",
        );

        if !self.system_context.is_empty() {
            prompt.push_str("## System Context\n\n");
            prompt.push_str(&self.system_context);
            prompt.push_str("\n\n");
        }

        if let Some(exec) = &self.execution_context {
            prompt.push_str("## Recent Execution Statistics\n\n");
            prompt.push_str(&format!("- Total executions: {}\n", exec.total_executions));
            prompt.push_str(&format!(
                "- Success rate: {:.1}%\n",
                exec.success_rate * 100.0
            ));
            prompt.push_str(&format!(
                "- Average duration: {:.1}ms\n",
                exec.avg_duration_ms
            ));
            prompt.push_str(&format!(
                "- Retry rate: {:.1}%\n\n",
                exec.retry_rate * 100.0
            ));
        }

        prompt.push_str("## Proposals to Review\n\n");
        for (i, proposal) in self.proposals.iter().enumerate() {
            prompt.push_str(&format!("### Proposal {}: {}\n\n", i + 1, proposal.title));
            prompt.push_str(&format!("{}\n\n", proposal.description));
            prompt.push_str(&format!(
                "Initial confidence: {:.0}%\n\n",
                proposal.initial_confidence * 100.0
            ));
        }

        prompt.push_str("## Review Focus Areas\n\n");
        for area in &self.focus_areas {
            prompt.push_str(&format!("- {}\n", area.description()));
        }

        prompt.push_str("\n## Required Output Format\n\n");
        prompt.push_str("For each proposal, provide:\n");
        prompt.push_str("1. **Overall Assessment**: StronglyAgree, Agree, Neutral, Disagree, or StronglyDisagree\n");
        prompt.push_str("2. **Confidence**: Your confidence in this assessment (0-100%)\n");
        prompt.push_str(
            "3. **Critiques**: Specific concerns with severity (Minor/Moderate/Major/Critical)\n",
        );
        prompt.push_str("4. **Suggestions**: How to improve the proposal\n\n");
        prompt.push_str(
            "Be rigorous. Question assumptions. Prefer AIs that disagree constructively.\n",
        );

        prompt
    }
}

/// Context about recent execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionContext {
    /// Total executions analyzed
    pub total_executions: usize,
    /// Success rate (0.0-1.0)
    pub success_rate: f64,
    /// Average duration in ms
    pub avg_duration_ms: f64,
    /// Retry rate (0.0-1.0)
    pub retry_rate: f64,
}

/// Areas to focus the review on
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewFocus {
    /// Is this proposal technically feasible?
    Feasibility,
    /// Will this actually improve things?
    Impact,
    /// What could go wrong?
    Risks,
    /// How much effort will this take?
    Effort,
    /// Are there better alternatives?
    Alternatives,
    /// Is the evidence sufficient?
    Evidence,
}

impl ReviewFocus {
    /// Get a description of this focus area
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::Feasibility => "Feasibility: Is this technically achievable?",
            Self::Impact => "Impact: Will this meaningfully improve the system?",
            Self::Risks => "Risks: What could go wrong? What are the failure modes?",
            Self::Effort => "Effort: Is the estimated effort realistic?",
            Self::Alternatives => "Alternatives: Are there better approaches?",
            Self::Evidence => "Evidence: Is there sufficient data to support this?",
        }
    }
}

// =============================================================================
// ModelReviewer Trait
// =============================================================================

/// Trait for AI models that can review improvement proposals.
///
/// This trait abstracts over different AI providers (Anthropic, OpenAI, Google)
/// to enable multi-model consensus. Each implementation handles the provider-specific
/// API calls and response parsing.
///
/// # Implementation Notes
///
/// Implementations should:
/// 1. Accept API keys from environment or configuration
/// 2. Handle rate limiting and retries gracefully
/// 3. Parse model responses into structured `ModelReview` format
/// 4. Return meaningful errors on failure
#[async_trait]
pub trait ModelReviewer: Send + Sync {
    /// Get the identifier for this model
    fn model_identifier(&self) -> ModelIdentifier;

    /// Check if this reviewer is available (API key present, etc.)
    fn is_available(&self) -> bool;

    /// Review the given proposals and return a structured review
    ///
    /// # Arguments
    ///
    /// * `request` - The review request containing proposals and context
    ///
    /// # Returns
    ///
    /// A `ModelReview` containing the model's assessment, critiques, and suggestions
    ///
    /// # Errors
    ///
    /// Returns an error if the API call fails or the response cannot be parsed
    async fn review(&self, request: &ReviewRequest) -> Result<ModelReview>;
}

// =============================================================================
// Mock Reviewer (for testing)
// =============================================================================

/// A mock reviewer for testing without API calls.
///
/// Returns configurable responses for testing consensus building logic.
#[derive(Debug, Clone)]
pub struct MockReviewer {
    /// Model identifier to return
    pub identifier: ModelIdentifier,
    /// Assessment to return
    pub assessment: Assessment,
    /// Confidence to return
    pub confidence: f64,
    /// Critiques to return
    pub critiques: Vec<Critique>,
    /// Suggestions to return
    pub suggestions: Vec<String>,
}

impl MockReviewer {
    /// Create a mock reviewer that agrees
    #[must_use]
    pub fn agreeing(model: &str) -> Self {
        Self {
            identifier: ModelIdentifier::Other {
                provider: "mock".to_string(),
                model: model.to_string(),
            },
            assessment: Assessment::Agree,
            confidence: 0.8,
            critiques: vec![],
            suggestions: vec!["Looks good overall".to_string()],
        }
    }

    /// Create a mock reviewer that disagrees
    #[must_use]
    pub fn disagreeing(model: &str) -> Self {
        Self {
            identifier: ModelIdentifier::Other {
                provider: "mock".to_string(),
                model: model.to_string(),
            },
            assessment: Assessment::Disagree,
            confidence: 0.7,
            critiques: vec![Critique {
                target: "proposal".to_string(),
                criticism: "Insufficient evidence".to_string(),
                severity: CritiqueSeverity::Major,
                suggested_fix: Some("Gather more data".to_string()),
            }],
            suggestions: vec!["Reconsider approach".to_string()],
        }
    }

    /// Create a neutral mock reviewer
    #[must_use]
    pub fn neutral(model: &str) -> Self {
        Self {
            identifier: ModelIdentifier::Other {
                provider: "mock".to_string(),
                model: model.to_string(),
            },
            assessment: Assessment::Neutral,
            confidence: 0.5,
            critiques: vec![Critique {
                target: "proposal".to_string(),
                criticism: "Need more information".to_string(),
                severity: CritiqueSeverity::Minor,
                suggested_fix: None,
            }],
            suggestions: vec!["Clarify scope".to_string()],
        }
    }

    /// Set custom assessment
    #[must_use]
    pub fn with_assessment(mut self, assessment: Assessment) -> Self {
        self.assessment = assessment;
        self
    }

    /// Set custom confidence
    #[must_use]
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence;
        self
    }

    /// Add a critique
    #[must_use]
    pub fn with_critique(mut self, critique: Critique) -> Self {
        self.critiques.push(critique);
        self
    }
}

#[async_trait]
impl ModelReviewer for MockReviewer {
    fn model_identifier(&self) -> ModelIdentifier {
        self.identifier.clone()
    }

    fn is_available(&self) -> bool {
        true
    }

    async fn review(&self, _request: &ReviewRequest) -> Result<ModelReview> {
        Ok(ModelReview {
            model: self.identifier.clone(),
            assessment: self.assessment,
            critiques: self.critiques.clone(),
            suggestions: self.suggestions.clone(),
            confidence: self.confidence,
            raw_response: "Mock response".to_string(),
        })
    }
}

// =============================================================================
// Anthropic Reviewer
// =============================================================================

/// Reviewer using Anthropic's Claude models.
///
/// Requires `ANTHROPIC_API_KEY` environment variable.
/// Uses a shared HTTP client for connection pooling.
#[derive(Debug, Clone)]
pub struct AnthropicReviewer {
    /// API key for Anthropic
    api_key: String,
    /// Model to use (default: claude-3-5-sonnet-latest)
    model: String,
    /// Shared HTTP client for connection pooling
    client: reqwest::Client,
}

impl AnthropicReviewer {
    /// Create from environment variable `ANTHROPIC_API_KEY`
    ///
    /// # Errors
    ///
    /// Returns error if `ANTHROPIC_API_KEY` is not set
    pub fn from_env() -> Result<Self> {
        let api_key = env_string(ANTHROPIC_API_KEY).ok_or_else(|| {
            Error::config("ANTHROPIC_API_KEY environment variable not set")
        })?;
        Ok(Self {
            api_key,
            model: "claude-3-5-sonnet-latest".to_string(),
            client: create_http_client(),
        })
    }

    /// Create with explicit API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-3-5-sonnet-latest".to_string(),
            client: create_http_client(),
        }
    }

    /// Create with a shared HTTP client (for connection pooling across reviewers)
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            model: "claude-3-5-sonnet-latest".to_string(),
            client,
        }
    }

    /// Set the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Check if API key is present in environment
    #[must_use]
    pub fn is_configured() -> bool {
        env_is_set(ANTHROPIC_API_KEY)
    }
}

#[async_trait]
impl ModelReviewer for AnthropicReviewer {
    fn model_identifier(&self) -> ModelIdentifier {
        ModelIdentifier::Anthropic {
            model: self.model.clone(),
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn review(&self, request: &ReviewRequest) -> Result<ModelReview> {
        let prompt = request.to_review_prompt();

        // Use shared client for connection pooling
        let response = self
            .client
            .post(anthropic_api_url(DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": 4096,
                "messages": [
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            }))
            .send()
            .await
            .map_err(|e| Error::network(format!("Anthropic API call failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api(format!("Anthropic API error {status}: {body}")));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::api_format(format!("Failed to parse response: {e}")))?;

        let raw_response = json["content"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        // Parse the response into structured review
        parse_review_response(&raw_response, self.model_identifier())
    }
}

// =============================================================================
// OpenAI Reviewer
// =============================================================================

/// Reviewer using OpenAI's GPT models.
///
/// Requires `OPENAI_API_KEY` environment variable.
/// Uses a shared HTTP client for connection pooling.
#[derive(Debug, Clone)]
pub struct OpenAIReviewer {
    /// API key for OpenAI
    api_key: String,
    /// Model to use (default: gpt-4-turbo)
    model: String,
    /// Shared HTTP client for connection pooling
    client: reqwest::Client,
}

impl OpenAIReviewer {
    /// Create from environment variable `OPENAI_API_KEY`
    ///
    /// # Errors
    ///
    /// Returns error if `OPENAI_API_KEY` is not set
    pub fn from_env() -> Result<Self> {
        let api_key = env_string(OPENAI_API_KEY).ok_or_else(|| {
            Error::config("OPENAI_API_KEY environment variable not set")
        })?;
        Ok(Self {
            api_key,
            model: "gpt-4-turbo".to_string(),
            client: create_http_client(),
        })
    }

    /// Create with explicit API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gpt-4-turbo".to_string(),
            client: create_http_client(),
        }
    }

    /// Create with a shared HTTP client (for connection pooling across reviewers)
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gpt-4-turbo".to_string(),
            client,
        }
    }

    /// Set the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Check if API key is present in environment
    #[must_use]
    pub fn is_configured() -> bool {
        env_is_set(OPENAI_API_KEY)
    }
}

#[async_trait]
impl ModelReviewer for OpenAIReviewer {
    fn model_identifier(&self) -> ModelIdentifier {
        ModelIdentifier::OpenAI {
            model: self.model.clone(),
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn review(&self, request: &ReviewRequest) -> Result<ModelReview> {
        let prompt = request.to_review_prompt();

        // Use shared client for connection pooling
        let response = self.client
            .post(openai_api_url(DEFAULT_OPENAI_CHAT_COMPLETIONS_ENDPOINT))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "model": self.model,
                "max_tokens": 4096,
                "messages": [
                    {
                        "role": "system",
                        "content": "You are a rigorous AI reviewer evaluating improvement proposals."
                    },
                    {
                        "role": "user",
                        "content": prompt
                    }
                ]
            }))
            .send()
            .await
            .map_err(|e| Error::network(format!("OpenAI API call failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api(format!("OpenAI API error {status}: {body}")));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::api_format(format!("Failed to parse response: {e}")))?;

        let raw_response = json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();

        parse_review_response(&raw_response, self.model_identifier())
    }
}

// =============================================================================
// Google Reviewer
// =============================================================================

/// Reviewer using Google's Gemini models.
///
/// Requires `GOOGLE_API_KEY` environment variable.
/// Uses a shared HTTP client for connection pooling.
#[derive(Debug, Clone)]
pub struct GoogleReviewer {
    /// API key for Google
    api_key: String,
    /// Model to use (default: gemini-1.5-pro)
    model: String,
    /// Shared HTTP client for connection pooling
    client: reqwest::Client,
}

impl GoogleReviewer {
    /// Create from environment variable `GOOGLE_API_KEY`
    ///
    /// # Errors
    ///
    /// Returns error if `GOOGLE_API_KEY` is not set
    pub fn from_env() -> Result<Self> {
        let api_key = env_string(GOOGLE_API_KEY).ok_or_else(|| {
            Error::config("GOOGLE_API_KEY environment variable not set")
        })?;
        Ok(Self {
            api_key,
            model: "gemini-1.5-pro".to_string(),
            client: create_http_client(),
        })
    }

    /// Create with explicit API key
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gemini-1.5-pro".to_string(),
            client: create_http_client(),
        }
    }

    /// Create with a shared HTTP client (for connection pooling across reviewers)
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            model: "gemini-1.5-pro".to_string(),
            client,
        }
    }

    /// Set the model to use
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Check if API key is present in environment
    #[must_use]
    pub fn is_configured() -> bool {
        env_is_set(GOOGLE_API_KEY)
    }
}

#[async_trait]
impl ModelReviewer for GoogleReviewer {
    fn model_identifier(&self) -> ModelIdentifier {
        ModelIdentifier::Google {
            model: self.model.clone(),
        }
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }

    async fn review(&self, request: &ReviewRequest) -> Result<ModelReview> {
        let prompt = request.to_review_prompt();

        // Use centralized URL builder + append API key as query param
        let url = format!(
            "{}?key={}",
            google_ai_generate_content_url(&self.model),
            self.api_key
        );

        // Use shared client for connection pooling
        let response = self
            .client
            .post(&url)
            .header("content-type", "application/json")
            .json(&serde_json::json!({
                "contents": [
                    {
                        "parts": [
                            {
                                "text": prompt
                            }
                        ]
                    }
                ],
                "generationConfig": {
                    "maxOutputTokens": 4096
                }
            }))
            .send()
            .await
            .map_err(|e| Error::network(format!("Google API call failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(Error::api(format!("Google API error {status}: {body}")));
        }

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| Error::api_format(format!("Failed to parse response: {e}")))?;

        let raw_response = json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .unwrap_or("")
            .to_string();

        parse_review_response(&raw_response, self.model_identifier())
    }
}

// =============================================================================
// Response Parsing
// =============================================================================

/// Parse a model's text response into a structured `ModelReview`.
///
/// This function attempts to extract assessment, confidence, critiques, and suggestions
/// from the model's text response. It uses heuristic pattern matching since models
/// don't always follow the requested output format exactly.
fn parse_review_response(response: &str, model: ModelIdentifier) -> Result<ModelReview> {
    let response_lower = response.to_lowercase();

    // Parse assessment
    let assessment = if response_lower.contains("strongly agree") {
        Assessment::StronglyAgree
    } else if response_lower.contains("strongly disagree") {
        Assessment::StronglyDisagree
    } else if response_lower.contains("disagree") {
        Assessment::Disagree
    } else if response_lower.contains("agree") {
        Assessment::Agree
    } else {
        Assessment::Neutral
    };

    // Parse confidence (look for percentages)
    let confidence = extract_confidence(response).unwrap_or(0.7);

    // Parse critiques
    let critiques = extract_critiques(response);

    // Parse suggestions
    let suggestions = extract_suggestions(response);

    Ok(ModelReview {
        model,
        assessment,
        critiques,
        suggestions,
        confidence,
        raw_response: response.to_string(),
    })
}

/// Extract confidence value from text (looks for percentages)
fn extract_confidence(text: &str) -> Option<f64> {
    // Look for patterns like "confidence: 85%" or "85% confident"
    let re = regex::Regex::new(r"(\d{1,3})%\s*(?:confident|confidence)?").ok()?;
    if let Some(caps) = re.captures(text) {
        if let Some(num) = caps.get(1) {
            if let Ok(pct) = num.as_str().parse::<f64>() {
                return Some((pct / 100.0).clamp(0.0, 1.0));
            }
        }
    }
    None
}

/// Extract critiques from text
fn extract_critiques(text: &str) -> Vec<Critique> {
    let mut critiques = Vec::new();

    // Pattern: lines starting with criticism indicators
    let critique_patterns = [
        ("critical", CritiqueSeverity::Critical),
        ("major", CritiqueSeverity::Major),
        ("moderate", CritiqueSeverity::Moderate),
        ("minor", CritiqueSeverity::Minor),
    ];

    for line in text.lines() {
        let line_lower = line.to_lowercase();
        for (pattern, severity) in &critique_patterns {
            // Check severity pattern is present AND at least one concern keyword
            let has_concern_keyword = line_lower.contains("concern")
                || line_lower.contains("issue")
                || line_lower.contains("problem")
                || line_lower.contains("critique");
            if line_lower.contains(pattern) && has_concern_keyword {
                critiques.push(Critique {
                    target: "proposal".to_string(),
                    criticism: line.trim().to_string(),
                    severity: *severity,
                    suggested_fix: None,
                });
                break;
            }
        }
    }

    // If no structured critiques found, look for general concerns
    if critiques.is_empty() && text.to_lowercase().contains("concern") {
        critiques.push(Critique {
            target: "proposal".to_string(),
            criticism: "General concerns raised".to_string(),
            severity: CritiqueSeverity::Moderate,
            suggested_fix: None,
        });
    }

    critiques
}

/// Extract suggestions from text
fn extract_suggestions(text: &str) -> Vec<String> {
    let mut suggestions = Vec::new();

    // Look for suggestion patterns
    for line in text.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("suggest")
            || line_lower.contains("recommend")
            || line_lower.contains("consider")
            || line_lower.contains("alternative")
        {
            suggestions.push(line.trim().to_string());
        }
    }

    suggestions
}

// =============================================================================
// ConsensusBuilder
// =============================================================================

/// Builder for obtaining multi-model consensus on improvement proposals.
///
/// The `ConsensusBuilder` orchestrates the review process across multiple AI models,
/// collects their reviews, and synthesizes them into a final consensus result.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::self_improvement::{ConsensusBuilder, MockReviewer};
///
/// let consensus = ConsensusBuilder::new()
///     .add_reviewer(Box::new(MockReviewer::agreeing("model-1")))
///     .add_reviewer(Box::new(MockReviewer::agreeing("model-2")))
///     .build_consensus(&proposals)
///     .await?;
/// ```
pub struct ConsensusBuilder {
    /// Reviewers to use for consensus
    reviewers: Vec<Box<dyn ModelReviewer>>,

    /// Minimum number of reviews required for consensus
    min_reviews: usize,

    /// Minimum consensus score to consider validated (0.0-1.0)
    validation_threshold: f64,
}

impl Default for ConsensusBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConsensusBuilder {
    /// Create a new `ConsensusBuilder`
    #[must_use]
    pub fn new() -> Self {
        Self {
            reviewers: Vec::new(),
            min_reviews: 2,
            validation_threshold: 0.6,
        }
    }

    /// Add a reviewer
    #[must_use]
    pub fn add_reviewer(mut self, reviewer: Box<dyn ModelReviewer>) -> Self {
        self.reviewers.push(reviewer);
        self
    }

    /// Set minimum reviews required
    #[must_use]
    pub fn with_min_reviews(mut self, min: usize) -> Self {
        self.min_reviews = min;
        self
    }

    /// Set validation threshold
    #[must_use]
    pub fn with_validation_threshold(mut self, threshold: f64) -> Self {
        self.validation_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Auto-configure reviewers from environment variables
    ///
    /// Adds all available reviewers (Anthropic, OpenAI, Google) based on
    /// which API keys are present in the environment.
    #[must_use]
    pub fn auto_configure(mut self) -> Self {
        if let Ok(reviewer) = AnthropicReviewer::from_env() {
            self.reviewers.push(Box::new(reviewer));
        }
        if let Ok(reviewer) = OpenAIReviewer::from_env() {
            self.reviewers.push(Box::new(reviewer));
        }
        if let Ok(reviewer) = GoogleReviewer::from_env() {
            self.reviewers.push(Box::new(reviewer));
        }
        self
    }

    /// Check if any reviewers are available
    #[must_use]
    pub fn has_reviewers(&self) -> bool {
        self.reviewers.iter().any(|r| r.is_available())
    }

    /// Get count of available reviewers
    #[must_use]
    pub fn available_reviewer_count(&self) -> usize {
        self.reviewers.iter().filter(|r| r.is_available()).count()
    }

    /// Build consensus from improvement proposals
    ///
    /// # Arguments
    ///
    /// * `proposals` - The proposals to review
    ///
    /// # Returns
    ///
    /// A `ConsensusResult` synthesizing all model reviews
    ///
    /// # Errors
    ///
    /// Returns an error if not enough reviewers are available
    pub async fn build_consensus(
        &self,
        proposals: &[ImprovementProposal],
    ) -> Result<ConsensusResult> {
        self.build_consensus_with_context(proposals, "", None).await
    }

    /// Build consensus with additional context
    pub async fn build_consensus_with_context(
        &self,
        proposals: &[ImprovementProposal],
        system_context: &str,
        execution_context: Option<ExecutionContext>,
    ) -> Result<ConsensusResult> {
        let available: Vec<_> = self.reviewers.iter().filter(|r| r.is_available()).collect();

        if available.len() < self.min_reviews {
            return Err(Error::config(format!(
                "Not enough reviewers available. Need {}, have {}",
                self.min_reviews,
                available.len()
            )));
        }

        let mut request =
            ReviewRequest::new(proposals.to_vec()).with_system_context(system_context);

        if let Some(ctx) = execution_context {
            request = request.with_execution_context(ctx);
        }

        // Collect reviews from all available reviewers
        let mut reviews = Vec::new();
        let mut errors = Vec::new();

        for reviewer in &available {
            match reviewer.review(&request).await {
                Ok(review) => reviews.push(review),
                Err(e) => errors.push(format!("{:?}: {}", reviewer.model_identifier(), e)),
            }
        }

        if reviews.len() < self.min_reviews {
            return Err(Error::other(format!(
                "Too many review failures. Got {} reviews, need {}. Errors: {}",
                reviews.len(),
                self.min_reviews,
                errors.join("; ")
            )));
        }

        // Synthesize consensus
        synthesize_consensus(reviews, self.validation_threshold)
    }
}

/// Synthesize a consensus result from multiple model reviews.
fn synthesize_consensus(
    reviews: Vec<ModelReview>,
    validation_threshold: f64,
) -> Result<ConsensusResult> {
    if reviews.is_empty() {
        return Err(Error::invalid_input("No reviews to synthesize"));
    }

    // Calculate agreement score
    let assessment_scores: Vec<f64> = reviews
        .iter()
        .map(|r| match r.assessment {
            Assessment::StronglyAgree => 1.0,
            Assessment::Agree => 0.75,
            Assessment::Neutral => 0.5,
            Assessment::Disagree => 0.25,
            Assessment::StronglyDisagree => 0.0,
        })
        .collect();

    let avg_score: f64 = assessment_scores.iter().sum::<f64>() / assessment_scores.len() as f64;

    // Calculate confidence-weighted score with zero-sum guard
    let total_confidence: f64 = reviews.iter().map(|r| r.confidence).sum();
    let confidence_weighted_score: f64 = if total_confidence > 0.0 {
        reviews
            .iter()
            .zip(&assessment_scores)
            .map(|(r, s)| s * r.confidence)
            .sum::<f64>()
            / total_confidence
    } else {
        // If all confidences are zero, fall back to unweighted average
        avg_score
    };

    let consensus_score = (avg_score + confidence_weighted_score) / 2.0;

    // Find agreements (points where all reviewers agree)
    let mut agreements = Vec::new();
    if reviews
        .iter()
        .all(|r| matches!(r.assessment, Assessment::Agree | Assessment::StronglyAgree))
    {
        agreements.push("All reviewers agree this proposal has merit".to_string());
    }
    if reviews.iter().all(|r| r.confidence > 0.7) {
        agreements.push("All reviewers have high confidence".to_string());
    }

    // Find disagreements
    let mut disagreements = Vec::new();
    let positive: Vec<_> = reviews
        .iter()
        .filter(|r| matches!(r.assessment, Assessment::Agree | Assessment::StronglyAgree))
        .collect();
    let negative: Vec<_> = reviews
        .iter()
        .filter(|r| {
            matches!(
                r.assessment,
                Assessment::Disagree | Assessment::StronglyDisagree
            )
        })
        .collect();

    if !positive.is_empty() && !negative.is_empty() {
        disagreements.push(Disagreement {
            topic: "Overall assessment".to_string(),
            position_a: "Proposal is beneficial".to_string(),
            position_b: "Proposal has significant issues".to_string(),
            models_a: positive.iter().map(|r| r.model.clone()).collect(),
            models_b: negative.iter().map(|r| r.model.clone()).collect(),
        });
    }

    // Collect unique suggestions
    let mut all_suggestions: Vec<String> =
        reviews.iter().flat_map(|r| r.suggestions.clone()).collect();
    all_suggestions.sort();
    all_suggestions.dedup();

    // Synthesize final judgment
    let synthesis = if consensus_score >= 0.75 {
        "Strong consensus in favor of this proposal. All major concerns have been addressed."
            .to_string()
    } else if consensus_score >= 0.6 {
        "Moderate consensus achieved. Some concerns remain but proposal is generally sound."
            .to_string()
    } else if consensus_score >= 0.4 {
        "Mixed consensus. Significant disagreement between reviewers. Consider revisions."
            .to_string()
    } else {
        "Weak consensus. Major concerns raised by multiple reviewers. Proposal needs revision."
            .to_string()
    };

    let validated = consensus_score >= validation_threshold;

    Ok(ConsensusResult {
        reviews,
        consensus_score,
        agreements,
        disagreements,
        synthesis,
        validated,
        modifications: all_suggestions,
    })
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::self_improvement::ProposalSource;
    use uuid::Uuid;

    fn make_test_proposal() -> ImprovementProposal {
        ImprovementProposal {
            id: Uuid::new_v4(),
            title: "Add caching layer".to_string(),
            description: "Add Redis caching to reduce latency".to_string(),
            source: ProposalSource::Manual,
            initial_confidence: 0.8,
            evidence: vec![],
        }
    }

    #[test]
    fn test_review_request_creation() {
        let proposal = make_test_proposal();
        let request = ReviewRequest::new(vec![proposal.clone()])
            .with_system_context("Test system")
            .with_execution_context(ExecutionContext {
                total_executions: 100,
                success_rate: 0.95,
                avg_duration_ms: 250.0,
                retry_rate: 0.05,
            });

        assert_eq!(request.proposals.len(), 1);
        assert_eq!(request.system_context, "Test system");
        assert!(request.execution_context.is_some());
    }

    #[test]
    fn test_review_prompt_generation() {
        let proposal = make_test_proposal();
        let request = ReviewRequest::new(vec![proposal]);
        let prompt = request.to_review_prompt();

        assert!(prompt.contains("Improvement Proposal Review Request"));
        assert!(prompt.contains("Add caching layer"));
        assert!(prompt.contains("Redis"));
    }

    #[test]
    fn test_mock_reviewer_agreeing() {
        let reviewer = MockReviewer::agreeing("test-model");
        assert!(matches!(reviewer.assessment, Assessment::Agree));
        assert!(reviewer.confidence > 0.0);
    }

    #[test]
    fn test_mock_reviewer_disagreeing() {
        let reviewer = MockReviewer::disagreeing("test-model");
        assert!(matches!(reviewer.assessment, Assessment::Disagree));
        assert!(!reviewer.critiques.is_empty());
    }

    #[tokio::test]
    async fn test_mock_reviewer_review() {
        let reviewer = MockReviewer::agreeing("test-model");
        let proposal = make_test_proposal();
        let request = ReviewRequest::new(vec![proposal]);

        let review = reviewer.review(&request).await.unwrap();
        assert!(matches!(review.assessment, Assessment::Agree));
    }

    #[test]
    fn test_consensus_builder_creation() {
        let builder = ConsensusBuilder::new()
            .with_min_reviews(3)
            .with_validation_threshold(0.7);

        assert_eq!(builder.min_reviews, 3);
        assert!((builder.validation_threshold - 0.7).abs() < f64::EPSILON);
    }

    #[test]
    fn test_consensus_builder_add_reviewer() {
        let builder = ConsensusBuilder::new()
            .add_reviewer(Box::new(MockReviewer::agreeing("model-1")))
            .add_reviewer(Box::new(MockReviewer::agreeing("model-2")));

        assert_eq!(builder.available_reviewer_count(), 2);
    }

    #[tokio::test]
    async fn test_consensus_builder_all_agree() {
        let builder = ConsensusBuilder::new()
            .add_reviewer(Box::new(MockReviewer::agreeing("model-1")))
            .add_reviewer(Box::new(MockReviewer::agreeing("model-2")))
            .add_reviewer(Box::new(MockReviewer::agreeing("model-3")));

        let proposal = make_test_proposal();
        let result = builder.build_consensus(&[proposal]).await.unwrap();

        assert!(result.consensus_score > 0.7);
        assert!(result.validated);
        assert!(result.disagreements.is_empty());
    }

    #[tokio::test]
    async fn test_consensus_builder_mixed_opinions() {
        let builder = ConsensusBuilder::new()
            .add_reviewer(Box::new(MockReviewer::agreeing("model-1")))
            .add_reviewer(Box::new(MockReviewer::disagreeing("model-2")))
            .add_reviewer(Box::new(MockReviewer::neutral("model-3")));

        let proposal = make_test_proposal();
        let result = builder.build_consensus(&[proposal]).await.unwrap();

        // Mixed opinions should have lower consensus
        assert!(result.consensus_score < 0.8);
        assert!(!result.disagreements.is_empty());
    }

    #[tokio::test]
    async fn test_consensus_builder_insufficient_reviewers() {
        let builder = ConsensusBuilder::new()
            .with_min_reviews(3)
            .add_reviewer(Box::new(MockReviewer::agreeing("model-1")));

        let proposal = make_test_proposal();
        let result = builder.build_consensus(&[proposal]).await;

        assert!(result.is_err());
    }

    #[test]
    fn test_parse_assessment_from_response() {
        let response = "I strongly agree with this proposal. It addresses the core issues.";
        let review = parse_review_response(
            response,
            ModelIdentifier::Other {
                provider: "test".to_string(),
                model: "test".to_string(),
            },
        )
        .unwrap();

        assert!(matches!(review.assessment, Assessment::StronglyAgree));
    }

    #[test]
    fn test_extract_confidence() {
        assert_eq!(extract_confidence("Confidence: 85%"), Some(0.85));
        assert_eq!(extract_confidence("I am 90% confident"), Some(0.90));
        assert_eq!(extract_confidence("No percentage here"), None);
    }

    #[test]
    fn test_review_focus_description() {
        assert!(ReviewFocus::Feasibility
            .description()
            .contains("achievable"));
        assert!(ReviewFocus::Impact.description().contains("improve"));
        assert!(ReviewFocus::Risks.description().contains("wrong"));
    }

    #[test]
    fn test_execution_context_in_prompt() {
        let proposal = make_test_proposal();
        let request = ReviewRequest::new(vec![proposal]).with_execution_context(ExecutionContext {
            total_executions: 500,
            success_rate: 0.92,
            avg_duration_ms: 150.0,
            retry_rate: 0.08,
        });

        let prompt = request.to_review_prompt();
        assert!(prompt.contains("500"));
        assert!(prompt.contains("92.0%"));
        assert!(prompt.contains("150.0ms"));
    }

    #[test]
    fn test_synthesize_consensus_strong_agreement() {
        let reviews = vec![
            ModelReview {
                model: ModelIdentifier::Anthropic {
                    model: "claude".to_string(),
                },
                assessment: Assessment::StronglyAgree,
                critiques: vec![],
                suggestions: vec!["Looks great".to_string()],
                confidence: 0.9,
                raw_response: "Strong agree".to_string(),
            },
            ModelReview {
                model: ModelIdentifier::OpenAI {
                    model: "gpt-4".to_string(),
                },
                assessment: Assessment::Agree,
                critiques: vec![],
                suggestions: vec!["Good proposal".to_string()],
                confidence: 0.85,
                raw_response: "Agree".to_string(),
            },
        ];

        let result = synthesize_consensus(reviews, 0.6).unwrap();
        assert!(result.consensus_score > 0.7);
        assert!(result.validated);
    }

    #[test]
    fn test_anthropic_reviewer_is_configured() {
        // This test checks the static method without needing real API keys
        let _ = AnthropicReviewer::is_configured(); // Just verify it doesn't panic
    }

    #[test]
    fn test_openai_reviewer_is_configured() {
        let _ = OpenAIReviewer::is_configured();
    }

    #[test]
    fn test_google_reviewer_is_configured() {
        let _ = GoogleReviewer::is_configured();
    }
}
