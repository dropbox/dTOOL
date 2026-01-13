//! # DashFlow Prompt Management
//!
//! Prompt management for LLM applications. Provides versioned prompt storage,
//! registry with retrieval, performance tracking, and A/B testing support.
//!
//! ## Features
//!
//! - **Prompt Registry**: Store and retrieve prompts by name and version
//! - **Version Management**: Semantic versioning with version history
//! - **Performance Tracking**: Track metrics (latency, token usage, success rate)
//! - **A/B Testing**: Compare prompt variants with statistical significance
//!
//! ## Example
//!
//! ```
//! use dashflow_prompts::{PromptRegistry, Prompt, PromptMetadata};
//! use std::path::PathBuf;
//!
//! # async fn example() -> Result<(), dashflow_prompts::PromptError> {
//! // Create registry with file storage
//! let registry = PromptRegistry::new(PathBuf::from("/tmp/prompts"));
//!
//! // Register a prompt
//! let prompt = Prompt::new(
//!     "code_review",
//!     "1.0.0",
//!     "Review this code for bugs and improvements:\n\n{{code}}",
//! );
//! registry.register(prompt).await?;
//!
//! // Get the best performing version
//! let best = registry.get_best("code_review").await?;
//! println!("Using prompt version: {}", best.version());
//!
//! // Record performance
//! registry.record_execution("code_review", "1.0.0", 150, 500, true).await?;
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Errors that can occur during prompt management
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PromptError {
    /// Prompt not found
    #[error("Prompt not found: {0}")]
    NotFound(String),

    /// Version not found
    #[error("Version not found: {0} v{1}")]
    VersionNotFound(String, String),

    /// Invalid version string
    #[error("Invalid version: {0}")]
    InvalidVersion(String),

    /// Storage error
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// IO error
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    /// Prompt already exists
    #[error("Prompt already exists: {0} v{1}")]
    AlreadyExists(String, String),

    /// No versions available
    #[error("No versions available for prompt: {0}")]
    NoVersions(String),
}

/// Prompt metadata for tracking and discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptMetadata {
    /// Human-readable description
    pub description: Option<String>,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Author name
    pub author: Option<String>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last updated timestamp
    pub updated_at: DateTime<Utc>,
    /// Custom key-value metadata
    pub custom: HashMap<String, String>,
}

impl Default for PromptMetadata {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            description: None,
            tags: Vec::new(),
            author: None,
            created_at: now,
            updated_at: now,
            custom: HashMap::new(),
        }
    }
}

impl PromptMetadata {
    /// Create new metadata with description
    #[must_use]
    pub fn with_description(description: impl Into<String>) -> Self {
        Self {
            description: Some(description.into()),
            ..Default::default()
        }
    }

    /// Add a tag
    #[must_use]
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    /// Add tags
    #[must_use]
    pub fn with_tags(mut self, tags: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.tags.extend(tags.into_iter().map(Into::into));
        self
    }

    /// Set author
    #[must_use]
    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = Some(author.into());
        self
    }

    /// Add custom metadata
    #[must_use]
    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }
}

/// Performance metrics for a prompt execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Unique execution ID
    pub execution_id: Uuid,
    /// Prompt name
    pub prompt_name: String,
    /// Prompt version
    pub version: String,
    /// Latency in milliseconds
    pub latency_ms: u64,
    /// Token count (input + output)
    pub token_count: u64,
    /// Whether execution succeeded
    pub success: bool,
    /// Optional error message
    pub error: Option<String>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Custom metrics
    pub custom: HashMap<String, f64>,
}

impl ExecutionMetrics {
    /// Create new execution metrics
    #[must_use]
    pub fn new(
        prompt_name: impl Into<String>,
        version: impl Into<String>,
        latency_ms: u64,
        token_count: u64,
        success: bool,
    ) -> Self {
        Self {
            execution_id: Uuid::new_v4(),
            prompt_name: prompt_name.into(),
            version: version.into(),
            latency_ms,
            token_count,
            success,
            error: None,
            timestamp: Utc::now(),
            custom: HashMap::new(),
        }
    }

    /// Add error message
    #[must_use]
    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.error = Some(error.into());
        self.success = false;
        self
    }

    /// Add custom metric
    #[must_use]
    pub fn with_metric(mut self, name: impl Into<String>, value: f64) -> Self {
        self.custom.insert(name.into(), value);
        self
    }
}

/// Aggregated performance statistics for a prompt version
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerformanceStats {
    /// Total number of executions
    pub total_executions: u64,
    /// Successful executions
    pub successful_executions: u64,
    /// Failed executions
    pub failed_executions: u64,
    /// Success rate (0.0 to 1.0)
    pub success_rate: f64,
    /// Average latency in milliseconds
    pub avg_latency_ms: f64,
    /// P50 latency
    pub p50_latency_ms: f64,
    /// P95 latency
    pub p95_latency_ms: f64,
    /// P99 latency
    pub p99_latency_ms: f64,
    /// Average token count
    pub avg_token_count: f64,
    /// Total token count
    pub total_token_count: u64,
    /// First execution timestamp
    pub first_execution: Option<DateTime<Utc>>,
    /// Last execution timestamp
    pub last_execution: Option<DateTime<Utc>>,
}

impl PerformanceStats {
    /// Calculate stats from a list of metrics
    #[must_use]
    pub fn from_metrics(metrics: &[ExecutionMetrics]) -> Self {
        if metrics.is_empty() {
            return Self::default();
        }

        let total = metrics.len() as u64;
        let successful = metrics.iter().filter(|m| m.success).count() as u64;
        let failed = total - successful;

        let mut latencies: Vec<u64> = metrics.iter().map(|m| m.latency_ms).collect();
        latencies.sort_unstable();

        let avg_latency = latencies.iter().sum::<u64>() as f64 / total as f64;
        let p50 = percentile(&latencies, 50) as f64;
        let p95 = percentile(&latencies, 95) as f64;
        let p99 = percentile(&latencies, 99) as f64;

        let total_tokens: u64 = metrics.iter().map(|m| m.token_count).sum();
        let avg_tokens = total_tokens as f64 / total as f64;

        let timestamps: Vec<_> = metrics.iter().map(|m| m.timestamp).collect();
        let first = timestamps.iter().min().copied();
        let last = timestamps.iter().max().copied();

        Self {
            total_executions: total,
            successful_executions: successful,
            failed_executions: failed,
            success_rate: successful as f64 / total as f64,
            avg_latency_ms: avg_latency,
            p50_latency_ms: p50,
            p95_latency_ms: p95,
            p99_latency_ms: p99,
            avg_token_count: avg_tokens,
            total_token_count: total_tokens,
            first_execution: first,
            last_execution: last,
        }
    }

