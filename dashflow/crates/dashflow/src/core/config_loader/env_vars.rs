//! Centralized environment variable access for DashFlow
//!
//! This module provides typed accessors for all DASHFLOW_* environment variables
//! with documented defaults and consistent error handling.
//!
//! # Design Rationale (M-153)
//!
//! Environment variable access was previously scattered across 97+ files with
//! inconsistent patterns. This module centralizes:
//!
//! 1. **Constants** - All env var names defined once
//! 2. **Typed accessors** - `env_bool`, `env_u64`, `env_f64`, `env_string`
//! 3. **Documentation** - Each variable documented with default and purpose
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::core::config_loader::env_vars::{
//!     env_bool, env_u64, env_string_or_default,
//!     DASHFLOW_WAL, DASHFLOW_WAL_DIR,
//! };
//!
//! // Check if WAL is enabled (default: true)
//! let wal_enabled = env_bool(DASHFLOW_WAL, true);
//!
//! // Get WAL directory (default: ".dashflow/wal")
//! let wal_dir = env_string_or_default(DASHFLOW_WAL_DIR, ".dashflow/wal");
//! ```
//!
//! # Environment Variables Reference
//!
//! ## Core Features (Invariant 6: ON by default)
//!
//! | Variable | Default | Purpose |
//! |----------|---------|---------|
//! | `DASHFLOW_WAL` | `true` | Enable Write-Ahead Log |
//! | `DASHFLOW_TRACE` | `true` | Enable trace persistence |
//! | `DASHFLOW_TRACE_REDACT` | `true` | Enable PII redaction |
//! | `DASHFLOW_LIVE_INTROSPECTION` | `true` | Enable live execution tracking |
//! | `DASHFLOW_TELEMETRY_DISABLED` | not set | Set to disable telemetry |
//!
//! ## WAL Configuration
//!
//! | Variable | Default | Purpose |
//! |----------|---------|---------|
//! | `DASHFLOW_WAL_DIR` | `.dashflow/wal` | WAL storage directory |
//! | `DASHFLOW_WAL_MAX_SEGMENT_MB` | `10` | Max segment size in MB |
//! | `DASHFLOW_WAL_PARQUET_DIR` | `{WAL_DIR}/parquet` | Parquet output directory |
//! | `DASHFLOW_WAL_RETENTION_HOURS` | `24` | Retention window for Parquet + index |
//! | `DASHFLOW_WAL_COMPACTION_INTERVAL_SECS` | `60` | Background compaction interval |
//! | `DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS` | `30` | Min segment age before compaction |
//! | `DASHFLOW_WAL_COMPACTION_DELETE_WAL` | `true` | Delete .wal segments after compaction |
//! | `DASHFLOW_WAL_COMPACTION_BATCH_ROWS` | `10000` | Parquet write batch size |
//! | `DASHFLOW_WAL_AUTO_COMPACTION` | `false` | Enable auto compaction |
//!
//! ## Self-Improvement Configuration
//!
//! | Variable | Default | Purpose |
//! |----------|---------|---------|
//! | `DASHFLOW_SELF_IMPROVE_INTERVAL` | `300` | Analysis interval in seconds |
//! | `DASHFLOW_SELF_IMPROVE_TRACES_DIR` | `.dashflow/traces` | Traces directory |
//! | `DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL` | `http://localhost:9090` | Prometheus endpoint |
//! | `DASHFLOW_SELF_IMPROVE_METRICS_SOURCE` | `file` | Metrics source (file/prometheus) |
//!
//! ## Storage Retention
//!
//! | Variable | Default | Purpose |
//! |----------|---------|---------|
//! | `DASHFLOW_STORAGE_RETENTION` | `true` | Enable storage retention |
//! | `DASHFLOW_STORAGE_MAX_REPORTS` | `100` | Max reports to keep |
//! | `DASHFLOW_STORAGE_MAX_PLANS` | `50` | Max plans per status |
//! | `DASHFLOW_STORAGE_PLAN_AGE_DAYS` | `30` | Plan retention in days |
//! | `DASHFLOW_TRACE_RETENTION` | `true` | Enable trace retention |
//! | `DASHFLOW_TRACE_MAX_COUNT` | `1000` | Max traces to keep |
//! | `DASHFLOW_TRACE_MAX_AGE_DAYS` | `7` | Trace retention in days |
//! | `DASHFLOW_TRACE_MAX_SIZE_MB` | `500` | Max trace storage size |

use std::time::Duration;

// =============================================================================
// Environment Variable Name Constants
// =============================================================================

// Core Feature Flags
/// Enable Write-Ahead Log (default: true)
pub const DASHFLOW_WAL: &str = "DASHFLOW_WAL";
/// Enable trace persistence (default: true)
pub const DASHFLOW_TRACE: &str = "DASHFLOW_TRACE";
/// Enable PII redaction in traces (default: true)
pub const DASHFLOW_TRACE_REDACT: &str = "DASHFLOW_TRACE_REDACT";
/// Enable live introspection (default: true)
pub const DASHFLOW_LIVE_INTROSPECTION: &str = "DASHFLOW_LIVE_INTROSPECTION";
/// Set to disable telemetry (any value disables)
pub const DASHFLOW_TELEMETRY_DISABLED: &str = "DASHFLOW_TELEMETRY_DISABLED";

// WAL Configuration
/// WAL storage directory (default: .dashflow/wal)
pub const DASHFLOW_WAL_DIR: &str = "DASHFLOW_WAL_DIR";
/// SQLite index path (default: .dashflow/index.db)
pub const DASHFLOW_INDEX_PATH: &str = "DASHFLOW_INDEX_PATH";
/// Max WAL segment size in MB (default: 10)
pub const DASHFLOW_WAL_MAX_SEGMENT_MB: &str = "DASHFLOW_WAL_MAX_SEGMENT_MB";
/// Parquet output directory (default: {WAL_DIR}/parquet)
pub const DASHFLOW_WAL_PARQUET_DIR: &str = "DASHFLOW_WAL_PARQUET_DIR";
/// Retention window in hours (default: 24)
pub const DASHFLOW_WAL_RETENTION_HOURS: &str = "DASHFLOW_WAL_RETENTION_HOURS";
/// Background compaction interval in seconds (default: 60)
pub const DASHFLOW_WAL_COMPACTION_INTERVAL_SECS: &str = "DASHFLOW_WAL_COMPACTION_INTERVAL_SECS";
/// Minimum segment age before compaction in seconds (default: 30)
pub const DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS: &str =
    "DASHFLOW_WAL_COMPACTION_MIN_SEGMENT_AGE_SECS";
/// Delete .wal segments after successful compaction (default: true)
pub const DASHFLOW_WAL_COMPACTION_DELETE_WAL: &str = "DASHFLOW_WAL_COMPACTION_DELETE_WAL";
/// Parquet write batch size in rows (default: 10000)
pub const DASHFLOW_WAL_COMPACTION_BATCH_ROWS: &str = "DASHFLOW_WAL_COMPACTION_BATCH_ROWS";
/// Enable auto compaction (default: false)
pub const DASHFLOW_WAL_AUTO_COMPACTION: &str = "DASHFLOW_WAL_AUTO_COMPACTION";

// State and Metrics Redaction
/// Redact state in events (values: full, partial, none; default: none)
pub const DASHFLOW_STATE_REDACT: &str = "DASHFLOW_STATE_REDACT";
/// Redact metrics (values: full, partial, none; default: none)
pub const DASHFLOW_METRICS_REDACT: &str = "DASHFLOW_METRICS_REDACT";

// Self-Improvement Configuration
/// Self-improvement analysis interval in seconds (default: 300)
pub const DASHFLOW_SELF_IMPROVE_INTERVAL: &str = "DASHFLOW_SELF_IMPROVE_INTERVAL";
/// Traces directory for self-improvement (default: .dashflow/traces)
pub const DASHFLOW_SELF_IMPROVE_TRACES_DIR: &str = "DASHFLOW_SELF_IMPROVE_TRACES_DIR";
/// Prometheus endpoint URL (default: http://localhost:9090)
pub const DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL: &str = "DASHFLOW_SELF_IMPROVE_PROMETHEUS_URL";
/// Metrics source: file or prometheus (default: file)
pub const DASHFLOW_SELF_IMPROVE_METRICS_SOURCE: &str = "DASHFLOW_SELF_IMPROVE_METRICS_SOURCE";
/// Slow node threshold in milliseconds (default: 1000)
pub const DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS: &str = "DASHFLOW_SELF_IMPROVE_SLOW_THRESHOLD_MS";
/// Error rate threshold for analysis (default: 0.1)
pub const DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD: &str = "DASHFLOW_SELF_IMPROVE_ERROR_THRESHOLD";
/// Retry threshold count (default: 3)
pub const DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD: &str = "DASHFLOW_SELF_IMPROVE_RETRY_THRESHOLD";
/// Minimum traces for analysis (default: 10)
pub const DASHFLOW_SELF_IMPROVE_MIN_TRACES: &str = "DASHFLOW_SELF_IMPROVE_MIN_TRACES";
/// Enable cleanup of old data (default: true)
pub const DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED: &str = "DASHFLOW_SELF_IMPROVE_CLEANUP_ENABLED";
/// Cleanup interval in seconds (default: 3600)
pub const DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL: &str = "DASHFLOW_SELF_IMPROVE_CLEANUP_INTERVAL";

// Storage Retention
/// Enable storage retention (default: true)
pub const DASHFLOW_STORAGE_RETENTION: &str = "DASHFLOW_STORAGE_RETENTION";
/// Max reports to keep (default: 100)
pub const DASHFLOW_STORAGE_MAX_REPORTS: &str = "DASHFLOW_STORAGE_MAX_REPORTS";
/// Max plans per status to keep (default: 50)
pub const DASHFLOW_STORAGE_MAX_PLANS: &str = "DASHFLOW_STORAGE_MAX_PLANS";
/// Plan retention age in days (default: 30)
pub const DASHFLOW_STORAGE_PLAN_AGE_DAYS: &str = "DASHFLOW_STORAGE_PLAN_AGE_DAYS";
/// Hypothesis retention age in days (default: 90)
pub const DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS: &str = "DASHFLOW_STORAGE_HYPOTHESIS_AGE_DAYS";
/// Storage warning size in MB (default: 100)
pub const DASHFLOW_STORAGE_WARNING_SIZE_MB: &str = "DASHFLOW_STORAGE_WARNING_SIZE_MB";
/// Storage critical size in MB (default: 500)
pub const DASHFLOW_STORAGE_CRITICAL_SIZE_MB: &str = "DASHFLOW_STORAGE_CRITICAL_SIZE_MB";
/// Report warning count threshold (default: 80)
pub const DASHFLOW_STORAGE_REPORT_WARNING_COUNT: &str = "DASHFLOW_STORAGE_REPORT_WARNING_COUNT";
/// Plan warning count threshold (default: 40)
pub const DASHFLOW_STORAGE_PLAN_WARNING_COUNT: &str = "DASHFLOW_STORAGE_PLAN_WARNING_COUNT";

// Trace Retention
/// Enable trace retention (default: true)
pub const DASHFLOW_TRACE_RETENTION: &str = "DASHFLOW_TRACE_RETENTION";
/// Max traces to keep (default: 1000)
pub const DASHFLOW_TRACE_MAX_COUNT: &str = "DASHFLOW_TRACE_MAX_COUNT";
/// Trace max age in days (default: 7)
pub const DASHFLOW_TRACE_MAX_AGE_DAYS: &str = "DASHFLOW_TRACE_MAX_AGE_DAYS";
/// Trace max storage size in MB (default: 500)
pub const DASHFLOW_TRACE_MAX_SIZE_MB: &str = "DASHFLOW_TRACE_MAX_SIZE_MB";
/// Trace compress age in days (default: 1)
pub const DASHFLOW_TRACE_COMPRESS_AGE_DAYS: &str = "DASHFLOW_TRACE_COMPRESS_AGE_DAYS";

// Health Check Configuration
/// Health check storage path (default: .dashflow/introspection)
pub const DASHFLOW_HEALTH_STORAGE_PATH: &str = "DASHFLOW_HEALTH_STORAGE_PATH";
/// Health check traces path (default: .dashflow/traces)
pub const DASHFLOW_HEALTH_TRACES_PATH: &str = "DASHFLOW_HEALTH_TRACES_PATH";
/// Enable storage health check (default: true)
pub const DASHFLOW_HEALTH_CHECK_STORAGE: &str = "DASHFLOW_HEALTH_CHECK_STORAGE";
/// Enable traces health check (default: true)
pub const DASHFLOW_HEALTH_CHECK_TRACES: &str = "DASHFLOW_HEALTH_CHECK_TRACES";
/// Enable cache health check (default: true)
pub const DASHFLOW_HEALTH_CHECK_CACHE: &str = "DASHFLOW_HEALTH_CHECK_CACHE";
/// Max storage size in MB for health check (default: 500)
pub const DASHFLOW_HEALTH_MAX_STORAGE_MB: &str = "DASHFLOW_HEALTH_MAX_STORAGE_MB";
/// Max trace count for health check (default: 10000)
pub const DASHFLOW_HEALTH_MAX_TRACES: &str = "DASHFLOW_HEALTH_MAX_TRACES";

// Network/Colony Configuration
/// Worker mode (values: standalone, distributed)
pub const DASHFLOW_WORKER_MODE: &str = "DASHFLOW_WORKER_MODE";
/// Worker ID for distributed mode
pub const DASHFLOW_WORKER_ID: &str = "DASHFLOW_WORKER_ID";
/// Parent peer ID for colony hierarchy
pub const DASHFLOW_PARENT_PEER_ID: &str = "DASHFLOW_PARENT_PEER_ID";
/// Task assignment for worker
pub const DASHFLOW_TASK: &str = "DASHFLOW_TASK";
/// Enable network features (default: false)
pub const DASHFLOW_NETWORK_ENABLED: &str = "DASHFLOW_NETWORK_ENABLED";

// Streaming Configuration
/// Kafka brokers (default: localhost:9092)
pub const KAFKA_BROKERS: &str = "KAFKA_BROKERS";
/// Kafka topic for events (default: dashstream-events)
pub const KAFKA_TOPIC: &str = "KAFKA_TOPIC";
/// Kafka consumer group ID (default varies by consumer)
pub const KAFKA_GROUP_ID: &str = "KAFKA_GROUP_ID";
/// Kafka auto offset reset (values: earliest, latest; default: earliest)
pub const KAFKA_AUTO_OFFSET_RESET: &str = "KAFKA_AUTO_OFFSET_RESET";
/// Flush timeout in seconds for streaming (default: 5)
pub const DASHFLOW_FLUSH_TIMEOUT_SECS: &str = "DASHFLOW_FLUSH_TIMEOUT_SECS";
/// Max payload size for DashStream decode in bytes (default: 10MB)
pub const DASHSTREAM_MAX_PAYLOAD_BYTES: &str = "DASHSTREAM_MAX_PAYLOAD_BYTES";

// Instance/Observability
/// Custom instance ID for metrics (default: auto-generated UUID)
pub const DASHFLOW_INSTANCE_ID: &str = "DASHFLOW_INSTANCE_ID";

// Lint/Feedback
/// Enable lint telemetry
pub const DASHFLOW_LINT_TELEMETRY: &str = "DASHFLOW_LINT_TELEMETRY";

// External URLs
/// DashSwarm registry URL
pub const DASHSWARM_REGISTRY_URL: &str = "DASHSWARM_REGISTRY_URL";
/// Prometheus URL for metrics queries
pub const PROMETHEUS_URL: &str = "PROMETHEUS_URL";
/// Ollama base URL (default: http://localhost:11434)
pub const OLLAMA_BASE_URL: &str = "OLLAMA_BASE_URL";
/// Qdrant URL (default: http://localhost:6333)
pub const QDRANT_URL: &str = "QDRANT_URL";
/// Chroma URL (default: http://localhost:8000)
pub const CHROMA_URL: &str = "CHROMA_URL";

// Provider Configuration
/// OpenAI API base URL override (used by some integrations for compatibility)
pub const OPENAI_API_BASE: &str = "OPENAI_API_BASE";
/// Azure OpenAI endpoint (e.g., <https://my-resource.openai.azure.com>)
pub const AZURE_OPENAI_ENDPOINT: &str = "AZURE_OPENAI_ENDPOINT";
/// Azure OpenAI API version (e.g., 2024-02-15-preview)
pub const AZURE_OPENAI_API_VERSION: &str = "AZURE_OPENAI_API_VERSION";
/// Azure OpenAI deployment name (e.g., gpt-4, gpt-35-turbo)
pub const AZURE_OPENAI_DEPLOYMENT_NAME: &str = "AZURE_OPENAI_DEPLOYMENT_NAME";

// API Keys (sensitive - always accessed via SecretReference)
/// OpenAI API key
pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";
/// Azure OpenAI API key
pub const AZURE_OPENAI_API_KEY: &str = "AZURE_OPENAI_API_KEY";
/// Anthropic API key
pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";
/// Google API key
pub const GOOGLE_API_KEY: &str = "GOOGLE_API_KEY";
/// Google Cloud access token
pub const GOOGLE_CLOUD_ACCESS_TOKEN: &str = "GOOGLE_CLOUD_ACCESS_TOKEN";
/// HuggingFace token
pub const HF_TOKEN: &str = "HF_TOKEN";

// AWS Configuration
/// AWS access key ID for Bedrock and other AWS services
pub const AWS_ACCESS_KEY_ID: &str = "AWS_ACCESS_KEY_ID";
/// AWS default region
pub const AWS_DEFAULT_REGION: &str = "AWS_DEFAULT_REGION";
/// AWS region (alternative to AWS_DEFAULT_REGION)
pub const AWS_REGION: &str = "AWS_REGION";

// Provider API Keys (additional providers)
/// Cohere API key
pub const COHERE_API_KEY: &str = "COHERE_API_KEY";
/// Gemini API key
pub const GEMINI_API_KEY: &str = "GEMINI_API_KEY";
/// xAI API key
pub const XAI_API_KEY: &str = "XAI_API_KEY";
/// DeepSeek API key
pub const DEEPSEEK_API_KEY: &str = "DEEPSEEK_API_KEY";
/// xAI API base URL (default: <https://api.x.ai/v1>)
pub const XAI_API_BASE: &str = "XAI_API_BASE";
/// DeepSeek API base URL (default: <https://api.deepseek.com/v1>)
pub const DEEPSEEK_API_BASE: &str = "DEEPSEEK_API_BASE";
/// Together AI API key
pub const TOGETHER_API_KEY: &str = "TOGETHER_API_KEY";
/// Replicate API token
pub const REPLICATE_API_TOKEN: &str = "REPLICATE_API_TOKEN";
/// Perplexity API key
pub const PERPLEXITY_API_KEY: &str = "PERPLEXITY_API_KEY";
/// HuggingFace API key (alternative to HF_TOKEN)
pub const HUGGINGFACE_API_KEY: &str = "HUGGINGFACE_API_KEY";
/// Cloudflare account ID
pub const CLOUDFLARE_ACCOUNT_ID: &str = "CLOUDFLARE_ACCOUNT_ID";
/// Cloudflare API token
pub const CLOUDFLARE_API_TOKEN: &str = "CLOUDFLARE_API_TOKEN";
/// Mistral API key
pub const MISTRAL_API_KEY: &str = "MISTRAL_API_KEY";
/// Voyage API key
pub const VOYAGE_API_KEY: &str = "VOYAGE_API_KEY";
/// Jina API key
pub const JINA_API_KEY: &str = "JINA_API_KEY";
/// Perplexity API key (alternative name)
pub const PPLX_API_KEY: &str = "PPLX_API_KEY";
/// Perplexity API base URL (default: <https://api.perplexity.ai>)
pub const PPLX_API_BASE: &str = "PPLX_API_BASE";
/// HuggingFace Hub API token (alternative to HF_TOKEN)
pub const HUGGINGFACEHUB_API_TOKEN: &str = "HUGGINGFACEHUB_API_TOKEN";
/// Tavily API key for web search
pub const TAVILY_API_KEY: &str = "TAVILY_API_KEY";
/// Fireworks AI API key
pub const FIREWORKS_API_KEY: &str = "FIREWORKS_API_KEY";
/// Nomic AI API key
pub const NOMIC_API_KEY: &str = "NOMIC_API_KEY";

// Integration API Keys (tools and services)
/// ClickUp access token
pub const CLICKUP_ACCESS_TOKEN: &str = "CLICKUP_ACCESS_TOKEN";
/// GitLab personal access token
pub const GITLAB_TOKEN: &str = "GITLAB_TOKEN";
/// YouTube Data API key
pub const YOUTUBE_API_KEY: &str = "YOUTUBE_API_KEY";
/// Zapier NLA API key
pub const ZAPIER_NLA_API_KEY: &str = "ZAPIER_NLA_API_KEY";
/// Slack bot token
pub const SLACK_BOT_TOKEN: &str = "SLACK_BOT_TOKEN";
/// Slack test channel for integration tests
pub const SLACK_TEST_CHANNEL: &str = "SLACK_TEST_CHANNEL";
/// Wolfram Alpha app ID
pub const WOLFRAM_APP_ID: &str = "WOLFRAM_APP_ID";
/// OpenWeatherMap API key
pub const OPENWEATHERMAP_API_KEY: &str = "OPENWEATHERMAP_API_KEY";

// Jira Configuration
/// Jira base URL (e.g., <https://your-domain.atlassian.net>)
pub const JIRA_BASE_URL: &str = "JIRA_BASE_URL";
/// Jira user email for authentication
pub const JIRA_EMAIL: &str = "JIRA_EMAIL";
/// Jira API token
pub const JIRA_API_TOKEN: &str = "JIRA_API_TOKEN";

// Google Search Configuration
/// Google Custom Search Engine ID
pub const GOOGLE_CSE_ID: &str = "GOOGLE_CSE_ID";

// Vector Store URLs
/// Elasticsearch URL (default: http://localhost:9200)
pub const ELASTICSEARCH_URL: &str = "ELASTICSEARCH_URL";
/// Milvus server URL (default: http://localhost:19530)
pub const MILVUS_URL: &str = "MILVUS_URL";
/// Pinecone API key
pub const PINECONE_API_KEY: &str = "PINECONE_API_KEY";
/// Pinecone index host URL
pub const PINECONE_INDEX_HOST: &str = "PINECONE_INDEX_HOST";

// LangSmith/LangChain Configuration
/// LangSmith API key
pub const LANGSMITH_API_KEY: &str = "LANGSMITH_API_KEY";
/// LangChain API key (alternative to LANGSMITH_API_KEY)
pub const LANGCHAIN_API_KEY: &str = "LANGCHAIN_API_KEY";
/// LangSmith API endpoint (default: <https://api.smith.langchain.com>)
pub const LANGSMITH_ENDPOINT: &str = "LANGSMITH_ENDPOINT";
/// LangChain API endpoint (alternative to LANGSMITH_ENDPOINT)
pub const LANGCHAIN_ENDPOINT: &str = "LANGCHAIN_ENDPOINT";
/// LangSmith project name
pub const LANGSMITH_PROJECT: &str = "LANGSMITH_PROJECT";
/// LangChain project name (alternative to LANGSMITH_PROJECT)
pub const LANGCHAIN_PROJECT: &str = "LANGCHAIN_PROJECT";

// Ollama Configuration
/// Ollama host URL (default: http://localhost:11434)
pub const OLLAMA_HOST: &str = "OLLAMA_HOST";

// CI/Development Environment Detection
/// CI environment indicator
pub const CI: &str = "CI";
/// Claude Code environment indicator
pub const CLAUDE_CODE: &str = "CLAUDE_CODE";
/// Kubernetes service host (presence indicates K8s environment)
pub const KUBERNETES_SERVICE_HOST: &str = "KUBERNETES_SERVICE_HOST";
/// Home directory
pub const HOME: &str = "HOME";
/// Enable debug mode for observability/introspection
pub const DASHFLOW_DEBUG: &str = "DASHFLOW_DEBUG";

// Cache Configuration
/// API key cache TTL in seconds (default: 60)
pub const CACHE_API_KEY_TTL_SECS: &str = "CACHE_API_KEY_TTL_SECS";
/// Resolution cache TTL in seconds (default: 300)
pub const CACHE_RESOLUTION_TTL_SECS: &str = "CACHE_RESOLUTION_TTL_SECS";
/// Search results cache TTL in seconds (default: 120)
pub const CACHE_SEARCH_TTL_SECS: &str = "CACHE_SEARCH_TTL_SECS";
/// Metadata cache TTL in seconds (default: 1800)
pub const CACHE_METADATA_TTL_SECS: &str = "CACHE_METADATA_TTL_SECS";
/// Maximum cache entries (default: 10000)
pub const CACHE_MAX_ENTRIES: &str = "CACHE_MAX_ENTRIES";
/// Track cache statistics (default: true)
pub const CACHE_TRACK_STATS: &str = "CACHE_TRACK_STATS";

// Redis Configuration
/// Redis URL (default: redis://localhost:6379)
pub const REDIS_URL: &str = "REDIS_URL";
/// Redis key prefix (default: dashflow:cache:)
pub const REDIS_PREFIX: &str = "REDIS_PREFIX";
/// Redis connection timeout in seconds (default: 5)
pub const REDIS_CONNECT_TIMEOUT_SECS: &str = "REDIS_CONNECT_TIMEOUT_SECS";
/// Redis operation timeout in seconds (default: 2)
pub const REDIS_OPERATION_TIMEOUT_SECS: &str = "REDIS_OPERATION_TIMEOUT_SECS";

// Metrics Exporter Configuration
/// Prometheus metrics port (default: 9190)
pub const METRICS_PORT: &str = "METRICS_PORT";
/// Prometheus metrics bind IP (default: 0.0.0.0)
pub const METRICS_BIND_IP: &str = "METRICS_BIND_IP";
/// Prometheus session timeout in seconds for session tracking (default: 300)
pub const PROMETHEUS_SESSION_TIMEOUT_SECS: &str = "PROMETHEUS_SESSION_TIMEOUT_SECS";

// Registry Server Configuration
/// Registry server host (default: 127.0.0.1)
pub const REGISTRY_HOST: &str = "REGISTRY_HOST";
/// Registry server port (default: 8080)
pub const REGISTRY_PORT: &str = "REGISTRY_PORT";
/// Database URL for registry (optional)
pub const DATABASE_URL: &str = "DATABASE_URL";
/// Rate limit requests per minute (default: 60)
pub const RATE_LIMIT_RPM: &str = "RATE_LIMIT_RPM";
/// Enable CORS (default: true)
pub const CORS_ENABLED: &str = "CORS_ENABLED";
/// CORS allowed origins (comma-separated, default: *)
pub const CORS_ORIGINS: &str = "CORS_ORIGINS";
/// Storage URL for packages (optional, e.g., file:///path or s3://bucket)
pub const STORAGE_URL: &str = "STORAGE_URL";
/// Storage path for filesystem backend (alternative to STORAGE_URL)
pub const STORAGE_PATH: &str = "STORAGE_PATH";
/// Base URL for registry API (default: http://localhost:8080)
pub const BASE_URL: &str = "BASE_URL";
/// Enable CDN for package downloads (default: false)
pub const CDN_ENABLED: &str = "CDN_ENABLED";

// Registry Client Configuration
/// DashFlow registry URL (default: http://localhost:3001 for development)
pub const DASHFLOW_REGISTRY_URL: &str = "DASHFLOW_REGISTRY_URL";
/// Default registry URL for development (production uses env var)
pub const DEFAULT_DASHFLOW_REGISTRY_URL: &str = "http://localhost:3001";
/// DashFlow registry API key (optional)
pub const DASHFLOW_REGISTRY_API_KEY: &str = "DASHFLOW_REGISTRY_API_KEY";

// OpenTelemetry Configuration
/// OTLP exporter endpoint
pub const OTEL_EXPORTER_OTLP_ENDPOINT: &str = "OTEL_EXPORTER_OTLP_ENDPOINT";
/// OTLP service name (default: dashflow-registry)
pub const OTEL_SERVICE_NAME: &str = "OTEL_SERVICE_NAME";
/// OTLP sampling rate (default: 1.0)
pub const OTEL_SAMPLING_RATE: &str = "OTEL_SAMPLING_RATE";

// S3/Cloud Storage Configuration
/// S3 bucket name
pub const S3_BUCKET: &str = "S3_BUCKET";
/// S3 storage type (values: aws, r2, minio; default: aws)
pub const S3_STORAGE_TYPE: &str = "S3_STORAGE_TYPE";
/// S3 endpoint URL (for non-AWS S3-compatible storage)
pub const S3_ENDPOINT: &str = "S3_ENDPOINT";
/// S3 region (default: us-east-1)
pub const S3_REGION: &str = "S3_REGION";
/// S3 path style (default: false)
pub const S3_PATH_STYLE: &str = "S3_PATH_STYLE";
/// S3 key prefix (default: packages/)
pub const S3_PREFIX: &str = "S3_PREFIX";
/// Cloudflare R2 account ID
pub const R2_ACCOUNT_ID: &str = "R2_ACCOUNT_ID";

// Vector Store Configuration (for registry semantic search)
/// Qdrant collection name (default: dashflow-registry)
pub const QDRANT_COLLECTION: &str = "QDRANT_COLLECTION";
/// OpenAI embedding model (default: text-embedding-3-small)
pub const OPENAI_EMBEDDING_MODEL: &str = "OPENAI_EMBEDDING_MODEL";
/// Embedding dimension (default: 1536)
pub const EMBEDDING_DIMENSION: &str = "EMBEDDING_DIMENSION";

// OpenSearch Configuration
/// OpenSearch connection URL (default: http://localhost:9200)
pub const OPENSEARCH_URL: &str = "OPENSEARCH_URL";

// Kafka Streaming Configuration
/// Kafka admin operation timeout in seconds (create/delete topic)
pub const KAFKA_OPERATION_TIMEOUT_SECS: &str = "KAFKA_OPERATION_TIMEOUT_SECS";
/// Kafka metadata fetch timeout in seconds (list topics)
pub const KAFKA_METADATA_TIMEOUT_SECS: &str = "KAFKA_METADATA_TIMEOUT_SECS";
/// Kafka bootstrap servers (alternative to KAFKA_BROKERS)
pub const KAFKA_BOOTSTRAP_SERVERS: &str = "KAFKA_BOOTSTRAP_SERVERS";
/// DashStream topic name (alternative to KAFKA_TOPIC)
pub const DASHSTREAM_TOPIC: &str = "DASHSTREAM_TOPIC";
/// Kafka partition number for consumer (default: 0)
pub const KAFKA_PARTITION: &str = "KAFKA_PARTITION";
/// Kafka tenant ID (default: "default")
pub const KAFKA_TENANT_ID: &str = "KAFKA_TENANT_ID";
/// Kafka security protocol (default: plaintext)
pub const KAFKA_SECURITY_PROTOCOL: &str = "KAFKA_SECURITY_PROTOCOL";
/// Kafka SASL mechanism (e.g., PLAIN, SCRAM-SHA-256)
pub const KAFKA_SASL_MECHANISM: &str = "KAFKA_SASL_MECHANISM";
/// Kafka SASL username
pub const KAFKA_SASL_USERNAME: &str = "KAFKA_SASL_USERNAME";
/// Kafka SASL password
pub const KAFKA_SASL_PASSWORD: &str = "KAFKA_SASL_PASSWORD";
/// Path to SSL CA certificate file
pub const KAFKA_SSL_CA_LOCATION: &str = "KAFKA_SSL_CA_LOCATION";
/// Path to SSL client certificate file
pub const KAFKA_SSL_CERTIFICATE_LOCATION: &str = "KAFKA_SSL_CERTIFICATE_LOCATION";
/// Path to SSL client private key file
pub const KAFKA_SSL_KEY_LOCATION: &str = "KAFKA_SSL_KEY_LOCATION";
/// Password for SSL private key file
pub const KAFKA_SSL_KEY_PASSWORD: &str = "KAFKA_SSL_KEY_PASSWORD";
/// SSL endpoint identification algorithm (default: https)
pub const KAFKA_SSL_ENDPOINT_ALGORITHM: &str = "KAFKA_SSL_ENDPOINT_ALGORITHM";
/// Kafka broker address family (values: any, v4, v6)
pub const KAFKA_BROKER_ADDRESS_FAMILY: &str = "KAFKA_BROKER_ADDRESS_FAMILY";

// Health Check Port Configuration
/// Health check port for streaming services (default varies by service)
pub const HEALTH_PORT: &str = "HEALTH_PORT";

// Grafana Configuration
/// Grafana URL (default: http://localhost:3000)
pub const GRAFANA_URL: &str = "GRAFANA_URL";
/// Grafana user (default: admin)
pub const GRAFANA_USER: &str = "GRAFANA_USER";
/// Grafana password (default: admin)
pub const GRAFANA_PASS: &str = "GRAFANA_PASS";

// OpenAI API Configuration
/// OpenAI API base URL (default: <https://api.openai.com>)
pub const OPENAI_API_BASE_URL: &str = "OPENAI_API_BASE_URL";

// Default URL Constants (not env var names)
/// Default OpenAI API base URL
pub const DEFAULT_OPENAI_API_BASE_URL: &str = "https://api.openai.com";
/// Default OpenAI files endpoint
pub const DEFAULT_OPENAI_FILES_ENDPOINT: &str = "/v1/files";
/// Default OpenAI fine-tuning jobs endpoint
pub const DEFAULT_OPENAI_FINE_TUNING_JOBS_ENDPOINT: &str = "/v1/fine_tuning/jobs";
/// Default OpenAI chat completions endpoint
pub const DEFAULT_OPENAI_CHAT_COMPLETIONS_ENDPOINT: &str = "/v1/chat/completions";

// Anthropic API Configuration
/// Anthropic API base URL (default: <https://api.anthropic.com>)
pub const ANTHROPIC_API_BASE_URL: &str = "ANTHROPIC_API_BASE_URL";

// Default Anthropic API URL
/// Default Anthropic API base URL
pub const DEFAULT_ANTHROPIC_API_BASE_URL: &str = "https://api.anthropic.com";
/// Default Anthropic messages endpoint
pub const DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT: &str = "/v1/messages";

// Google AI (Gemini) API Configuration
/// Google AI API base URL (default: <https://generativelanguage.googleapis.com>)
pub const GOOGLE_AI_API_BASE_URL: &str = "GOOGLE_AI_API_BASE_URL";

// Default Google AI API URLs
/// Default Google AI API base URL (Gemini)
pub const DEFAULT_GOOGLE_AI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com";
/// Default Google AI API version path
pub const DEFAULT_GOOGLE_AI_API_VERSION: &str = "/v1beta";
/// Default Google AI models path template (use with format!)
pub const DEFAULT_GOOGLE_AI_MODELS_PATH: &str = "/models/";

// Cohere API Configuration
/// Cohere API base URL (default: <https://api.cohere.com>)
pub const COHERE_API_BASE_URL: &str = "COHERE_API_BASE_URL";

// Default Cohere API URLs
/// Default Cohere API base URL (note: api.cohere.ai redirects to api.cohere.com)
pub const DEFAULT_COHERE_API_BASE_URL: &str = "https://api.cohere.com";
/// Default Cohere API v1 path (chat, rerank)
pub const DEFAULT_COHERE_API_V1_PATH: &str = "/v1";
/// Default Cohere API v2 path (embeddings)
pub const DEFAULT_COHERE_API_V2_PATH: &str = "/v2";
/// Default Cohere chat endpoint
pub const DEFAULT_COHERE_CHAT_ENDPOINT: &str = "/chat";
/// Default Cohere rerank endpoint
pub const DEFAULT_COHERE_RERANK_ENDPOINT: &str = "/rerank";
/// Default Cohere embed endpoint
pub const DEFAULT_COHERE_EMBED_ENDPOINT: &str = "/embed";

// OpenAI-compatible provider defaults
/// Default xAI API base URL (OpenAI-compatible)
pub const DEFAULT_XAI_API_BASE: &str = "https://api.x.ai/v1";
/// Default DeepSeek API base URL (OpenAI-compatible)
pub const DEFAULT_DEEPSEEK_API_BASE: &str = "https://api.deepseek.com/v1";
/// Default Perplexity API base URL (OpenAI-compatible)
pub const DEFAULT_PPLX_API_BASE: &str = "https://api.perplexity.ai";

// WASM Executor Configuration
/// JWT secret for WASM executor authentication (REQUIRED for production)
pub const JWT_SECRET: &str = "JWT_SECRET";

// Observability WebSocket Server Configuration
/// Redis clear timeout in seconds for replay buffer
pub const REDIS_CLEAR_TIMEOUT_SECS: &str = "REDIS_CLEAR_TIMEOUT_SECS";
/// Include full payload in dead letter queue (default: false)
pub const DLQ_INCLUDE_FULL_PAYLOAD: &str = "DLQ_INCLUDE_FULL_PAYLOAD";
/// Trusted proxy IPs for WebSocket server (comma-separated)
pub const WEBSOCKET_TRUSTED_PROXY_IPS: &str = "WEBSOCKET_TRUSTED_PROXY_IPS";
/// Kafka decode error handling strategy (values: skip, fail)
pub const KAFKA_ON_DECODE_ERROR: &str = "KAFKA_ON_DECODE_ERROR";

// =============================================================================
// Typed Accessor Functions
// =============================================================================

/// Read a boolean environment variable with a default value.
///
/// Treats "false", "FALSE", "False", "0" as false.
/// Any other value (including empty string) is treated as true.
/// Missing variable returns the default.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{env_bool, DASHFLOW_WAL};
///
/// // Returns true if DASHFLOW_WAL is not set
/// let wal_enabled = env_bool(DASHFLOW_WAL, true);
///
/// // Returns false if DASHFLOW_WAL="false"
/// std::env::set_var("DASHFLOW_WAL", "false");
/// let wal_enabled = env_bool(DASHFLOW_WAL, true);
/// assert!(!wal_enabled);
/// ```
#[must_use]
pub fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| !(v.eq_ignore_ascii_case("false") || v == "0"))
        .unwrap_or(default)
}

