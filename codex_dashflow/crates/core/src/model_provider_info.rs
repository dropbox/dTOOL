//! Registry of model providers supported by the Codex CLI.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Codex works out-of-the-box.
//!   2. User-defined entries inside `~/.codex/config.toml` under the `model_providers`
//!      key. These override or extend the defaults at runtime.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env::VarError;
use std::time::Duration;

use crate::error::Error;

// Default configuration constants
const DEFAULT_STREAM_IDLE_TIMEOUT_MS: u64 = 300_000; // 5 minutes
const DEFAULT_STREAM_MAX_RETRIES: u64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: u64 = 4;

/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: u64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: u64 = 100;

/// Default ports for local LLM servers
pub const DEFAULT_LMSTUDIO_PORT: u16 = 1234;
pub const DEFAULT_OLLAMA_PORT: u16 = 11434;

/// Provider IDs for local LLM servers
pub const LMSTUDIO_OSS_PROVIDER_ID: &str = "lmstudio";
pub const OLLAMA_OSS_PROVIDER_ID: &str = "ollama";
pub const OPENAI_PROVIDER_ID: &str = "openai";
pub const ANTHROPIC_PROVIDER_ID: &str = "anthropic";

/// Wire protocol that the provider speaks.
///
/// Most third-party services only implement the classic OpenAI Chat Completions JSON schema,
/// whereas OpenAI itself (and a handful of others) additionally expose the more modern
/// *Responses* API. The two protocols use different request/response shapes and *cannot*
/// be auto-detected at runtime, therefore each provider entry must declare which one it expects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    Responses,

    /// Regular Chat Completions compatible with `/v1/chat/completions`.
    #[default]
    Chat,
}

impl std::fmt::Display for WireApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WireApi::Responses => write!(f, "responses"),
            WireApi::Chat => write!(f, "chat"),
        }
    }
}

/// Retry configuration for HTTP requests.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_attempts: u64,
    /// Base delay between retries in milliseconds
    pub base_delay_ms: u64,
    /// Whether to retry on 429 (rate limit) errors
    #[serde(default = "default_false")]
    pub retry_429: bool,
    /// Whether to retry on 5xx server errors
    #[serde(default = "default_true")]
    pub retry_5xx: bool,
    /// Whether to retry on transport/network errors
    #[serde(default = "default_true")]
    pub retry_transport: bool,
}

fn default_false() -> bool {
    false
}
fn default_true() -> bool {
    true
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: DEFAULT_REQUEST_MAX_RETRIES,
            base_delay_ms: 200,
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        }
    }
}

/// Serializable representation of a provider definition.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelProviderInfo {
    /// Friendly display name.
    pub name: String,

    /// Base URL for the provider's OpenAI-compatible API.
    pub base_url: Option<String>,

    /// Environment variable that stores the user's API key for this provider.
    pub env_key: Option<String>,

    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub env_key_instructions: Option<String>,

    /// Value to use with `Authorization: Bearer <token>` header. Use of this
    /// config is discouraged in favor of `env_key` for security reasons, but
    /// this may be necessary when using this programmatically.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub experimental_bearer_token: Option<String>,

    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,

    /// Optional query parameters to append to the base URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_params: Option<HashMap<String, String>>,

    /// Additional HTTP headers to include in requests to this provider where
    /// the (key, value) pairs are the header name and value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_headers: Option<HashMap<String, String>>,

    /// Optional HTTP headers to include in requests to this provider where the
    /// (key, value) pairs are the header name and _environment variable_ whose
    /// value should be used. If the environment variable is not set, or the
    /// value is empty, the header will not be included in the request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_http_headers: Option<HashMap<String, String>>,

    /// Maximum number of times to retry a failed HTTP request to this provider.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_max_retries: Option<u64>,

    /// Number of times to retry reconnecting a dropped streaming response before failing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_max_retries: Option<u64>,

    /// Idle timeout (in milliseconds) to wait for activity on a streaming response
    /// before treating the connection as lost.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_idle_timeout_ms: Option<u64>,

    /// Does this provider require special authentication flow? If true,
    /// user is presented with login screen on first run. If false (default),
    /// API key comes from the "env_key" environment variable.
    #[serde(default)]
    pub requires_special_auth: bool,
}