    /// Calculate a performance score (higher is better)
    ///
    /// Combines success rate, latency, and token efficiency
    #[must_use]
    pub fn score(&self) -> f64 {
        if self.total_executions == 0 {
            return 0.0;
        }

        // Weight: 60% success rate, 25% latency (inverse), 15% token efficiency (inverse)
        let success_score = self.success_rate * 60.0;

        // Latency score: 100ms = 25 points, scales down logarithmically
        let latency_score = if self.avg_latency_ms > 0.0 {
            (25.0 / (1.0 + (self.avg_latency_ms / 100.0).ln().max(0.0))).min(25.0)
        } else {
            25.0
        };

        // Token score: fewer tokens is better, 500 tokens = 15 points
        let token_score = if self.avg_token_count > 0.0 {
            (15.0 / (1.0 + (self.avg_token_count / 500.0).ln().max(0.0))).min(15.0)
        } else {
            15.0
        };

        success_score + latency_score + token_score
    }
}

/// Calculate percentile from sorted values
fn percentile(sorted: &[u64], p: u8) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let idx = ((p as usize) * sorted.len() / 100).min(sorted.len() - 1);
    sorted[idx]
}

/// A versioned prompt with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    /// Unique prompt name/identifier
    name: String,
    /// Semantic version
    version: String,
    /// Prompt template content
    template: String,
    /// Associated metadata
    metadata: PromptMetadata,
    /// Input variables required by the template
    input_variables: Vec<String>,
    /// Whether this version is active (can be used)
    active: bool,
}

impl Prompt {
    /// Create a new prompt
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        template: impl Into<String>,
    ) -> Self {
        let template = template.into();
        let input_variables = extract_variables(&template);

        Self {
            name: name.into(),
            version: version.into(),
            template,
            metadata: PromptMetadata::default(),
            input_variables,
            active: true,
        }
    }

    /// Create a prompt with metadata
    #[must_use]
    pub fn with_metadata(mut self, metadata: PromptMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Set active status
    #[must_use]
    pub fn with_active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Get prompt name
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get prompt version
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Get parsed semantic version
    pub fn parsed_version(&self) -> Result<Version, PromptError> {
        Version::parse(&self.version)
            .map_err(|e| PromptError::InvalidVersion(format!("{}: {}", self.version, e)))
    }

    /// Get template content
    #[must_use]
    pub fn template(&self) -> &str {
        &self.template
    }

    /// Get metadata
    #[must_use]
    pub fn metadata(&self) -> &PromptMetadata {
        &self.metadata
    }

    /// Get input variables
    #[must_use]
    pub fn input_variables(&self) -> &[String] {
        &self.input_variables
    }

    /// Check if prompt is active
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Format the template with provided values
    pub fn format(&self, values: &HashMap<String, String>) -> Result<String, PromptError> {
        let mut result = self.template.clone();

        for var in &self.input_variables {
            let placeholder = format!("{{{{{}}}}}", var);
            if let Some(value) = values.get(var) {
                result = result.replace(&placeholder, value);
            }
        }

        Ok(result)
    }
}

/// Extract variable names from a template ({{var}} format)
fn extract_variables(template: &str) -> Vec<String> {
    let mut vars = Vec::new();
    let mut i = 0;
    let bytes = template.as_bytes();

    while i < bytes.len().saturating_sub(3) {
        if bytes[i] == b'{' && bytes[i + 1] == b'{' {
            let start = i + 2;
            let mut end = start;
            while end < bytes.len() && bytes[end] != b'}' {
                end += 1;
            }
            if end + 1 < bytes.len() && bytes[end] == b'}' && bytes[end + 1] == b'}' {
                let var = String::from_utf8_lossy(&bytes[start..end])
                    .trim()
                    .to_string();
                if !var.is_empty() && !vars.contains(&var) {
                    vars.push(var);
                }
                i = end + 2;
                continue;
            }
        }
        i += 1;
    }

    vars
}

/// Storage backend trait for prompt persistence
#[async_trait]
pub trait PromptStorage: Send + Sync {
    /// Save a prompt
    async fn save(&self, prompt: &Prompt) -> Result<(), PromptError>;

    /// Load a prompt by name and version
    async fn load(&self, name: &str, version: &str) -> Result<Prompt, PromptError>;

    /// List all versions of a prompt
    async fn list_versions(&self, name: &str) -> Result<Vec<String>, PromptError>;

    /// List all prompt names
    async fn list_prompts(&self) -> Result<Vec<String>, PromptError>;

    /// Delete a prompt version
    async fn delete(&self, name: &str, version: &str) -> Result<(), PromptError>;

    /// Save execution metrics
    async fn save_metrics(&self, metrics: &ExecutionMetrics) -> Result<(), PromptError>;

    /// Load metrics for a prompt version
    async fn load_metrics(
        &self,
        name: &str,
        version: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ExecutionMetrics>, PromptError>;
}

/// In-memory storage backend (for testing)
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    prompts: RwLock<HashMap<String, HashMap<String, Prompt>>>,
    metrics: RwLock<HashMap<String, Vec<ExecutionMetrics>>>,
}