/// Read a u64 environment variable with a default value.
///
/// Returns the default if the variable is not set or cannot be parsed.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{env_u64, DASHFLOW_WAL_RETENTION_HOURS};
///
/// let hours = env_u64(DASHFLOW_WAL_RETENTION_HOURS, 24);
/// ```
#[must_use]
pub fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(default)
}

/// Read a usize environment variable with a default value.
///
/// Returns the default if the variable is not set or cannot be parsed.
#[must_use]
pub fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(default)
}

/// Read an f64 environment variable with a default value.
///
/// Returns the default if the variable is not set or cannot be parsed.
#[must_use]
pub fn env_f64(name: &str, default: f64) -> f64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(default)
}

/// Read a string environment variable, returning None if not set.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{env_string, DASHFLOW_WORKER_ID};
///
/// if let Some(worker_id) = env_string(DASHFLOW_WORKER_ID) {
///     println!("Running as worker: {}", worker_id);
/// }
/// ```
#[must_use]
pub fn env_string(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Read a string environment variable with a default value.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{env_string_or_default, DASHFLOW_WAL_DIR};
///
/// let dir = env_string_or_default(DASHFLOW_WAL_DIR, ".dashflow/wal");
/// ```
#[must_use]
pub fn env_string_or_default(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

/// Read a Duration from an environment variable specifying seconds.
///
/// Returns the default Duration if the variable is not set or cannot be parsed.
///
/// # Example
///
/// ```rust,ignore
/// use std::time::Duration;
/// use dashflow::core::config_loader::env_vars::{
///     env_duration_secs, DASHFLOW_WAL_COMPACTION_INTERVAL_SECS
/// };
///
/// let interval = env_duration_secs(DASHFLOW_WAL_COMPACTION_INTERVAL_SECS, Duration::from_secs(60));
/// ```
#[must_use]
pub fn env_duration_secs(name: &str, default: Duration) -> Duration {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or(default)
}

/// Check if an environment variable is set (regardless of value).
///
/// This is useful for presence-based flags like `CI` or `KUBERNETES_SERVICE_HOST`.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{env_is_set, CI, KUBERNETES_SERVICE_HOST};
///
/// let is_ci = env_is_set(CI);
/// let is_k8s = env_is_set(KUBERNETES_SERVICE_HOST);
/// ```
#[must_use]
pub fn env_is_set(name: &str) -> bool {
    std::env::var(name).is_ok()
}

/// Check if an API key is available in the environment.
///
/// This is a convenience function for checking if external services can be used.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{has_api_key, OPENAI_API_KEY};
///
/// if has_api_key(OPENAI_API_KEY) {
///     // Can use OpenAI services
/// }
/// ```
#[must_use]
pub fn has_api_key(name: &str) -> bool {
    std::env::var(name).map(|v| !v.is_empty()).unwrap_or(false)
}

/// Check if any LLM API key is available.
///
/// Returns true if ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY is set.
#[must_use]
pub fn has_any_llm_api_key() -> bool {
    has_api_key(ANTHROPIC_API_KEY) || has_api_key(OPENAI_API_KEY) || has_api_key(GOOGLE_API_KEY)
}

/// Get the OpenAI API base URL, with env var override support.
///
/// Checks `OPENAI_API_BASE_URL` environment variable first, falls back to default.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::openai_api_base_url;
///
/// // Returns "https://api.openai.com" by default
/// let url = openai_api_base_url();
///
/// // Or set via env var:
/// std::env::set_var("OPENAI_API_BASE_URL", "http://localhost:8080");
/// let url = openai_api_base_url();
/// assert_eq!(url, "http://localhost:8080");
/// ```
#[must_use]
pub fn openai_api_base_url() -> String {
    env_string_or_default(OPENAI_API_BASE_URL, DEFAULT_OPENAI_API_BASE_URL)
}

/// Build an OpenAI API URL with the given endpoint.
///
/// Uses the base URL from `openai_api_base_url()` and appends the endpoint.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{openai_api_url, DEFAULT_OPENAI_FILES_ENDPOINT};
///
/// // Returns "https://api.openai.com/v1/files"
/// let url = openai_api_url(DEFAULT_OPENAI_FILES_ENDPOINT);
/// ```
#[must_use]
pub fn openai_api_url(endpoint: &str) -> String {
    format!("{}{}", openai_api_base_url(), endpoint)
}

/// Get the Anthropic API base URL, with optional override via environment variable.
///
/// Checks `ANTHROPIC_API_BASE_URL` environment variable first, falls back to default.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::anthropic_api_base_url;
///
/// // Returns "https://api.anthropic.com" by default
/// let url = anthropic_api_base_url();
///
/// // Or set via env var:
/// std::env::set_var("ANTHROPIC_API_BASE_URL", "http://localhost:8080");
/// let url = anthropic_api_base_url();
/// assert_eq!(url, "http://localhost:8080");
/// ```
#[must_use]
pub fn anthropic_api_base_url() -> String {
    env_string_or_default(ANTHROPIC_API_BASE_URL, DEFAULT_ANTHROPIC_API_BASE_URL)
}

/// Build an Anthropic API URL with the given endpoint.
///
/// Uses the base URL from `anthropic_api_base_url()` and appends the endpoint.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{anthropic_api_url, DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT};
///
/// // Returns "https://api.anthropic.com/v1/messages"
/// let url = anthropic_api_url(DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT);
/// ```
#[must_use]
pub fn anthropic_api_url(endpoint: &str) -> String {
    format!("{}{}", anthropic_api_base_url(), endpoint)
}

/// Get the Google AI (Gemini) API base URL, with optional override via environment variable.
///
/// Checks `GOOGLE_AI_API_BASE_URL` environment variable first, falls back to default.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::google_ai_api_base_url;
///
/// // Returns "https://generativelanguage.googleapis.com" by default
/// let url = google_ai_api_base_url();
/// ```
#[must_use]
pub fn google_ai_api_base_url() -> String {
    env_string_or_default(GOOGLE_AI_API_BASE_URL, DEFAULT_GOOGLE_AI_API_BASE_URL)
}

/// Build a Google AI (Gemini) generate content URL for a given model.
///
/// Constructs: `{base_url}/v1beta/models/{model}:generateContent`
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::google_ai_generate_content_url;
///
/// // Returns "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
/// let url = google_ai_generate_content_url("gemini-pro");
/// ```
#[must_use]
pub fn google_ai_generate_content_url(model: &str) -> String {
    format!(
        "{}{}{}{}:generateContent",
        google_ai_api_base_url(),
        DEFAULT_GOOGLE_AI_API_VERSION,
        DEFAULT_GOOGLE_AI_MODELS_PATH,
        model
    )
}

/// Get the Cohere API base URL, with optional override via environment variable.
///
/// Checks `COHERE_API_BASE_URL` environment variable first, falls back to default.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::cohere_api_base_url;
///
/// // Returns "https://api.cohere.com" by default
/// let url = cohere_api_base_url();
/// ```
#[must_use]
pub fn cohere_api_base_url() -> String {
    env_string_or_default(COHERE_API_BASE_URL, DEFAULT_COHERE_API_BASE_URL)
}

/// Build a Cohere API v1 URL (chat, rerank).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{cohere_api_v1_url, DEFAULT_COHERE_CHAT_ENDPOINT};
///
/// // Returns "https://api.cohere.com/v1/chat"
/// let url = cohere_api_v1_url(DEFAULT_COHERE_CHAT_ENDPOINT);
/// ```
#[must_use]
pub fn cohere_api_v1_url(endpoint: &str) -> String {
    format!(
        "{}{}{}",
        cohere_api_base_url(),
        DEFAULT_COHERE_API_V1_PATH,
        endpoint
    )
}

/// Build a Cohere API v2 URL (embeddings).
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::env_vars::{cohere_api_v2_url, DEFAULT_COHERE_EMBED_ENDPOINT};
///
/// // Returns "https://api.cohere.com/v2/embed"
/// let url = cohere_api_v2_url(DEFAULT_COHERE_EMBED_ENDPOINT);
/// ```
#[must_use]
pub fn cohere_api_v2_url(endpoint: &str) -> String {
    format!(
        "{}{}{}",
        cohere_api_base_url(),
        DEFAULT_COHERE_API_V2_PATH,
        endpoint
    )
}

// =============================================================================
// DashFlow Registry URL Helpers
// =============================================================================

/// Get the DashFlow registry URL with fallback to default.
///
/// Uses `DASHFLOW_REGISTRY_URL` env var if set, otherwise returns the
/// default development URL.
///
/// # Examples
///
/// ```
/// use dashflow::core::config_loader::env_vars::dashflow_registry_url;
///
/// // Uses DASHFLOW_REGISTRY_URL if set, otherwise "http://localhost:3001"
/// let url = dashflow_registry_url();
/// ```
#[must_use]
pub fn dashflow_registry_url() -> String {
    env_string_or_default(DASHFLOW_REGISTRY_URL, DEFAULT_DASHFLOW_REGISTRY_URL)
}

/// Get the DashFlow registry API key if set.
///
/// Returns `None` if `DASHFLOW_REGISTRY_API_KEY` is not set or empty.
///
/// # Examples
///
/// ```
/// use dashflow::core::config_loader::env_vars::dashflow_registry_api_key;
///
/// if let Some(key) = dashflow_registry_api_key() {
///     println!("API key found");
/// }
/// ```
#[must_use]
pub fn dashflow_registry_api_key() -> Option<String> {
    env_string(DASHFLOW_REGISTRY_API_KEY).filter(|s| !s.is_empty())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_bool_default_true() {
        const VAR: &str = "TEST_BOOL_DEFAULT_TRUE";
        std::env::remove_var(VAR);
        assert!(env_bool(VAR, true));
    }

    #[test]
    fn test_env_bool_default_false() {
        const VAR: &str = "TEST_BOOL_DEFAULT_FALSE";
        std::env::remove_var(VAR);
        assert!(!env_bool(VAR, false));
    }

    #[test]
    fn test_env_bool_false_values() {
        const VAR: &str = "TEST_BOOL_FALSE_VALUES";
        std::env::set_var(VAR, "false");
        assert!(!env_bool(VAR, true));

        std::env::set_var(VAR, "FALSE");
        assert!(!env_bool(VAR, true));

        std::env::set_var(VAR, "False");
        assert!(!env_bool(VAR, true));

        std::env::set_var(VAR, "0");
        assert!(!env_bool(VAR, true));

        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_bool_true_values() {
        const VAR: &str = "TEST_BOOL_TRUE_VALUES";
        std::env::set_var(VAR, "true");
        assert!(env_bool(VAR, false));

        std::env::set_var(VAR, "1");
        assert!(env_bool(VAR, false));

        std::env::set_var(VAR, "yes");
        assert!(env_bool(VAR, false));

        std::env::set_var(VAR, "");
        assert!(env_bool(VAR, false));

        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_u64_default() {
        const VAR: &str = "TEST_U64_DEFAULT";
        std::env::remove_var(VAR);
        assert_eq!(env_u64(VAR, 42), 42);
    }

    #[test]
    fn test_env_u64_valid() {
        const VAR: &str = "TEST_U64_VALID";
        std::env::set_var(VAR, "100");
        assert_eq!(env_u64(VAR, 42), 100);
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_u64_invalid() {
        const VAR: &str = "TEST_U64_INVALID";
        std::env::set_var(VAR, "not_a_number");
        assert_eq!(env_u64(VAR, 42), 42);

        std::env::set_var(VAR, "-5");
        assert_eq!(env_u64(VAR, 42), 42);

        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_usize_default() {
        const VAR: &str = "TEST_USIZE_DEFAULT";
        std::env::remove_var(VAR);
        assert_eq!(env_usize(VAR, 10), 10);
    }

    #[test]
    fn test_env_usize_valid() {
        const VAR: &str = "TEST_USIZE_VALID";
        std::env::set_var(VAR, "25");
        assert_eq!(env_usize(VAR, 10), 25);
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_f64_default() {
        const VAR: &str = "TEST_F64_DEFAULT";
        std::env::remove_var(VAR);
        assert!((env_f64(VAR, 0.5) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_env_f64_valid() {
        const VAR: &str = "TEST_F64_VALID";
        std::env::set_var(VAR, "0.75");
        assert!((env_f64(VAR, 0.5) - 0.75).abs() < f64::EPSILON);
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_f64_invalid() {
        const VAR: &str = "TEST_F64_INVALID";
        std::env::set_var(VAR, "not_a_float");
        assert!((env_f64(VAR, 0.5) - 0.5).abs() < f64::EPSILON);
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_string() {
        const VAR: &str = "TEST_ENV_STRING";
        std::env::remove_var(VAR);
        assert!(env_string(VAR).is_none());

        std::env::set_var(VAR, "hello");
        assert_eq!(env_string(VAR), Some("hello".to_string()));
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_string_or_default() {
        const VAR: &str = "TEST_STRING_OR_DEFAULT";
        std::env::remove_var(VAR);
        assert_eq!(env_string_or_default(VAR, "default"), "default");

        std::env::set_var(VAR, "custom");
        assert_eq!(env_string_or_default(VAR, "default"), "custom");
        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_duration_secs() {
        const VAR: &str = "TEST_DURATION_SECS";
        std::env::remove_var(VAR);
        assert_eq!(
            env_duration_secs(VAR, Duration::from_secs(60)),
            Duration::from_secs(60)
        );

        std::env::set_var(VAR, "120");
        assert_eq!(
            env_duration_secs(VAR, Duration::from_secs(60)),
            Duration::from_secs(120)
        );

        std::env::set_var(VAR, "invalid");
        assert_eq!(
            env_duration_secs(VAR, Duration::from_secs(60)),
            Duration::from_secs(60)
        );

        std::env::remove_var(VAR);
    }

    #[test]
    fn test_env_is_set() {
        const VAR: &str = "TEST_ENV_IS_SET";
        std::env::remove_var(VAR);
        assert!(!env_is_set(VAR));

        std::env::set_var(VAR, "anything");
        assert!(env_is_set(VAR));

        std::env::set_var(VAR, "");
        assert!(env_is_set(VAR));

        std::env::remove_var(VAR);
    }

    #[test]
    fn test_has_api_key() {
        const VAR: &str = "TEST_HAS_API_KEY";
        std::env::remove_var(VAR);
        assert!(!has_api_key(VAR));

        std::env::set_var(VAR, "");
        assert!(!has_api_key(VAR));

        std::env::set_var(VAR, "sk-test-key");
        assert!(has_api_key(VAR));

        std::env::remove_var(VAR);
    }

    #[test]
    #[allow(clippy::const_is_empty)]
    fn test_constant_names_not_empty() {
        // Verify all constants are non-empty strings
        assert!(!DASHFLOW_WAL.is_empty());
        assert!(!DASHFLOW_TRACE.is_empty());
        assert!(!DASHFLOW_WAL_DIR.is_empty());
        assert!(!OPENAI_API_KEY.is_empty());
    }

    #[test]
    fn test_constant_names_uppercase() {
        // Verify all constants follow SCREAMING_SNAKE_CASE
        assert!(DASHFLOW_WAL.chars().all(|c| c.is_uppercase() || c == '_'));
        assert!(DASHFLOW_TRACE.chars().all(|c| c.is_uppercase() || c == '_'));
        assert!(KAFKA_BROKERS.chars().all(|c| c.is_uppercase() || c == '_'));
    }

    #[test]
    fn test_anthropic_url_constants() {
        // Verify URL constants are properly formatted
        assert_eq!(DEFAULT_ANTHROPIC_API_BASE_URL, "https://api.anthropic.com");
        assert_eq!(DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT, "/v1/messages");
        assert_eq!(ANTHROPIC_API_BASE_URL, "ANTHROPIC_API_BASE_URL");
    }

    #[test]
    fn test_anthropic_api_url_construction() {
        // Test that URL construction works correctly (without env var interference)
        // Note: env_string_or_default returns default when env var is not set
        let base = env_string_or_default("TEST_ANTHROPIC_BASE", DEFAULT_ANTHROPIC_API_BASE_URL);
        let full = format!("{}{}", base, DEFAULT_ANTHROPIC_MESSAGES_ENDPOINT);
        assert_eq!(full, "https://api.anthropic.com/v1/messages");
    }

    #[test]
    fn test_google_ai_url_constants() {
        // Verify URL constants are properly formatted
        assert_eq!(
            DEFAULT_GOOGLE_AI_API_BASE_URL,
            "https://generativelanguage.googleapis.com"
        );
        assert_eq!(DEFAULT_GOOGLE_AI_API_VERSION, "/v1beta");
        assert_eq!(DEFAULT_GOOGLE_AI_MODELS_PATH, "/models/");
        assert_eq!(GOOGLE_AI_API_BASE_URL, "GOOGLE_AI_API_BASE_URL");
    }

    #[test]
    fn test_google_ai_generate_content_url_construction() {
        // Test that URL construction works correctly
        let url = google_ai_generate_content_url("gemini-pro");
        assert_eq!(
            url,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-pro:generateContent"
        );

        let url_flash = google_ai_generate_content_url("gemini-1.5-flash");
        assert_eq!(
            url_flash,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent"
        );
    }

    #[test]
    fn test_cohere_url_constants() {
        // Verify URL constants are properly formatted
        assert_eq!(DEFAULT_COHERE_API_BASE_URL, "https://api.cohere.com");
        assert_eq!(DEFAULT_COHERE_API_V1_PATH, "/v1");
        assert_eq!(DEFAULT_COHERE_API_V2_PATH, "/v2");
        assert_eq!(DEFAULT_COHERE_CHAT_ENDPOINT, "/chat");
        assert_eq!(DEFAULT_COHERE_RERANK_ENDPOINT, "/rerank");
        assert_eq!(DEFAULT_COHERE_EMBED_ENDPOINT, "/embed");
        assert_eq!(COHERE_API_BASE_URL, "COHERE_API_BASE_URL");
    }

    #[test]
    fn test_cohere_api_v1_url_construction() {
        // Test v1 URL construction (chat, rerank)
        let chat_url = cohere_api_v1_url(DEFAULT_COHERE_CHAT_ENDPOINT);
        assert_eq!(chat_url, "https://api.cohere.com/v1/chat");

        let rerank_url = cohere_api_v1_url(DEFAULT_COHERE_RERANK_ENDPOINT);
        assert_eq!(rerank_url, "https://api.cohere.com/v1/rerank");
    }

    #[test]
    fn test_cohere_api_v2_url_construction() {
        // Test v2 URL construction (embeddings)
        let embed_url = cohere_api_v2_url(DEFAULT_COHERE_EMBED_ENDPOINT);
        assert_eq!(embed_url, "https://api.cohere.com/v2/embed");
    }

    #[test]
    fn test_openai_compatible_provider_base_defaults() {
        assert_eq!(DEFAULT_XAI_API_BASE, "https://api.x.ai/v1");
        assert_eq!(DEFAULT_DEEPSEEK_API_BASE, "https://api.deepseek.com/v1");
        assert_eq!(DEFAULT_PPLX_API_BASE, "https://api.perplexity.ai");

        assert_eq!(XAI_API_BASE, "XAI_API_BASE");
        assert_eq!(DEEPSEEK_API_BASE, "DEEPSEEK_API_BASE");
        assert_eq!(PPLX_API_BASE, "PPLX_API_BASE");
    }

    #[test]
    fn test_dashflow_registry_url_default() {
        // Remove env var to test default
        std::env::remove_var(DASHFLOW_REGISTRY_URL);
        let url = dashflow_registry_url();
        assert_eq!(url, DEFAULT_DASHFLOW_REGISTRY_URL);
        assert_eq!(url, "http://localhost:3001");
    }

    #[test]
    fn test_dashflow_registry_url_from_env() {
        std::env::set_var(DASHFLOW_REGISTRY_URL, "https://registry.example.com");
        let url = dashflow_registry_url();
        assert_eq!(url, "https://registry.example.com");
        std::env::remove_var(DASHFLOW_REGISTRY_URL);
    }

    /// Combined test to avoid env var race conditions in parallel test execution
    #[test]
    fn test_dashflow_registry_api_key_all_cases() {
        // Use a static mutex to serialize env var access across parallel tests
        use std::sync::Mutex;
        static API_KEY_TEST_MUTEX: Mutex<()> = Mutex::new(());
        let _guard = API_KEY_TEST_MUTEX.lock().unwrap();

        // Test 1: None when unset
        std::env::remove_var(DASHFLOW_REGISTRY_API_KEY);
        assert!(dashflow_registry_api_key().is_none(), "Should be None when unset");

        // Test 2: None when empty
        std::env::set_var(DASHFLOW_REGISTRY_API_KEY, "");
        assert!(dashflow_registry_api_key().is_none(), "Should be None when empty");

        // Test 3: Returns value when set
        std::env::set_var(DASHFLOW_REGISTRY_API_KEY, "test-api-key-123");
        let key = dashflow_registry_api_key();
        assert_eq!(key, Some("test-api-key-123".to_string()), "Should return set value");

        // Cleanup
        std::env::remove_var(DASHFLOW_REGISTRY_API_KEY);
    }
}