impl ModelProviderInfo {
    /// Create a new provider with minimal configuration.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            base_url: None,
            env_key: None,
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_special_auth: false,
        }
    }

    /// Set the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = Some(url.into());
        self
    }

    /// Set the environment variable for the API key.
    pub fn with_env_key(mut self, env_key: impl Into<String>) -> Self {
        self.env_key = Some(env_key.into());
        self
    }

    /// Set the wire API protocol.
    pub fn with_wire_api(mut self, wire_api: WireApi) -> Self {
        self.wire_api = wire_api;
        self
    }

    /// Build HTTP headers from static and environment-based configuration.
    pub fn build_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();

        // Add static headers
        if let Some(extra) = &self.http_headers {
            for (k, v) in extra {
                headers.insert(k.clone(), v.clone());
            }
        }

        // Add headers from environment variables
        if let Some(env_headers) = &self.env_http_headers {
            for (header, env_var) in env_headers {
                if let Ok(val) = std::env::var(env_var) {
                    if !val.trim().is_empty() {
                        headers.insert(header.clone(), val);
                    }
                }
            }
        }

        headers
    }

    /// If `env_key` is Some, returns the API key for this provider if present
    /// (and non-empty) in the environment. If `env_key` is required but
    /// cannot be found, returns an error.
    pub fn api_key(&self) -> crate::Result<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let env_value = std::env::var(env_key);
                env_value
                    .and_then(|v| {
                        if v.trim().is_empty() {
                            Err(VarError::NotPresent)
                        } else {
                            Ok(Some(v))
                        }
                    })
                    .map_err(|_| {
                        let msg = if let Some(instructions) = &self.env_key_instructions {
                            format!("Environment variable {} not set. {}", env_key, instructions)
                        } else {
                            format!("Environment variable {} not set", env_key)
                        };
                        Error::LlmApi(msg)
                    })
            }
            None => Ok(None),
        }
    }

    /// Get the bearer token, either from explicit config or environment variable.
    pub fn bearer_token(&self) -> crate::Result<Option<String>> {
        // First check for explicit bearer token
        if let Some(token) = &self.experimental_bearer_token {
            return Ok(Some(token.clone()));
        }

        // Fall back to environment variable
        self.api_key()
    }

    /// Effective maximum number of request retries for this provider.
    pub fn request_max_retries(&self) -> u64 {
        self.request_max_retries
            .unwrap_or(DEFAULT_REQUEST_MAX_RETRIES)
            .min(MAX_REQUEST_MAX_RETRIES)
    }

    /// Effective maximum number of stream reconnection attempts for this provider.
    pub fn stream_max_retries(&self) -> u64 {
        self.stream_max_retries
            .unwrap_or(DEFAULT_STREAM_MAX_RETRIES)
            .min(MAX_STREAM_MAX_RETRIES)
    }

    /// Effective idle timeout for streaming responses.
    pub fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(DEFAULT_STREAM_IDLE_TIMEOUT_MS))
    }

    /// Get the retry configuration for this provider.
    pub fn retry_config(&self) -> RetryConfig {
        RetryConfig {
            max_attempts: self.request_max_retries(),
            base_delay_ms: 200,
            retry_429: false,
            retry_5xx: true,
            retry_transport: true,
        }
    }

    /// Get the effective base URL for API requests.
    ///
    /// Returns the configured base_url or a default based on provider type.
    pub fn effective_base_url(&self) -> String {
        self.base_url
            .clone()
            .unwrap_or_else(|| "https://api.openai.com/v1".to_string())
    }

    /// Check if this provider appears to be an Azure endpoint.
    pub fn is_azure_endpoint(&self) -> bool {
        // Check by name
        if self.name.to_lowercase().contains("azure") {
            return true;
        }

        // Check by URL patterns
        if let Some(base_url) = &self.base_url {
            let lower = base_url.to_lowercase();
            return lower.contains(".openai.azure.com")
                || lower.contains(".openai.azure.us")
                || lower.contains(".cognitiveservices.azure")
                || lower.contains(".aoai.azure.com")
                || lower.contains(".azure-api.net")
                || lower.contains(".azurefd.net");
        }

        false
    }
}

/// Built-in default provider list.
pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    let mut providers = HashMap::new();

    // OpenAI provider
    providers.insert(
        OPENAI_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "OpenAI".into(),
            // Allow users to override the default OpenAI endpoint by
            // exporting `OPENAI_BASE_URL`. This is useful when pointing
            // at a proxy, mock server, or Azure-style deployment.
            base_url: std::env::var("OPENAI_BASE_URL")
                .ok()
                .filter(|v| !v.trim().is_empty()),
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get an API key from https://platform.openai.com/api-keys".to_string(),
            ),
            experimental_bearer_token: None,
            wire_api: WireApi::Chat, // Use Chat API by default for broader compatibility
            query_params: None,
            http_headers: Some(
                [(
                    "X-Client-Version".to_string(),
                    env!("CARGO_PKG_VERSION").to_string(),
                )]
                .into_iter()
                .collect(),
            ),
            env_http_headers: Some(
                [
                    (
                        "OpenAI-Organization".to_string(),
                        "OPENAI_ORGANIZATION".to_string(),
                    ),
                    ("OpenAI-Project".to_string(), "OPENAI_PROJECT".to_string()),
                ]
                .into_iter()
                .collect(),
            ),
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_special_auth: false,
        },
    );

    // Anthropic provider
    providers.insert(
        ANTHROPIC_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "Anthropic".into(),
            base_url: std::env::var("ANTHROPIC_BASE_URL")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| Some("https://api.anthropic.com/v1".to_string())),
            env_key: Some("ANTHROPIC_API_KEY".to_string()),
            env_key_instructions: Some(
                "Get an API key from https://console.anthropic.com/settings/keys".to_string(),
            ),
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: Some(
                [
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    (
                        "X-Client-Version".to_string(),
                        env!("CARGO_PKG_VERSION").to_string(),
                    ),
                ]
                .into_iter()
                .collect(),
            ),
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_special_auth: false,
        },
    );

    // Ollama local provider
    providers.insert(
        OLLAMA_OSS_PROVIDER_ID.to_string(),
        create_oss_provider(DEFAULT_OLLAMA_PORT, WireApi::Chat, "Ollama"),
    );

    // LMStudio local provider
    providers.insert(
        LMSTUDIO_OSS_PROVIDER_ID.to_string(),
        create_oss_provider(DEFAULT_LMSTUDIO_PORT, WireApi::Chat, "LMStudio"),
    );

    providers
}