impl InMemoryStorage {
    /// Create new in-memory storage
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl PromptStorage for InMemoryStorage {
    async fn save(&self, prompt: &Prompt) -> Result<(), PromptError> {
        let mut prompts = self.prompts.write().await;
        let versions = prompts.entry(prompt.name.clone()).or_default();
        versions.insert(prompt.version.clone(), prompt.clone());
        Ok(())
    }

    async fn load(&self, name: &str, version: &str) -> Result<Prompt, PromptError> {
        let prompts = self.prompts.read().await;
        prompts
            .get(name)
            .and_then(|v| v.get(version))
            .cloned()
            .ok_or_else(|| PromptError::VersionNotFound(name.to_string(), version.to_string()))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, PromptError> {
        let prompts = self.prompts.read().await;
        Ok(prompts
            .get(name)
            .map(|v| v.keys().cloned().collect())
            .unwrap_or_default())
    }

    async fn list_prompts(&self) -> Result<Vec<String>, PromptError> {
        let prompts = self.prompts.read().await;
        Ok(prompts.keys().cloned().collect())
    }

    async fn delete(&self, name: &str, version: &str) -> Result<(), PromptError> {
        let mut prompts = self.prompts.write().await;
        if let Some(versions) = prompts.get_mut(name) {
            versions.remove(version);
        }
        Ok(())
    }

    async fn save_metrics(&self, metrics: &ExecutionMetrics) -> Result<(), PromptError> {
        let key = format!("{}:{}", metrics.prompt_name, metrics.version);
        let mut all_metrics = self.metrics.write().await;
        all_metrics.entry(key).or_default().push(metrics.clone());
        Ok(())
    }

    async fn load_metrics(
        &self,
        name: &str,
        version: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ExecutionMetrics>, PromptError> {
        let key = format!("{}:{}", name, version);
        let all_metrics = self.metrics.read().await;
        let metrics = all_metrics.get(&key).cloned().unwrap_or_default();

        Ok(match limit {
            Some(n) => metrics.into_iter().rev().take(n).collect(),
            None => metrics,
        })
    }
}

/// File-based storage backend
pub struct FileStorage {
    base_path: PathBuf,
}

impl FileStorage {
    /// Create new file storage at the given path
    pub fn new(base_path: PathBuf) -> Self {
        Self { base_path }
    }

    fn prompt_path(&self, name: &str, version: &str) -> PathBuf {
        self.base_path
            .join("prompts")
            .join(name)
            .join(format!("{}.json", version))
    }

    fn metrics_path(&self, name: &str, version: &str) -> PathBuf {
        self.base_path
            .join("metrics")
            .join(name)
            .join(format!("{}.jsonl", version))
    }
}

#[async_trait]
impl PromptStorage for FileStorage {
    async fn save(&self, prompt: &Prompt) -> Result<(), PromptError> {
        let path = self.prompt_path(&prompt.name, &prompt.version);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(prompt)
            .map_err(|e| PromptError::SerializationError(e.to_string()))?;

        tokio::fs::write(path, content).await?;
        Ok(())
    }

    async fn load(&self, name: &str, version: &str) -> Result<Prompt, PromptError> {
        let path = self.prompt_path(name, version);
        let content = tokio::fs::read_to_string(&path)
            .await
            .map_err(|e| PromptError::VersionNotFound(format!("{name}: {e}"), version.to_string()))?;

        serde_json::from_str(&content).map_err(|e| PromptError::SerializationError(e.to_string()))
    }

    async fn list_versions(&self, name: &str) -> Result<Vec<String>, PromptError> {
        let dir = self.base_path.join("prompts").join(name);
        if !tokio::fs::try_exists(&dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    versions.push(name.trim_end_matches(".json").to_string());
                }
            }
        }

        Ok(versions)
    }

    async fn list_prompts(&self) -> Result<Vec<String>, PromptError> {
        let dir = self.base_path.join("prompts");
        if !tokio::fs::try_exists(&dir).await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let mut names = Vec::new();
        let mut entries = tokio::fs::read_dir(&dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }

        Ok(names)
    }

    async fn delete(&self, name: &str, version: &str) -> Result<(), PromptError> {
        let path = self.prompt_path(name, version);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            tokio::fs::remove_file(path).await?;
        }
        Ok(())
    }

    async fn save_metrics(&self, metrics: &ExecutionMetrics) -> Result<(), PromptError> {
        let path = self.metrics_path(&metrics.prompt_name, &metrics.version);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let line = serde_json::to_string(metrics)
            .map_err(|e| PromptError::SerializationError(e.to_string()))?;

        use tokio::io::AsyncWriteExt;
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;
        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;

        Ok(())
    }

    async fn load_metrics(
        &self,
        name: &str,
        version: &str,
        limit: Option<usize>,
    ) -> Result<Vec<ExecutionMetrics>, PromptError> {
        let path = self.metrics_path(name, version);
        if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(Vec::new());
        }

        let content = tokio::fs::read_to_string(&path).await?;
        let mut metrics: Vec<ExecutionMetrics> = content
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();

        if let Some(n) = limit {
            metrics = metrics.into_iter().rev().take(n).collect();
        }

        Ok(metrics)
    }
}

/// Prompt registry for managing versioned prompts
pub struct PromptRegistry {
    storage: Arc<dyn PromptStorage>,
    /// Cache of prompts (name -> version -> prompt)
    cache: RwLock<HashMap<String, HashMap<String, Prompt>>>,
}

impl PromptRegistry {
    /// Create a new registry with file storage
    #[must_use]
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            storage: Arc::new(FileStorage::new(base_path)),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new registry with custom storage
    #[must_use]
    pub fn with_storage(storage: Arc<dyn PromptStorage>) -> Self {
        Self {
            storage,
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new registry with in-memory storage (for testing)
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            storage: Arc::new(InMemoryStorage::new()),
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new prompt
    pub async fn register(&self, prompt: Prompt) -> Result<(), PromptError> {
        // Validate version
        let _ = prompt.parsed_version()?;

        // Save to storage
        self.storage.save(&prompt).await?;

        // Update cache
        let mut cache = self.cache.write().await;
        let versions = cache.entry(prompt.name.clone()).or_default();
        versions.insert(prompt.version.clone(), prompt);

        Ok(())
    }

    /// Get a specific prompt version
    pub async fn get(&self, name: &str, version: &str) -> Result<Prompt, PromptError> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(versions) = cache.get(name) {
                if let Some(prompt) = versions.get(version) {
                    return Ok(prompt.clone());
                }
            }
        }

        // Load from storage
        let prompt = self.storage.load(name, version).await?;

        // Update cache
        let mut cache = self.cache.write().await;
        let versions = cache.entry(name.to_string()).or_default();
        versions.insert(version.to_string(), prompt.clone());

        Ok(prompt)
    }

    /// Get the latest version of a prompt
    pub async fn get_latest(&self, name: &str) -> Result<Prompt, PromptError> {
        let versions = self.storage.list_versions(name).await?;
        if versions.is_empty() {
            return Err(PromptError::NoVersions(name.to_string()));
        }

        // Parse and sort versions
        let mut parsed: Vec<(Version, String)> = versions
            .into_iter()
            .filter_map(|v| Version::parse(&v).ok().map(|parsed| (parsed, v)))
            .collect();

        parsed.sort_by(|a, b| b.0.cmp(&a.0));

        let latest = parsed
            .first()
            .ok_or_else(|| PromptError::NoVersions(name.to_string()))?;

        self.get(name, &latest.1).await
    }

    /// Get the best performing version of a prompt
    ///
    /// Considers success rate, latency, and token usage
    pub async fn get_best(&self, name: &str) -> Result<Prompt, PromptError> {
        let versions = self.storage.list_versions(name).await?;
        if versions.is_empty() {
            return Err(PromptError::NoVersions(name.to_string()));
        }

        let mut best_version: Option<(String, f64)> = None;

        for version in &versions {
            let metrics = self.storage.load_metrics(name, version, None).await?;
            if metrics.is_empty() {
                continue;
            }

            let stats = PerformanceStats::from_metrics(&metrics);
            let score = stats.score();

            if best_version.as_ref().map_or(true, |(_, best_score)| score > *best_score) {
                best_version = Some((version.clone(), score));
            }
        }

        // If no metrics, fall back to latest
        let version = best_version
            .map(|(v, _)| v)
            .ok_or_else(|| PromptError::NoVersions(name.to_string()))?;

        self.get(name, &version).await
    }

    /// List all versions of a prompt
    pub async fn list_versions(&self, name: &str) -> Result<Vec<String>, PromptError> {
        self.storage.list_versions(name).await
    }

    /// List all prompts
    pub async fn list_prompts(&self) -> Result<Vec<String>, PromptError> {
        self.storage.list_prompts().await
    }

    /// Delete a prompt version
    pub async fn delete(&self, name: &str, version: &str) -> Result<(), PromptError> {
        self.storage.delete(name, version).await?;

        // Update cache
        let mut cache = self.cache.write().await;
        if let Some(versions) = cache.get_mut(name) {
            versions.remove(version);
        }

        Ok(())
    }

    /// Record an execution of a prompt
    pub async fn record_execution(
        &self,
        name: &str,
        version: &str,
        latency_ms: u64,
        token_count: u64,
        success: bool,
    ) -> Result<(), PromptError> {
        let metrics = ExecutionMetrics::new(name, version, latency_ms, token_count, success);
        self.storage.save_metrics(&metrics).await
    }

    /// Record execution with full metrics
    pub async fn record_metrics(&self, metrics: ExecutionMetrics) -> Result<(), PromptError> {
        self.storage.save_metrics(&metrics).await
    }

    /// Get performance statistics for a prompt version
    pub async fn get_stats(
        &self,
        name: &str,
        version: &str,
    ) -> Result<PerformanceStats, PromptError> {
        let metrics = self.storage.load_metrics(name, version, None).await?;
        Ok(PerformanceStats::from_metrics(&metrics))
    }

    /// Compare performance of two prompt versions
    pub async fn compare(
        &self,
        name: &str,
        version_a: &str,
        version_b: &str,
    ) -> Result<VersionComparison, PromptError> {
        let stats_a = self.get_stats(name, version_a).await?;
        let stats_b = self.get_stats(name, version_b).await?;

        Ok(VersionComparison {
            version_a: version_a.to_string(),
            version_b: version_b.to_string(),
            stats_a,
            stats_b,
        })
    }

    /// Clear the cache
    pub async fn clear_cache(&self) {
        let mut cache = self.cache.write().await;
        cache.clear();
    }
}

/// Comparison between two prompt versions
#[derive(Debug, Clone)]
pub struct VersionComparison {
    /// First version
    pub version_a: String,
    /// Second version
    pub version_b: String,
    /// Stats for version A
    pub stats_a: PerformanceStats,
    /// Stats for version B
    pub stats_b: PerformanceStats,
}

impl VersionComparison {
    /// Get the winner based on performance score
    #[must_use]
    pub fn winner(&self) -> &str {
        if self.stats_a.score() >= self.stats_b.score() {
            &self.version_a
        } else {
            &self.version_b
        }
    }

    /// Calculate improvement percentage of B over A
    #[must_use]
    pub fn improvement_percent(&self) -> f64 {
        let score_a = self.stats_a.score();
        let score_b = self.stats_b.score();

        if score_a == 0.0 {
            return 0.0;
        }

        ((score_b - score_a) / score_a) * 100.0
    }

    /// Check if difference is statistically significant (simplified)
    ///
    /// Uses a simple heuristic: at least 30 samples each and >5% difference
    #[must_use]
    pub fn is_significant(&self) -> bool {
        let min_samples = 30;
        let min_difference = 5.0;

        self.stats_a.total_executions >= min_samples
            && self.stats_b.total_executions >= min_samples
            && self.improvement_percent().abs() >= min_difference
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_prompt_creation() {
        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}}!");