/// Create a local/OSS provider configuration.
pub fn create_oss_provider(default_port: u16, wire_api: WireApi, name: &str) -> ModelProviderInfo {
    // Allow environment variable overrides for local providers
    let base_url = match std::env::var("CODEX_OSS_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(url) => url,
        None => {
            let port = std::env::var("CODEX_OSS_PORT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(default_port);
            format!("http://localhost:{}/v1", port)
        }
    };

    create_oss_provider_with_base_url(&base_url, wire_api, name)
}

/// Create a local/OSS provider with a specific base URL.
pub fn create_oss_provider_with_base_url(
    base_url: &str,
    wire_api: WireApi,
    name: &str,
) -> ModelProviderInfo {
    ModelProviderInfo {
        name: name.to_string(),
        base_url: Some(base_url.into()),
        env_key: None, // Local providers don't need API keys
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_special_auth: false,
    }
}

/// Provider registry that combines built-in providers with user-defined ones.
#[derive(Debug, Clone, Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, ModelProviderInfo>,
}

impl ProviderRegistry {
    /// Create a new registry with only built-in providers.
    pub fn new() -> Self {
        Self {
            providers: built_in_model_providers(),
        }
    }

    /// Create an empty registry without built-in providers.
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Add or override a provider.
    pub fn add(&mut self, id: impl Into<String>, provider: ModelProviderInfo) {
        self.providers.insert(id.into(), provider);
    }

    /// Get a provider by ID.
    pub fn get(&self, id: &str) -> Option<&ModelProviderInfo> {
        self.providers.get(id)
    }

    /// Check if a provider exists.
    pub fn contains(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }

    /// Get all provider IDs.
    pub fn provider_ids(&self) -> impl Iterator<Item = &String> {
        self.providers.keys()
    }

    /// Get all providers.
    pub fn providers(&self) -> impl Iterator<Item = (&String, &ModelProviderInfo)> {
        self.providers.iter()
    }

    /// Merge user-defined providers into the registry.
    /// User-defined providers override built-ins with the same ID.
    pub fn merge(&mut self, user_providers: HashMap<String, ModelProviderInfo>) {
        for (id, provider) in user_providers {
            self.providers.insert(id, provider);
        }
    }

    /// Get the provider for a given model name.
    ///
    /// Tries to match the model name prefix to a known provider:
    /// - "gpt-*", "o1-*" -> openai
    /// - "claude-*" -> anthropic
    /// - Otherwise returns the default provider (openai)
    pub fn provider_for_model(&self, model: &str) -> Option<&ModelProviderInfo> {
        let lower = model.to_lowercase();

        let provider_id = if lower.starts_with("gpt-")
            || lower.starts_with("o1-")
            || lower.starts_with("o3-")
            || lower.starts_with("chatgpt-")
        {
            OPENAI_PROVIDER_ID
        } else if lower.starts_with("claude-") {
            ANTHROPIC_PROVIDER_ID
        } else if lower.starts_with("llama")
            || lower.starts_with("mistral")
            || lower.starts_with("codellama")
        {
            // Local models commonly used with Ollama
            OLLAMA_OSS_PROVIDER_ID
        } else {
            // Default to OpenAI
            OPENAI_PROVIDER_ID
        };

        self.providers.get(provider_id)
    }
}

impl Default for ModelProviderInfo {
    fn default() -> Self {
        Self::new("Default")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wire_api_default() {
        let api: WireApi = Default::default();
        assert_eq!(api, WireApi::Chat);
    }

    #[test]
    fn test_wire_api_serialization() {
        assert_eq!(serde_json::to_string(&WireApi::Chat).unwrap(), "\"chat\"");
        assert_eq!(
            serde_json::to_string(&WireApi::Responses).unwrap(),
            "\"responses\""
        );

        assert_eq!(
            serde_json::from_str::<WireApi>("\"chat\"").unwrap(),
            WireApi::Chat
        );
        assert_eq!(
            serde_json::from_str::<WireApi>("\"responses\"").unwrap(),
            WireApi::Responses
        );
    }

    #[test]
    fn test_model_provider_info_new() {
        let provider = ModelProviderInfo::new("Test")
            .with_base_url("https://api.example.com")
            .with_env_key("TEST_API_KEY")
            .with_wire_api(WireApi::Responses);

        assert_eq!(provider.name, "Test");
        assert_eq!(
            provider.base_url,
            Some("https://api.example.com".to_string())
        );
        assert_eq!(provider.env_key, Some("TEST_API_KEY".to_string()));
        assert_eq!(provider.wire_api, WireApi::Responses);
    }

    #[test]
    fn test_built_in_providers() {
        let providers = built_in_model_providers();

        assert!(providers.contains_key("openai"));
        assert!(providers.contains_key("anthropic"));
        assert!(providers.contains_key("ollama"));
        assert!(providers.contains_key("lmstudio"));

        let openai = providers.get("openai").unwrap();
        assert_eq!(openai.name, "OpenAI");
        assert_eq!(openai.env_key, Some("OPENAI_API_KEY".to_string()));

        let anthropic = providers.get("anthropic").unwrap();
        assert_eq!(anthropic.name, "Anthropic");
        assert_eq!(anthropic.env_key, Some("ANTHROPIC_API_KEY".to_string()));
    }

    #[test]
    fn test_retry_defaults() {
        let provider = ModelProviderInfo::new("Test");
        assert_eq!(provider.request_max_retries(), DEFAULT_REQUEST_MAX_RETRIES);
        assert_eq!(provider.stream_max_retries(), DEFAULT_STREAM_MAX_RETRIES);
        assert_eq!(
            provider.stream_idle_timeout(),
            Duration::from_millis(DEFAULT_STREAM_IDLE_TIMEOUT_MS)
        );
    }

    #[test]
    fn test_retry_caps() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            request_max_retries: Some(200), // Over the cap
            stream_max_retries: Some(200),  // Over the cap
            ..Default::default()
        };