        assert_eq!(prompt.name(), "test");
        assert_eq!(prompt.version(), "1.0.0");
        assert_eq!(prompt.input_variables(), &["name".to_string()]);
    }

    #[test]
    fn test_prompt_format() {
        let prompt = Prompt::new(
            "test",
            "1.0.0",
            "Hello {{name}}, you are {{age}} years old.",
        );

        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        values.insert("age".to_string(), "30".to_string());

        let result = prompt.format(&values).unwrap();
        assert_eq!(result, "Hello Alice, you are 30 years old.");
    }

    #[test]
    fn test_extract_variables() {
        let vars = extract_variables("Hello {{name}}, welcome to {{place}}!");
        assert_eq!(vars, vec!["name".to_string(), "place".to_string()]);

        let vars = extract_variables("No variables here");
        assert!(vars.is_empty());

        let vars = extract_variables("{{single}}");
        assert_eq!(vars, vec!["single".to_string()]);
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = PromptMetadata::with_description("A test prompt")
            .with_tag("test")
            .with_tag("example")
            .with_author("Alice")
            .with_custom("key", "value");

        assert_eq!(metadata.description, Some("A test prompt".to_string()));
        assert_eq!(metadata.tags, vec!["test", "example"]);
        assert_eq!(metadata.author, Some("Alice".to_string()));
        assert_eq!(metadata.custom.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_execution_metrics() {
        let metrics =
            ExecutionMetrics::new("test", "1.0.0", 150, 500, true).with_metric("quality", 0.95);

        assert_eq!(metrics.prompt_name, "test");
        assert_eq!(metrics.latency_ms, 150);
        assert!(metrics.success);
        assert_eq!(metrics.custom.get("quality"), Some(&0.95));
    }

    #[test]
    fn test_performance_stats() {
        let metrics = vec![
            ExecutionMetrics::new("test", "1.0.0", 100, 400, true),
            ExecutionMetrics::new("test", "1.0.0", 150, 500, true),
            ExecutionMetrics::new("test", "1.0.0", 200, 600, false),
        ];

        let stats = PerformanceStats::from_metrics(&metrics);

        assert_eq!(stats.total_executions, 3);
        assert_eq!(stats.successful_executions, 2);
        assert_eq!(stats.failed_executions, 1);
        assert!((stats.success_rate - 0.6667).abs() < 0.01);
        assert!((stats.avg_latency_ms - 150.0).abs() < 0.01);
    }

    #[test]
    fn test_performance_score() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 95,
            success_rate: 0.95,
            avg_latency_ms: 100.0,
            avg_token_count: 500.0,
            ..Default::default()
        };

        let score = stats.score();
        assert!(score > 0.0);
        assert!(score <= 100.0);
    }

    #[tokio::test]
    async fn test_in_memory_storage() {
        let storage = InMemoryStorage::new();

        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}}!");
        storage.save(&prompt).await.unwrap();

        let loaded = storage.load("test", "1.0.0").await.unwrap();
        assert_eq!(loaded.name(), "test");
        assert_eq!(loaded.version(), "1.0.0");

        let versions = storage.list_versions("test").await.unwrap();
        assert_eq!(versions, vec!["1.0.0"]);
    }

    #[tokio::test]
    async fn test_registry_register_and_get() {
        let registry = PromptRegistry::in_memory();

        let prompt = Prompt::new("test", "1.0.0", "Hello!");
        registry.register(prompt).await.unwrap();

        let loaded = registry.get("test", "1.0.0").await.unwrap();
        assert_eq!(loaded.name(), "test");
    }

    #[tokio::test]
    async fn test_registry_get_latest() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "1.5.0", "v1.5"))
            .await
            .unwrap();

        let latest = registry.get_latest("test").await.unwrap();
        assert_eq!(latest.version(), "2.0.0");
    }

    #[tokio::test]
    async fn test_registry_record_and_stats() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();

        for i in 0..10 {
            registry
                .record_execution("test", "1.0.0", 100 + i * 10, 500, i % 3 != 0)
                .await
                .unwrap();
        }

        let stats = registry.get_stats("test", "1.0.0").await.unwrap();
        assert_eq!(stats.total_executions, 10);
    }

    #[tokio::test]
    async fn test_registry_compare() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();

        // Record metrics for v1 (worse performance)
        for _ in 0..30 {
            registry
                .record_execution("test", "1.0.0", 200, 1000, true)
                .await
                .unwrap();
        }

        // Record metrics for v2 (better performance)
        for _ in 0..30 {
            registry
                .record_execution("test", "2.0.0", 100, 500, true)
                .await
                .unwrap();
        }

        let comparison = registry.compare("test", "1.0.0", "2.0.0").await.unwrap();
        assert_eq!(comparison.winner(), "2.0.0");
        assert!(comparison.is_significant());
    }

    // ========================================================================
    // PromptError tests
    // ========================================================================

    #[test]
    fn test_error_not_found_display() {
        let err = PromptError::NotFound("my_prompt".to_string());
        assert!(err.to_string().contains("my_prompt"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_error_version_not_found_display() {
        let err = PromptError::VersionNotFound("my_prompt".to_string(), "1.0.0".to_string());
        assert!(err.to_string().contains("my_prompt"));
        assert!(err.to_string().contains("1.0.0"));
    }

    #[test]
    fn test_error_invalid_version_display() {
        let err = PromptError::InvalidVersion("bad-version".to_string());
        assert!(err.to_string().contains("bad-version"));
    }

    #[test]
    fn test_error_storage_display() {
        let err = PromptError::StorageError("connection failed".to_string());
        assert!(err.to_string().contains("connection failed"));
    }

    #[test]
    fn test_error_serialization_display() {
        let err = PromptError::SerializationError("invalid json".to_string());
        assert!(err.to_string().contains("invalid json"));
    }

    #[test]
    fn test_error_already_exists_display() {
        let err = PromptError::AlreadyExists("my_prompt".to_string(), "1.0.0".to_string());
        assert!(err.to_string().contains("my_prompt"));
        assert!(err.to_string().contains("1.0.0"));
    }

    #[test]
    fn test_error_no_versions_display() {
        let err = PromptError::NoVersions("my_prompt".to_string());
        assert!(err.to_string().contains("my_prompt"));
    }

    // ========================================================================
    // PromptMetadata tests
    // ========================================================================

    #[test]
    fn test_metadata_default() {
        let metadata = PromptMetadata::default();
        assert!(metadata.description.is_none());
        assert!(metadata.tags.is_empty());
        assert!(metadata.author.is_none());
        assert!(metadata.custom.is_empty());
        assert!(metadata.created_at <= Utc::now());
        assert!(metadata.updated_at <= Utc::now());
    }

    #[test]
    fn test_metadata_with_tags() {
        let metadata = PromptMetadata::default().with_tags(vec!["a", "b", "c"]);
        assert_eq!(metadata.tags, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_metadata_with_tags_empty_iterator() {
        let metadata = PromptMetadata::default().with_tags(std::iter::empty::<String>());
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_metadata_chaining() {
        let metadata = PromptMetadata::with_description("desc")
            .with_tags(vec!["t1", "t2"])
            .with_tag("t3")
            .with_author("Bob")
            .with_custom("k1", "v1")
            .with_custom("k2", "v2");

        assert_eq!(metadata.description, Some("desc".to_string()));
        assert_eq!(metadata.tags, vec!["t1", "t2", "t3"]);
        assert_eq!(metadata.author, Some("Bob".to_string()));
        assert_eq!(metadata.custom.len(), 2);
    }

    // ========================================================================
    // ExecutionMetrics tests
    // ========================================================================

    #[test]
    fn test_execution_metrics_with_error() {
        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true)
            .with_error("something went wrong");

        assert!(!metrics.success); // with_error sets success to false
        assert_eq!(metrics.error, Some("something went wrong".to_string()));
    }

    #[test]
    fn test_execution_metrics_multiple_custom() {
        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true)
            .with_metric("accuracy", 0.95)
            .with_metric("f1_score", 0.92)
            .with_metric("latency_p99", 150.0);

        assert_eq!(metrics.custom.len(), 3);
        assert_eq!(metrics.custom.get("accuracy"), Some(&0.95));
        assert_eq!(metrics.custom.get("f1_score"), Some(&0.92));
        assert_eq!(metrics.custom.get("latency_p99"), Some(&150.0));
    }

    #[test]
    fn test_execution_metrics_has_unique_id() {
        let m1 = ExecutionMetrics::new("test", "1.0.0", 100, 500, true);
        let m2 = ExecutionMetrics::new("test", "1.0.0", 100, 500, true);
        assert_ne!(m1.execution_id, m2.execution_id);
    }

    #[test]
    fn test_execution_metrics_timestamp() {
        let before = Utc::now();
        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true);
        let after = Utc::now();
        assert!(metrics.timestamp >= before);
        assert!(metrics.timestamp <= after);
    }

    // ========================================================================
    // PerformanceStats tests
    // ========================================================================

    #[test]
    fn test_performance_stats_empty() {
        let stats = PerformanceStats::from_metrics(&[]);
        assert_eq!(stats.total_executions, 0);
        assert_eq!(stats.successful_executions, 0);
        assert_eq!(stats.failed_executions, 0);
        assert_eq!(stats.success_rate, 0.0);
        assert!(stats.first_execution.is_none());
        assert!(stats.last_execution.is_none());
    }

    #[test]
    fn test_performance_stats_single_metric() {
        let metrics = vec![ExecutionMetrics::new("test", "1.0.0", 100, 500, true)];
        let stats = PerformanceStats::from_metrics(&metrics);

        assert_eq!(stats.total_executions, 1);
        assert_eq!(stats.successful_executions, 1);
        assert_eq!(stats.failed_executions, 0);
        assert!((stats.success_rate - 1.0).abs() < 0.001);
        assert_eq!(stats.avg_latency_ms, 100.0);
        assert_eq!(stats.total_token_count, 500);
    }

    #[test]
    fn test_performance_stats_all_failures() {
        let metrics = vec![
            ExecutionMetrics::new("test", "1.0.0", 100, 500, false),
            ExecutionMetrics::new("test", "1.0.0", 150, 600, false),
        ];
        let stats = PerformanceStats::from_metrics(&metrics);

        assert_eq!(stats.total_executions, 2);
        assert_eq!(stats.successful_executions, 0);
        assert_eq!(stats.failed_executions, 2);
        assert_eq!(stats.success_rate, 0.0);
    }

    #[test]
    fn test_performance_stats_percentiles() {
        // Create metrics with increasing latencies
        let metrics: Vec<ExecutionMetrics> = (1..=100)
            .map(|i| ExecutionMetrics::new("test", "1.0.0", i as u64, 500, true))
            .collect();

        let stats = PerformanceStats::from_metrics(&metrics);

        // p50 should be around 50
        assert!((stats.p50_latency_ms - 50.0).abs() < 5.0);
        // p95 should be around 95
        assert!((stats.p95_latency_ms - 95.0).abs() < 5.0);
        // p99 should be around 99
        assert!((stats.p99_latency_ms - 99.0).abs() < 5.0);
    }

    #[test]
    fn test_performance_stats_score_zero_executions() {
        let stats = PerformanceStats::default();
        assert_eq!(stats.score(), 0.0);
    }

    #[test]
    fn test_performance_stats_score_perfect() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 100,
            success_rate: 1.0,
            avg_latency_ms: 50.0, // Very low latency
            avg_token_count: 100.0, // Very low token count
            ..Default::default()
        };
        let score = stats.score();
        // Perfect score should be high (close to 100)
        assert!(score > 80.0);
    }

    #[test]
    fn test_performance_stats_score_zero_latency() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 100,
            success_rate: 1.0,
            avg_latency_ms: 0.0,
            avg_token_count: 500.0,
            ..Default::default()
        };
        let score = stats.score();
        assert!(score > 0.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn test_performance_stats_score_zero_tokens() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 100,
            success_rate: 1.0,
            avg_latency_ms: 100.0,
            avg_token_count: 0.0,
            ..Default::default()
        };
        let score = stats.score();
        assert!(score > 0.0);
        assert!(score <= 100.0);
    }

    #[test]
    fn test_performance_stats_score_high_latency() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 100,
            success_rate: 1.0,
            avg_latency_ms: 10000.0, // Very high latency
            avg_token_count: 500.0,
            ..Default::default()
        };
        let score = stats.score();
        // High latency should reduce score
        assert!(score < 90.0);
    }

    // ========================================================================
    // percentile function tests
    // ========================================================================

    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 50), 0);
    }

    #[test]
    fn test_percentile_single() {
        assert_eq!(percentile(&[100], 50), 100);
        assert_eq!(percentile(&[100], 99), 100);
    }

    #[test]
    fn test_percentile_sorted_array() {
        let sorted = vec![10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&sorted, 0), 10);
        assert_eq!(percentile(&sorted, 100), 100);
    }

    // ========================================================================
    // extract_variables tests
    // ========================================================================

    #[test]
    fn test_extract_variables_with_whitespace() {
        let vars = extract_variables("{{ name }} and {{  place  }}");
        assert_eq!(vars, vec!["name".to_string(), "place".to_string()]);
    }

    #[test]
    fn test_extract_variables_duplicate() {
        let vars = extract_variables("{{name}} meets {{name}} at {{place}}");
        // Should not have duplicates
        assert_eq!(vars, vec!["name".to_string(), "place".to_string()]);
    }

    #[test]
    fn test_extract_variables_empty_placeholder() {
        let vars = extract_variables("This has {{}} empty placeholder");
        // Empty placeholders should be ignored
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_unclosed() {
        let vars = extract_variables("This has {{unclosed placeholder");
        assert!(vars.is_empty());
    }

    #[test]
    fn test_extract_variables_nested_braces() {
        // Single braces should not be treated as variables
        let vars = extract_variables("JSON: {\"key\": \"value\"} and {{var}}");
        assert_eq!(vars, vec!["var".to_string()]);
    }

    #[test]
    fn test_extract_variables_adjacent() {
        let vars = extract_variables("{{a}}{{b}}{{c}}");
        assert_eq!(
            vars,
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn test_extract_variables_at_string_boundaries() {
        let vars = extract_variables("{{start}}middle{{end}}");
        assert_eq!(vars, vec!["start".to_string(), "end".to_string()]);
    }

    // ========================================================================
    // Prompt tests
    // ========================================================================

    #[test]
    fn test_prompt_with_metadata() {
        let metadata = PromptMetadata::with_description("Test description");
        let prompt = Prompt::new("test", "1.0.0", "Hello!").with_metadata(metadata);

        assert_eq!(
            prompt.metadata().description,
            Some("Test description".to_string())
        );
    }

    #[test]
    fn test_prompt_with_active() {
        let prompt = Prompt::new("test", "1.0.0", "Hello!").with_active(false);
        assert!(!prompt.is_active());

        let prompt = Prompt::new("test", "1.0.0", "Hello!").with_active(true);
        assert!(prompt.is_active());
    }

    #[test]
    fn test_prompt_is_active_default() {
        let prompt = Prompt::new("test", "1.0.0", "Hello!");
        assert!(prompt.is_active()); // Default is true
    }

    #[test]
    fn test_prompt_parsed_version_valid() {
        let prompt = Prompt::new("test", "1.2.3", "Hello!");
        let version = prompt.parsed_version().unwrap();
        assert_eq!(version.major, 1);
        assert_eq!(version.minor, 2);
        assert_eq!(version.patch, 3);
    }

    #[test]
    fn test_prompt_parsed_version_invalid() {
        let prompt = Prompt::new("test", "not-a-version", "Hello!");
        assert!(prompt.parsed_version().is_err());
    }

    #[test]
    fn test_prompt_parsed_version_prerelease() {
        let prompt = Prompt::new("test", "1.0.0-alpha.1", "Hello!");
        let version = prompt.parsed_version().unwrap();
        assert!(!version.pre.is_empty());
    }

    #[test]
    fn test_prompt_format_missing_variable() {
        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}} from {{city}}!");
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        // "city" is not provided

        let result = prompt.format(&values).unwrap();
        // Should still contain unresolved placeholder
        assert!(result.contains("Alice"));
        assert!(result.contains("{{city}}"));
    }

    #[test]
    fn test_prompt_format_extra_values() {
        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}}!");
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        values.insert("unused".to_string(), "ignored".to_string());

        let result = prompt.format(&values).unwrap();
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_prompt_template_getter() {
        let prompt = Prompt::new("test", "1.0.0", "My template");
        assert_eq!(prompt.template(), "My template");
    }

    #[test]
    fn test_prompt_input_variables_empty() {
        let prompt = Prompt::new("test", "1.0.0", "No variables here");
        assert!(prompt.input_variables().is_empty());
    }

    // ========================================================================
    // InMemoryStorage tests
    // ========================================================================

    #[tokio::test]
    async fn test_in_memory_storage_delete() {
        let storage = InMemoryStorage::new();

        let prompt = Prompt::new("test", "1.0.0", "Hello!");
        storage.save(&prompt).await.unwrap();

        // Verify it exists
        assert!(storage.load("test", "1.0.0").await.is_ok());

        // Delete it
        storage.delete("test", "1.0.0").await.unwrap();

        // Verify it's gone
        assert!(storage.load("test", "1.0.0").await.is_err());
    }

    #[tokio::test]
    async fn test_in_memory_storage_delete_nonexistent() {
        let storage = InMemoryStorage::new();
        // Should not error when deleting nonexistent
        storage.delete("nonexistent", "1.0.0").await.unwrap();
    }

    #[tokio::test]
    async fn test_in_memory_storage_list_prompts() {
        let storage = InMemoryStorage::new();

        storage
            .save(&Prompt::new("prompt_a", "1.0.0", "A"))
            .await
            .unwrap();
        storage
            .save(&Prompt::new("prompt_b", "1.0.0", "B"))
            .await
            .unwrap();
        storage
            .save(&Prompt::new("prompt_c", "1.0.0", "C"))
            .await
            .unwrap();

        let prompts = storage.list_prompts().await.unwrap();
        assert_eq!(prompts.len(), 3);
        assert!(prompts.contains(&"prompt_a".to_string()));
        assert!(prompts.contains(&"prompt_b".to_string()));
        assert!(prompts.contains(&"prompt_c".to_string()));
    }

    #[tokio::test]
    async fn test_in_memory_storage_list_prompts_empty() {
        let storage = InMemoryStorage::new();
        let prompts = storage.list_prompts().await.unwrap();
        assert!(prompts.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_storage_list_versions_empty() {
        let storage = InMemoryStorage::new();
        let versions = storage.list_versions("nonexistent").await.unwrap();
        assert!(versions.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_storage_multiple_versions() {
        let storage = InMemoryStorage::new();

        storage
            .save(&Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        storage
            .save(&Prompt::new("test", "1.1.0", "v1.1"))
            .await
            .unwrap();
        storage
            .save(&Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();

        let versions = storage.list_versions("test").await.unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_storage_metrics() {
        let storage = InMemoryStorage::new();

        let m1 = ExecutionMetrics::new("test", "1.0.0", 100, 500, true);
        let m2 = ExecutionMetrics::new("test", "1.0.0", 150, 600, true);
        let m3 = ExecutionMetrics::new("test", "1.0.0", 200, 700, false);

        storage.save_metrics(&m1).await.unwrap();
        storage.save_metrics(&m2).await.unwrap();
        storage.save_metrics(&m3).await.unwrap();

        let loaded = storage.load_metrics("test", "1.0.0", None).await.unwrap();
        assert_eq!(loaded.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_storage_metrics_with_limit() {
        let storage = InMemoryStorage::new();

        for i in 0..10 {
            let metrics = ExecutionMetrics::new("test", "1.0.0", i * 100, 500, true);
            storage.save_metrics(&metrics).await.unwrap();
        }

        let loaded = storage.load_metrics("test", "1.0.0", Some(3)).await.unwrap();
        assert_eq!(loaded.len(), 3);
    }

    #[tokio::test]
    async fn test_in_memory_storage_metrics_empty() {
        let storage = InMemoryStorage::new();
        let loaded = storage.load_metrics("nonexistent", "1.0.0", None).await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_in_memory_storage_metrics_different_versions() {
        let storage = InMemoryStorage::new();

        storage
            .save_metrics(&ExecutionMetrics::new("test", "1.0.0", 100, 500, true))
            .await
            .unwrap();
        storage
            .save_metrics(&ExecutionMetrics::new("test", "2.0.0", 200, 600, true))
            .await
            .unwrap();

        let v1_metrics = storage.load_metrics("test", "1.0.0", None).await.unwrap();
        let v2_metrics = storage.load_metrics("test", "2.0.0", None).await.unwrap();

        assert_eq!(v1_metrics.len(), 1);
        assert_eq!(v2_metrics.len(), 1);
        assert_eq!(v1_metrics[0].latency_ms, 100);
        assert_eq!(v2_metrics[0].latency_ms, 200);
    }

    // ========================================================================
    // PromptRegistry tests
    // ========================================================================

    #[tokio::test]
    async fn test_registry_delete() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();
        assert!(registry.get("test", "1.0.0").await.is_ok());

        registry.delete("test", "1.0.0").await.unwrap();
        assert!(registry.get("test", "1.0.0").await.is_err());
    }

    #[tokio::test]
    async fn test_registry_list_prompts() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("prompt_a", "1.0.0", "A"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("prompt_b", "1.0.0", "B"))
            .await
            .unwrap();

        let prompts = registry.list_prompts().await.unwrap();
        assert_eq!(prompts.len(), 2);
    }

    #[tokio::test]
    async fn test_registry_list_versions() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "1.1.0", "v1.1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();

        let versions = registry.list_versions("test").await.unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_registry_register_invalid_version() {
        let registry = PromptRegistry::in_memory();

        let prompt = Prompt::new("test", "invalid", "Hello!");
        let result = registry.register(prompt).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_get_nonexistent() {
        let registry = PromptRegistry::in_memory();
        let result = registry.get("nonexistent", "1.0.0").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_get_latest_no_versions() {
        let registry = PromptRegistry::in_memory();
        let result = registry.get_latest("nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_get_best_no_metrics() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();

        // With no metrics recorded, get_best should return NoVersions error
        let result = registry.get_best("test").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_registry_get_best_with_metrics() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "v1"))
            .await
            .unwrap();
        registry
            .register(Prompt::new("test", "2.0.0", "v2"))
            .await
            .unwrap();

        // Record poor metrics for v1
        for _ in 0..5 {
            registry
                .record_execution("test", "1.0.0", 500, 1000, false)
                .await
                .unwrap();
        }

        // Record good metrics for v2
        for _ in 0..5 {
            registry
                .record_execution("test", "2.0.0", 50, 100, true)
                .await
                .unwrap();
        }

        let best = registry.get_best("test").await.unwrap();
        assert_eq!(best.version(), "2.0.0");
    }

    #[tokio::test]
    async fn test_registry_clear_cache() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();

        // Populate cache by getting the prompt
        let _ = registry.get("test", "1.0.0").await.unwrap();

        registry.clear_cache().await;

        // Should still work (reloads from storage)
        let loaded = registry.get("test", "1.0.0").await.unwrap();
        assert_eq!(loaded.name(), "test");
    }

    #[tokio::test]
    async fn test_registry_record_metrics() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();

        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true)
            .with_metric("custom", 42.0);

        registry.record_metrics(metrics).await.unwrap();

        let stats = registry.get_stats("test", "1.0.0").await.unwrap();
        assert_eq!(stats.total_executions, 1);
    }

    #[tokio::test]
    async fn test_registry_get_stats_empty() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();

        let stats = registry.get_stats("test", "1.0.0").await.unwrap();
        assert_eq!(stats.total_executions, 0);
    }

    #[tokio::test]
    async fn test_registry_caching() {
        let registry = PromptRegistry::in_memory();

        registry
            .register(Prompt::new("test", "1.0.0", "Hello!"))
            .await
            .unwrap();

        // First get populates cache
        let p1 = registry.get("test", "1.0.0").await.unwrap();

        // Second get should hit cache
        let p2 = registry.get("test", "1.0.0").await.unwrap();

        assert_eq!(p1.name(), p2.name());
        assert_eq!(p1.version(), p2.version());
    }

    // ========================================================================
    // VersionComparison tests
    // ========================================================================

    #[test]
    fn test_version_comparison_winner_a() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats {
                total_executions: 100,
                success_rate: 1.0,
                avg_latency_ms: 50.0,
                avg_token_count: 100.0,
                ..Default::default()
            },
            stats_b: PerformanceStats {
                total_executions: 100,
                success_rate: 0.5,
                avg_latency_ms: 500.0,
                avg_token_count: 1000.0,
                ..Default::default()
            },
        };

        assert_eq!(comparison.winner(), "1.0.0");
    }

    #[test]
    fn test_version_comparison_winner_equal() {
        let stats = PerformanceStats {
            total_executions: 100,
            success_rate: 1.0,
            avg_latency_ms: 100.0,
            avg_token_count: 500.0,
            ..Default::default()
        };

        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: stats.clone(),
            stats_b: stats,
        };

        // When equal, version_a wins (>= comparison)
        assert_eq!(comparison.winner(), "1.0.0");
    }

    #[test]
    fn test_version_comparison_improvement_percent() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats {
                total_executions: 100,
                success_rate: 0.5,
                avg_latency_ms: 200.0,
                avg_token_count: 1000.0,
                ..Default::default()
            },
            stats_b: PerformanceStats {
                total_executions: 100,
                success_rate: 1.0,
                avg_latency_ms: 100.0,
                avg_token_count: 500.0,
                ..Default::default()
            },
        };

        let improvement = comparison.improvement_percent();
        // B should be significantly better than A
        assert!(improvement > 0.0);
    }

    #[test]
    fn test_version_comparison_improvement_percent_zero_score_a() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats::default(), // score = 0
            stats_b: PerformanceStats {
                total_executions: 100,
                success_rate: 1.0,
                avg_latency_ms: 100.0,
                avg_token_count: 500.0,
                ..Default::default()
            },
        };

        let improvement = comparison.improvement_percent();
        // Should return 0.0 to avoid division by zero
        assert_eq!(improvement, 0.0);
    }

    #[test]
    fn test_version_comparison_is_significant_insufficient_samples() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats {
                total_executions: 10, // Less than 30
                success_rate: 0.5,
                ..Default::default()
            },
            stats_b: PerformanceStats {
                total_executions: 10, // Less than 30
                success_rate: 1.0,
                ..Default::default()
            },
        };

        assert!(!comparison.is_significant());
    }

    #[test]
    fn test_version_comparison_is_significant_small_difference() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats {
                total_executions: 100,
                success_rate: 0.95,
                avg_latency_ms: 100.0,
                avg_token_count: 500.0,
                ..Default::default()
            },
            stats_b: PerformanceStats {
                total_executions: 100,
                success_rate: 0.96, // Only 1% better
                avg_latency_ms: 99.0,
                avg_token_count: 495.0,
                ..Default::default()
            },
        };

        // Difference is less than 5%
        assert!(!comparison.is_significant());
    }

    // ========================================================================
    // Clone/Debug trait tests
    // ========================================================================

    #[test]
    fn test_prompt_clone() {
        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}}!");
        let cloned = prompt.clone();
        assert_eq!(prompt.name(), cloned.name());
        assert_eq!(prompt.version(), cloned.version());
        assert_eq!(prompt.template(), cloned.template());
    }

    #[test]
    fn test_prompt_metadata_clone() {
        let metadata = PromptMetadata::with_description("Test")
            .with_tag("a")
            .with_author("Bob");
        let cloned = metadata.clone();
        assert_eq!(metadata.description, cloned.description);
        assert_eq!(metadata.tags, cloned.tags);
        assert_eq!(metadata.author, cloned.author);
    }

    #[test]
    fn test_execution_metrics_clone() {
        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true);
        let cloned = metrics.clone();
        assert_eq!(metrics.execution_id, cloned.execution_id);
        assert_eq!(metrics.latency_ms, cloned.latency_ms);
    }

    #[test]
    fn test_performance_stats_clone() {
        let stats = PerformanceStats {
            total_executions: 100,
            success_rate: 0.95,
            ..Default::default()
        };
        let cloned = stats.clone();
        assert_eq!(stats.total_executions, cloned.total_executions);
        assert_eq!(stats.success_rate, cloned.success_rate);
    }

    #[test]
    fn test_version_comparison_clone() {
        let comparison = VersionComparison {
            version_a: "1.0.0".to_string(),
            version_b: "2.0.0".to_string(),
            stats_a: PerformanceStats::default(),
            stats_b: PerformanceStats::default(),
        };
        let cloned = comparison.clone();
        assert_eq!(comparison.version_a, cloned.version_a);
        assert_eq!(comparison.version_b, cloned.version_b);
    }

    #[test]
    fn test_in_memory_storage_debug() {
        let storage = InMemoryStorage::new();
        let debug_str = format!("{:?}", storage);
        assert!(debug_str.contains("InMemoryStorage"));
    }

    // ========================================================================
    // Serde serialization/deserialization tests
    // ========================================================================

    #[test]
    fn test_prompt_metadata_serde() {
        let metadata = PromptMetadata::with_description("Test")
            .with_tag("tag1")
            .with_author("Alice")
            .with_custom("key", "value");

        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: PromptMetadata = serde_json::from_str(&json).unwrap();

        assert_eq!(metadata.description, deserialized.description);
        assert_eq!(metadata.tags, deserialized.tags);
        assert_eq!(metadata.author, deserialized.author);
    }

    #[test]
    fn test_execution_metrics_serde() {
        let metrics = ExecutionMetrics::new("test", "1.0.0", 100, 500, true)
            .with_metric("accuracy", 0.95);

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: ExecutionMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(metrics.prompt_name, deserialized.prompt_name);
        assert_eq!(metrics.latency_ms, deserialized.latency_ms);
        assert_eq!(metrics.custom.get("accuracy"), deserialized.custom.get("accuracy"));
    }

    #[test]
    fn test_performance_stats_serde() {
        let stats = PerformanceStats {
            total_executions: 100,
            successful_executions: 95,
            success_rate: 0.95,
            avg_latency_ms: 150.0,
            ..Default::default()
        };

        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: PerformanceStats = serde_json::from_str(&json).unwrap();

        assert_eq!(stats.total_executions, deserialized.total_executions);
        assert_eq!(stats.success_rate, deserialized.success_rate);
    }

    #[test]
    fn test_prompt_serde() {
        let prompt = Prompt::new("test", "1.0.0", "Hello {{name}}!")
            .with_metadata(PromptMetadata::with_description("A greeting prompt"))
            .with_active(true);

        let json = serde_json::to_string(&prompt).unwrap();
        let deserialized: Prompt = serde_json::from_str(&json).unwrap();

        assert_eq!(prompt.name(), deserialized.name());
        assert_eq!(prompt.version(), deserialized.version());
        assert_eq!(prompt.template(), deserialized.template());
        assert_eq!(prompt.is_active(), deserialized.is_active());
        assert_eq!(prompt.input_variables(), deserialized.input_variables());
    }
}