        assert_eq!(provider.request_max_retries(), MAX_REQUEST_MAX_RETRIES);
        assert_eq!(provider.stream_max_retries(), MAX_STREAM_MAX_RETRIES);
    }

    #[test]
    fn test_azure_detection() {
        let azure_urls = [
            "https://foo.openai.azure.com/openai",
            "https://foo.openai.azure.us/openai/deployments/bar",
            "https://foo.cognitiveservices.azure.cn/openai",
            "https://foo.aoai.azure.com/openai",
            "https://foo.openai.azure-api.net/openai",
            "https://foo.z01.azurefd.net/",
        ];

        for url in azure_urls {
            let provider = ModelProviderInfo::new("test").with_base_url(url);
            assert!(
                provider.is_azure_endpoint(),
                "expected {} to be detected as Azure",
                url
            );
        }

        // Named Azure
        let named = ModelProviderInfo::new("Azure");
        assert!(named.is_azure_endpoint());

        // Non-Azure URLs
        let non_azure_urls = [
            "https://api.openai.com/v1",
            "https://example.com/openai",
            "https://myproxy.azurewebsites.net/openai", // azurewebsites is not Azure OpenAI
        ];

        for url in non_azure_urls {
            let provider = ModelProviderInfo::new("test").with_base_url(url);
            assert!(
                !provider.is_azure_endpoint(),
                "expected {} not to be detected as Azure",
                url
            );
        }
    }

    #[test]
    fn test_provider_registry() {
        let registry = ProviderRegistry::new();

        assert!(registry.contains("openai"));
        assert!(registry.contains("anthropic"));
        assert!(registry.contains("ollama"));
        assert!(registry.contains("lmstudio"));

        let openai = registry.get("openai");
        assert!(openai.is_some());
        assert_eq!(openai.unwrap().name, "OpenAI");
    }

    #[test]
    fn test_provider_registry_merge() {
        let mut registry = ProviderRegistry::new();

        // Add custom provider
        let custom = ModelProviderInfo::new("Custom Provider")
            .with_base_url("https://custom.api.com/v1")
            .with_env_key("CUSTOM_API_KEY");

        registry.add("custom", custom);
        assert!(registry.contains("custom"));

        // Override built-in
        let override_openai =
            ModelProviderInfo::new("Custom OpenAI").with_base_url("https://proxy.example.com/v1");

        registry.add("openai", override_openai);
        assert_eq!(registry.get("openai").unwrap().name, "Custom OpenAI");
    }

    #[test]
    fn test_provider_for_model() {
        let registry = ProviderRegistry::new();

        // OpenAI models
        assert_eq!(registry.provider_for_model("gpt-4").unwrap().name, "OpenAI");
        assert_eq!(
            registry.provider_for_model("gpt-4o-mini").unwrap().name,
            "OpenAI"
        );
        assert_eq!(
            registry.provider_for_model("o1-preview").unwrap().name,
            "OpenAI"
        );

        // Anthropic models
        assert_eq!(
            registry
                .provider_for_model("claude-3-5-sonnet-latest")
                .unwrap()
                .name,
            "Anthropic"
        );
        assert_eq!(
            registry
                .provider_for_model("claude-3-opus-20240229")
                .unwrap()
                .name,
            "Anthropic"
        );

        // Local models -> Ollama
        assert_eq!(
            registry.provider_for_model("llama3.2").unwrap().name,
            "Ollama"
        );
        assert_eq!(
            registry.provider_for_model("mistral").unwrap().name,
            "Ollama"
        );

        // Unknown -> OpenAI (default)
        assert_eq!(
            registry.provider_for_model("unknown-model").unwrap().name,
            "OpenAI"
        );
    }

    #[test]
    fn test_build_headers() {
        let mut provider = ModelProviderInfo::new("Test");
        provider.http_headers = Some(
            [
                ("X-Custom-Header".to_string(), "custom-value".to_string()),
                ("Authorization".to_string(), "Bearer test".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        let headers = provider.build_headers();
        assert_eq!(
            headers.get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
        assert_eq!(
            headers.get("Authorization"),
            Some(&"Bearer test".to_string())
        );
    }

    #[test]
    fn test_deserialize_ollama_provider_toml() {
        let toml_str = r#"
name = "Ollama"
base_url = "http://localhost:11434/v1"
        "#;
        let provider: ModelProviderInfo = toml::from_str(toml_str).unwrap();
        assert_eq!(provider.name, "Ollama");
        assert_eq!(
            provider.base_url,
            Some("http://localhost:11434/v1".to_string())
        );
        assert_eq!(provider.wire_api, WireApi::Chat);
        assert!(!provider.requires_special_auth);
    }

    #[test]
    fn test_deserialize_azure_provider_toml() {
        let toml_str = r#"
name = "Azure"
base_url = "https://myaccount.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"

[query_params]
api-version = "2024-10-01-preview"
        "#;
        let provider: ModelProviderInfo = toml::from_str(toml_str).unwrap();
        assert_eq!(provider.name, "Azure");
        assert!(provider.is_azure_endpoint());
        assert_eq!(provider.env_key, Some("AZURE_OPENAI_API_KEY".to_string()));
        assert!(provider.query_params.is_some());
        assert_eq!(
            provider.query_params.as_ref().unwrap().get("api-version"),
            Some(&"2024-10-01-preview".to_string())
        );
    }

    #[test]
    fn test_deserialize_provider_with_headers_toml() {
        let toml_str = r#"
name = "Custom"
base_url = "https://api.example.com/v1"
env_key = "CUSTOM_API_KEY"

[http_headers]
X-Custom-Header = "custom-value"

[env_http_headers]
X-Auth-Header = "AUTH_TOKEN_ENV"
        "#;
        let provider: ModelProviderInfo = toml::from_str(toml_str).unwrap();
        assert_eq!(provider.name, "Custom");
        assert!(provider.http_headers.is_some());
        assert_eq!(
            provider
                .http_headers
                .as_ref()
                .unwrap()
                .get("X-Custom-Header"),
            Some(&"custom-value".to_string())
        );
        assert!(provider.env_http_headers.is_some());
        assert_eq!(
            provider
                .env_http_headers
                .as_ref()
                .unwrap()
                .get("X-Auth-Header"),
            Some(&"AUTH_TOKEN_ENV".to_string())
        );
    }

    #[test]
    fn test_oss_provider_creation() {
        let provider = create_oss_provider(11434, WireApi::Chat, "Ollama");
        assert_eq!(provider.name, "Ollama");
        assert!(provider.base_url.as_ref().unwrap().contains("11434"));
        assert!(provider.env_key.is_none()); // No API key needed for local
    }

    #[test]
    fn test_effective_base_url() {
        let provider_with_url =
            ModelProviderInfo::new("Test").with_base_url("https://custom.com/v1");
        assert_eq!(
            provider_with_url.effective_base_url(),
            "https://custom.com/v1"
        );

        let provider_without_url = ModelProviderInfo::new("Test");
        assert_eq!(
            provider_without_url.effective_base_url(),
            "https://api.openai.com/v1"
        );
    }

    #[test]
    fn test_retry_config() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            request_max_retries: Some(10),
            ..Default::default()
        };

        let config = provider.retry_config();
        assert_eq!(config.max_attempts, 10);
        assert_eq!(config.base_delay_ms, 200);
        assert!(config.retry_5xx);
        assert!(config.retry_transport);
        assert!(!config.retry_429);
    }

    #[test]
    fn test_wire_api_display() {
        assert_eq!(format!("{}", WireApi::Chat), "chat");
        assert_eq!(format!("{}", WireApi::Responses), "responses");
    }

    #[test]
    fn test_wire_api_clone_copy() {
        let api = WireApi::Chat;
        let copied = api; // Copy
        let cloned = api;
        assert_eq!(api, copied);
        assert_eq!(api, cloned);
    }

    #[test]
    fn test_retry_config_default() {
        let config = RetryConfig::default();
        assert_eq!(config.max_attempts, DEFAULT_REQUEST_MAX_RETRIES);
        assert_eq!(config.base_delay_ms, 200);
        assert!(!config.retry_429);
        assert!(config.retry_5xx);
        assert!(config.retry_transport);
    }

    #[test]
    fn test_retry_config_clone() {
        let config = RetryConfig {
            max_attempts: 5,
            base_delay_ms: 100,
            retry_429: true,
            retry_5xx: false,
            retry_transport: false,
        };
        let cloned = config.clone();
        assert_eq!(config, cloned);
    }

    #[test]
    fn test_retry_config_partial_eq() {
        let config1 = RetryConfig::default();
        let config2 = RetryConfig::default();
        let config3 = RetryConfig {
            max_attempts: 10,
            ..Default::default()
        };
        assert_eq!(config1, config2);
        assert_ne!(config1, config3);
    }

    #[test]
    fn test_model_provider_info_clone() {
        let provider = ModelProviderInfo::new("Test")
            .with_base_url("https://api.test.com")
            .with_env_key("TEST_KEY");
        let cloned = provider.clone();
        assert_eq!(provider, cloned);
        assert_eq!(provider.name, cloned.name);
        assert_eq!(provider.base_url, cloned.base_url);
    }

    #[test]
    fn test_model_provider_info_default() {
        let provider = ModelProviderInfo::default();
        assert_eq!(provider.name, "Default");
        assert!(provider.base_url.is_none());
        assert!(provider.env_key.is_none());
        assert_eq!(provider.wire_api, WireApi::Chat);
        assert!(!provider.requires_special_auth);
    }

    #[test]
    fn test_model_provider_info_partial_eq() {
        let p1 = ModelProviderInfo::new("Test");
        let p2 = ModelProviderInfo::new("Test");
        let p3 = ModelProviderInfo::new("Other");
        assert_eq!(p1, p2);
        assert_ne!(p1, p3);
    }

    #[test]
    fn test_provider_registry_empty() {
        let registry = ProviderRegistry::empty();
        assert!(!registry.contains("openai"));
        assert!(!registry.contains("anthropic"));
        assert!(registry.get("openai").is_none());
    }

    #[test]
    fn test_provider_registry_default() {
        let registry = ProviderRegistry::default();
        // Default should be same as empty
        assert!(!registry.contains("openai"));
    }

    #[test]
    fn test_provider_registry_provider_ids() {
        let mut registry = ProviderRegistry::empty();
        registry.add("provider1", ModelProviderInfo::new("P1"));
        registry.add("provider2", ModelProviderInfo::new("P2"));

        let ids: Vec<_> = registry.provider_ids().collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.iter().any(|&id| id == "provider1"));
        assert!(ids.iter().any(|&id| id == "provider2"));
    }

    #[test]
    fn test_provider_registry_providers() {
        let mut registry = ProviderRegistry::empty();
        registry.add("p1", ModelProviderInfo::new("Provider1"));
        registry.add("p2", ModelProviderInfo::new("Provider2"));

        let providers: Vec<_> = registry.providers().collect();
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_create_oss_provider_with_base_url() {
        let provider =
            create_oss_provider_with_base_url("http://custom:8080/v1", WireApi::Chat, "Custom");
        assert_eq!(provider.name, "Custom");
        assert_eq!(provider.base_url, Some("http://custom:8080/v1".to_string()));
        assert!(provider.env_key.is_none());
        assert_eq!(provider.wire_api, WireApi::Chat);
    }

    #[test]
    fn test_provider_for_model_additional_prefixes() {
        let registry = ProviderRegistry::new();

        // o3 models -> OpenAI
        assert_eq!(
            registry.provider_for_model("o3-mini").unwrap().name,
            "OpenAI"
        );

        // chatgpt models -> OpenAI
        assert_eq!(
            registry
                .provider_for_model("chatgpt-4o-latest")
                .unwrap()
                .name,
            "OpenAI"
        );

        // codellama models -> Ollama
        assert_eq!(
            registry.provider_for_model("codellama:7b").unwrap().name,
            "Ollama"
        );
    }

    #[test]
    fn test_stream_idle_timeout_custom() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            stream_idle_timeout_ms: Some(60_000), // 1 minute
            ..Default::default()
        };
        assert_eq!(provider.stream_idle_timeout(), Duration::from_secs(60));
    }

    #[test]
    fn test_model_provider_info_debug() {
        let provider = ModelProviderInfo::new("Test");
        let debug_str = format!("{:?}", provider);
        assert!(debug_str.contains("ModelProviderInfo"));
        assert!(debug_str.contains("Test"));
    }

    #[test]
    fn test_wire_api_debug() {
        let api = WireApi::Chat;
        let debug_str = format!("{:?}", api);
        assert_eq!(debug_str, "Chat");

        let api = WireApi::Responses;
        let debug_str = format!("{:?}", api);
        assert_eq!(debug_str, "Responses");
    }

    #[test]
    fn test_retry_config_debug() {
        let config = RetryConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("RetryConfig"));
    }

    #[test]
    fn test_provider_registry_debug() {
        let registry = ProviderRegistry::new();
        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("ProviderRegistry"));
    }

    #[test]
    fn test_provider_registry_clone() {
        let registry = ProviderRegistry::new();
        let cloned = registry.clone();
        assert!(cloned.contains("openai"));
        assert!(cloned.contains("anthropic"));
    }

    #[test]
    fn test_build_headers_empty() {
        let provider = ModelProviderInfo::new("Test");
        let headers = provider.build_headers();
        assert!(headers.is_empty());
    }

    #[test]
    fn test_retry_config_serialization() {
        let config = RetryConfig {
            max_attempts: 3,
            base_delay_ms: 150,
            retry_429: true,
            retry_5xx: false,
            retry_transport: true,
        };
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("3"));
        assert!(json.contains("150"));

        let parsed: RetryConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
    }

    #[test]
    fn test_constants() {
        // Verify constants are exported and have expected values
        assert_eq!(DEFAULT_LMSTUDIO_PORT, 1234);
        assert_eq!(DEFAULT_OLLAMA_PORT, 11434);
        assert_eq!(LMSTUDIO_OSS_PROVIDER_ID, "lmstudio");
        assert_eq!(OLLAMA_OSS_PROVIDER_ID, "ollama");
        assert_eq!(OPENAI_PROVIDER_ID, "openai");
        assert_eq!(ANTHROPIC_PROVIDER_ID, "anthropic");
    }

    #[test]
    fn test_default_functions() {
        assert!(!default_false());
        assert!(default_true());
    }

    #[test]
    fn test_provider_for_model_case_insensitive() {
        let registry = ProviderRegistry::new();

        // GPT variants with different cases
        assert_eq!(registry.provider_for_model("GPT-4").unwrap().name, "OpenAI");
        assert_eq!(
            registry.provider_for_model("Gpt-4o-mini").unwrap().name,
            "OpenAI"
        );

        // Claude variants
        assert_eq!(
            registry.provider_for_model("CLAUDE-3-OPUS").unwrap().name,
            "Anthropic"
        );

        // Llama variants
        assert_eq!(
            registry.provider_for_model("LLAMA3.2").unwrap().name,
            "Ollama"
        );
    }

    #[test]
    fn test_model_provider_info_serialize() {
        let provider = ModelProviderInfo::new("Test")
            .with_base_url("https://api.test.com")
            .with_env_key("TEST_KEY");

        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("Test"));
        assert!(json.contains("https://api.test.com"));
        assert!(json.contains("TEST_KEY"));
    }

    #[test]
    fn test_model_provider_info_deserialize() {
        let json = r#"{
            "name": "Test",
            "base_url": "https://api.test.com",
            "env_key": "TEST_KEY",
            "wire_api": "responses"
        }"#;

        let provider: ModelProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.name, "Test");
        assert_eq!(provider.base_url, Some("https://api.test.com".to_string()));
        assert_eq!(provider.env_key, Some("TEST_KEY".to_string()));
        assert_eq!(provider.wire_api, WireApi::Responses);
    }

    #[test]
    fn test_retry_config_serde_defaults() {
        // Test that serde defaults work correctly when fields are omitted
        let json = r#"{"max_attempts": 3, "base_delay_ms": 100}"#;
        let config: RetryConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_attempts, 3);
        assert_eq!(config.base_delay_ms, 100);
        assert!(!config.retry_429); // default_false
        assert!(config.retry_5xx); // default_true
        assert!(config.retry_transport); // default_true
    }

    #[test]
    fn test_azure_detection_by_name_only() {
        // Provider with Azure in name but no URL
        let provider = ModelProviderInfo::new("My Azure Provider");
        assert!(provider.is_azure_endpoint());

        let provider2 = ModelProviderInfo::new("My AZURE Provider");
        assert!(provider2.is_azure_endpoint());

        let provider3 = ModelProviderInfo::new("azure-custom");
        assert!(provider3.is_azure_endpoint());
    }

    #[test]
    fn test_azure_detection_no_url() {
        let provider = ModelProviderInfo::new("Regular Provider");
        assert!(!provider.is_azure_endpoint());
    }

    #[test]
    fn test_provider_registry_merge_multiple() {
        let mut registry = ProviderRegistry::new();

        let mut user_providers = HashMap::new();
        user_providers.insert("custom1".to_string(), ModelProviderInfo::new("Custom 1"));
        user_providers.insert("custom2".to_string(), ModelProviderInfo::new("Custom 2"));
        user_providers.insert(
            "openai".to_string(),
            ModelProviderInfo::new("Override OpenAI"),
        );

        registry.merge(user_providers);

        assert!(registry.contains("custom1"));
        assert!(registry.contains("custom2"));
        assert_eq!(registry.get("openai").unwrap().name, "Override OpenAI");
    }

    #[test]
    fn test_model_provider_info_builder_chain() {
        let provider = ModelProviderInfo::new("Test")
            .with_base_url("https://test.com")
            .with_env_key("KEY")
            .with_wire_api(WireApi::Responses);

        assert_eq!(provider.name, "Test");
        assert_eq!(provider.base_url, Some("https://test.com".to_string()));
        assert_eq!(provider.env_key, Some("KEY".to_string()));
        assert_eq!(provider.wire_api, WireApi::Responses);
    }

    #[test]
    fn test_request_max_retries_custom() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            request_max_retries: Some(10),
            ..Default::default()
        };
        assert_eq!(provider.request_max_retries(), 10);
    }

    #[test]
    fn test_stream_max_retries_custom() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            stream_max_retries: Some(15),
            ..Default::default()
        };
        assert_eq!(provider.stream_max_retries(), 15);
    }

    #[test]
    fn test_request_max_retries_zero() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            request_max_retries: Some(0),
            ..Default::default()
        };
        assert_eq!(provider.request_max_retries(), 0);
    }

    #[test]
    fn test_stream_max_retries_zero() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            stream_max_retries: Some(0),
            ..Default::default()
        };
        assert_eq!(provider.stream_max_retries(), 0);
    }

    #[test]
    fn test_stream_idle_timeout_very_short() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            stream_idle_timeout_ms: Some(100), // 100ms
            ..Default::default()
        };
        assert_eq!(provider.stream_idle_timeout(), Duration::from_millis(100));
    }

    #[test]
    fn test_stream_idle_timeout_very_long() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            stream_idle_timeout_ms: Some(3_600_000), // 1 hour
            ..Default::default()
        };
        assert_eq!(provider.stream_idle_timeout(), Duration::from_secs(3600));
    }

    #[test]
    fn test_create_oss_provider_with_responses_api() {
        let provider = create_oss_provider_with_base_url(
            "http://localhost:8080/v1",
            WireApi::Responses,
            "Custom",
        );
        assert_eq!(provider.wire_api, WireApi::Responses);
    }

    #[test]
    fn test_provider_for_model_empty_string() {
        let registry = ProviderRegistry::new();
        // Empty model name defaults to OpenAI
        let provider = registry.provider_for_model("");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "OpenAI");
    }

    #[test]
    fn test_provider_for_model_whitespace() {
        let registry = ProviderRegistry::new();
        // Whitespace model name defaults to OpenAI
        let provider = registry.provider_for_model("   ");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "OpenAI");
    }

    #[test]
    fn test_provider_for_model_numeric_only() {
        let registry = ProviderRegistry::new();
        let provider = registry.provider_for_model("1234");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().name, "OpenAI"); // default
    }

    #[test]
    fn test_wire_api_equality() {
        assert_eq!(WireApi::Chat, WireApi::Chat);
        assert_eq!(WireApi::Responses, WireApi::Responses);
        assert_ne!(WireApi::Chat, WireApi::Responses);
    }

    #[test]
    fn test_deserialize_provider_minimal() {
        let json = r#"{"name": "Minimal"}"#;
        let provider: ModelProviderInfo = serde_json::from_str(json).unwrap();
        assert_eq!(provider.name, "Minimal");
        assert!(provider.base_url.is_none());
        assert!(provider.env_key.is_none());
        assert_eq!(provider.wire_api, WireApi::Chat); // default
    }

    #[test]
    fn test_model_provider_info_with_all_optional_fields() {
        let provider = ModelProviderInfo {
            name: "Full".into(),
            base_url: Some("https://api.full.com".into()),
            env_key: Some("FULL_KEY".into()),
            env_key_instructions: Some("Get key from...".into()),
            experimental_bearer_token: Some("bearer123".into()),
            wire_api: WireApi::Responses,
            query_params: Some(
                [("key".to_string(), "value".to_string())]
                    .into_iter()
                    .collect(),
            ),
            http_headers: Some(
                [("X-Custom".to_string(), "header".to_string())]
                    .into_iter()
                    .collect(),
            ),
            env_http_headers: Some(
                [("X-Env".to_string(), "ENV_VAR".to_string())]
                    .into_iter()
                    .collect(),
            ),
            request_max_retries: Some(10),
            stream_max_retries: Some(20),
            stream_idle_timeout_ms: Some(120_000),
            requires_special_auth: true,
        };

        assert_eq!(provider.name, "Full");
        assert!(provider.requires_special_auth);
        assert_eq!(provider.request_max_retries(), 10);
        assert_eq!(provider.stream_max_retries(), 20);
        assert_eq!(provider.stream_idle_timeout(), Duration::from_secs(120));
    }

    #[test]
    fn test_retry_config_all_disabled() {
        let config = RetryConfig {
            max_attempts: 0,
            base_delay_ms: 0,
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        };
        assert_eq!(config.max_attempts, 0);
        assert!(!config.retry_429);
        assert!(!config.retry_5xx);
        assert!(!config.retry_transport);
    }

    #[test]
    fn test_retry_config_all_enabled() {
        let config = RetryConfig {
            max_attempts: 10,
            base_delay_ms: 500,
            retry_429: true,
            retry_5xx: true,
            retry_transport: true,
        };
        assert!(config.retry_429);
        assert!(config.retry_5xx);
        assert!(config.retry_transport);
    }

    #[test]
    fn test_provider_requires_special_auth_default() {
        let provider = ModelProviderInfo::new("Test");
        assert!(!provider.requires_special_auth);
    }

    #[test]
    fn test_provider_query_params() {
        let mut provider = ModelProviderInfo::new("Test");
        provider.query_params = Some(
            [
                ("api-version".to_string(), "2024-01-01".to_string()),
                ("region".to_string(), "us-west".to_string()),
            ]
            .into_iter()
            .collect(),
        );

        assert!(provider.query_params.is_some());
        let params = provider.query_params.as_ref().unwrap();
        assert_eq!(params.get("api-version"), Some(&"2024-01-01".to_string()));
        assert_eq!(params.get("region"), Some(&"us-west".to_string()));
    }

    #[test]
    fn test_provider_env_key_instructions() {
        let provider = ModelProviderInfo {
            name: "Test".into(),
            env_key: Some("TEST_KEY".into()),
            env_key_instructions: Some("Get your key at https://example.com".into()),
            ..Default::default()
        };

        assert_eq!(
            provider.env_key_instructions,
            Some("Get your key at https://example.com".to_string())
        );
    }
}
