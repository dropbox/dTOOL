//! Google Cloud Translation transformer for documents.
//!
//! This transformer uses the Google Cloud Translation API to translate
//! document content between languages.

use crate::core::config_loader::env_vars::{env_string, GOOGLE_CLOUD_ACCESS_TOKEN};
use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::{Error, Result};
use crate::core::http_client::create_basic_client;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Configuration for Google Cloud Translation API.
#[derive(Debug, Clone)]
pub struct GoogleTranslateConfig {
    /// Google Cloud Project ID
    pub project_id: String,
    /// Translation model location (default: "global")
    pub location: String,
    /// Optional model ID to use
    pub model_id: Option<String>,
    /// Optional glossary ID to use
    pub glossary_id: Option<String>,
    /// Optional regional API endpoint
    pub api_endpoint: Option<String>,
}

impl Default for GoogleTranslateConfig {
    fn default() -> Self {
        Self {
            project_id: String::new(),
            location: "global".to_string(),
            model_id: None,
            glossary_id: None,
            api_endpoint: None,
        }
    }
}

impl GoogleTranslateConfig {
    /// Create a new Google Translate configuration with the given project ID.
    #[must_use]
    pub fn new(project_id: impl Into<String>) -> Self {
        Self {
            project_id: project_id.into(),
            ..Default::default()
        }
    }

    /// Set the Google Cloud Project ID.
    #[must_use]
    pub fn with_project_id(mut self, project_id: impl Into<String>) -> Self {
        self.project_id = project_id.into();
        self
    }

    /// Set the translation model location.
    #[must_use]
    pub fn with_location(mut self, location: impl Into<String>) -> Self {
        self.location = location.into();
        self
    }

    /// Set the model ID to use.
    #[must_use]
    pub fn with_model_id(mut self, model_id: impl Into<String>) -> Self {
        self.model_id = Some(model_id.into());
        self
    }

    /// Set the glossary ID to use.
    #[must_use]
    pub fn with_glossary_id(mut self, glossary_id: impl Into<String>) -> Self {
        self.glossary_id = Some(glossary_id.into());
        self
    }

    /// Set a custom API endpoint.
    #[must_use]
    pub fn with_api_endpoint(mut self, endpoint: impl Into<String>) -> Self {
        self.api_endpoint = Some(endpoint.into());
        self
    }

    /// Validate the configuration.
    ///
    /// Returns an error if the project ID is empty.
    pub fn validate(&self) -> Result<()> {
        if self.project_id.is_empty() {
            return Err(Error::InvalidInput("project_id is required".to_string()));
        }
        Ok(())
    }
}

/// Translation request parameters.
#[derive(Debug, Clone)]
pub struct TranslationParams {
    /// ISO 639 language code of the input document
    pub source_language_code: Option<String>,
    /// ISO 639 language code of the output document (required)
    pub target_language_code: String,
    /// Media type of input text ("text/plain" or "text/html")
    pub mime_type: String,
}

impl Default for TranslationParams {
    fn default() -> Self {
        Self {
            source_language_code: None,
            target_language_code: String::new(),
            mime_type: "text/plain".to_string(),
        }
    }
}

impl TranslationParams {
    /// Create new translation parameters with the target language code.
    #[must_use]
    pub fn new(target_language_code: impl Into<String>) -> Self {
        Self {
            target_language_code: target_language_code.into(),
            ..Default::default()
        }
    }

    /// Set the source language code (ISO 639).
    #[must_use]
    pub fn with_source_language(mut self, code: impl Into<String>) -> Self {
        self.source_language_code = Some(code.into());
        self
    }

    /// Set the target language code (ISO 639).
    #[must_use]
    pub fn with_target_language(mut self, code: impl Into<String>) -> Self {
        self.target_language_code = code.into();
        self
    }

    /// Set the mime type ("text/plain" or "text/html").
    #[must_use]
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = mime_type.into();
        self
    }

    /// Validate the parameters.
    ///
    /// Returns an error if the target language code is empty.
    pub fn validate(&self) -> Result<()> {
        if self.target_language_code.is_empty() {
            return Err(Error::InvalidInput(
                "target_language_code is required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Request structure for Google Translate API.
#[derive(Debug, Serialize)]
struct TranslateTextRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_language_code: Option<String>,
    target_language_code: String,
    contents: Vec<String>,
    mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    glossary_config: Option<GlossaryConfig>,
}

#[derive(Debug, Serialize)]
struct GlossaryConfig {
    glossary: String,
}

/// Response structure from Google Translate API.
#[derive(Debug, Deserialize)]
struct TranslateTextResponse {
    translations: Vec<Translation>,
    #[serde(default)]
    glossary_translations: Vec<Translation>,
}

#[derive(Debug, Deserialize)]
struct Translation {
    translated_text: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    detected_language_code: String,
}

/// Document transformer that translates text using Google Cloud Translation API.
///
/// This transformer sends document content to Google Cloud Translation API
/// and returns documents with translated text. It preserves original metadata
/// while adding translation-specific metadata.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{DocumentTransformer, GoogleTranslateTransformer, GoogleTranslateConfig, TranslationParams};
/// use dashflow::core::documents::Document;
///
/// let config = GoogleTranslateConfig {
///     project_id: "my-project".to_string(),
///     ..Default::default()
/// };
///
/// let params = TranslationParams {
///     target_language_code: "es".to_string(),
///     ..Default::default()
/// };
///
/// let transformer = GoogleTranslateTransformer::new(config, params);
/// let docs = vec![Document::new("Hello world")];
/// let translated = transformer.transform_documents(docs).await?;
/// ```
pub struct GoogleTranslateTransformer {
    config: GoogleTranslateConfig,
    params: TranslationParams,
    client: reqwest::Client,
}

impl GoogleTranslateTransformer {
    /// Create a new Google Translate transformer.
    ///
    /// # Arguments
    ///
    /// * `config` - Google Cloud Translation API configuration
    /// * `params` - Translation parameters (source/target languages, mime type)
    ///
    /// # Returns
    ///
    /// A new `GoogleTranslateTransformer` instance
    ///
    /// # Panics
    ///
    /// Panics if the fallback HTTP client cannot be built (should be impossible).
    #[must_use]
    #[allow(clippy::expect_used)] // Fallback client builder failure is "impossible"
    pub fn new(config: GoogleTranslateConfig, params: TranslationParams) -> Self {
        let client = create_basic_client().unwrap_or_else(|_| {
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("Failed to build HTTP client")
        });

        Self {
            config,
            params,
            client,
        }
    }

    /// Get the API endpoint URL for translation requests.
    fn get_endpoint(&self) -> String {
        let base = self
            .config
            .api_endpoint
            .as_deref()
            .unwrap_or("https://translation.googleapis.com");

        let parent = format!(
            "projects/{}/locations/{}",
            self.config.project_id, self.config.location
        );

        format!("{base}/v3/{parent}:translateText")
    }

    /// Build the model path if a model ID is specified.
    fn get_model_path(&self) -> Option<String> {
        self.config.model_id.as_ref().map(|model_id| {
            format!(
                "projects/{}/locations/{}/models/{}",
                self.config.project_id, self.config.location, model_id
            )
        })
    }

    /// Build the glossary path if a glossary ID is specified.
    fn get_glossary_path(&self) -> Option<String> {
        self.config.glossary_id.as_ref().map(|glossary_id| {
            format!(
                "projects/{}/locations/{}/glossaries/{}",
                self.config.project_id, self.config.location, glossary_id
            )
        })
    }

    /// Translate documents using the Google Cloud Translation API.
    async fn translate_documents_async(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Extract content from documents
        let contents: Vec<String> = documents
            .iter()
            .map(|doc| doc.page_content.clone())
            .collect();

        // Build request
        let request = TranslateTextRequest {
            source_language_code: self.params.source_language_code.clone(),
            target_language_code: self.params.target_language_code.clone(),
            contents,
            mime_type: self.params.mime_type.clone(),
            model: self.get_model_path(),
            glossary_config: self
                .get_glossary_path()
                .map(|path| GlossaryConfig { glossary: path }),
        };

        // Get authentication token from environment or metadata service
        let token = self.get_auth_token().await?;

        // Send request
        let response = self
            .client
            .post(self.get_endpoint())
            .header("Authorization", format!("Bearer {token}"))
            .json(&request)
            .send()
            .await
            .map_err(|e| Error::Http(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(Error::Http(format!(
                "Translation API error ({status}): {error_text}"
            )));
        }

        let response_body: TranslateTextResponse = response
            .json()
            .await
            .map_err(|e| Error::Http(format!("Failed to parse response: {e}")))?;

        // Use glossary translations if available, otherwise use regular translations
        let translations = if response_body.glossary_translations.is_empty() {
            &response_body.translations
        } else {
            &response_body.glossary_translations
        };

        // Create new documents with translated content
        let translated_docs: Vec<Document> = documents
            .into_iter()
            .zip(translations.iter())
            .map(|(mut doc, translation)| {
                doc.page_content = translation.translated_text.clone();

                // Add translation metadata
                if !translation.model.is_empty() {
                    doc.metadata
                        .insert("model".to_string(), translation.model.clone().into());
                }
                if !translation.detected_language_code.is_empty() {
                    doc.metadata.insert(
                        "detected_language_code".to_string(),
                        translation.detected_language_code.clone().into(),
                    );
                }

                doc
            })
            .collect();

        Ok(translated_docs)
    }

    /// Get authentication token for Google Cloud API.
    ///
    /// This attempts to get a token from the environment variable
    /// `GOOGLE_CLOUD_ACCESS_TOKEN` or from the GCP metadata service.
    async fn get_auth_token(&self) -> Result<String> {
        // Try to get token from environment variable
        if let Some(token) = env_string(GOOGLE_CLOUD_ACCESS_TOKEN) {
            return Ok(token);
        }

        // Try to get from application default credentials
        // In production, you would use google-authz crate or similar
        // For now, return an error asking user to set the token
        Err(Error::InvalidInput(
            "Google Cloud authentication required. Set GOOGLE_CLOUD_ACCESS_TOKEN environment variable or use Application Default Credentials.".to_string()
        ))
    }
}

#[async_trait]
impl DocumentTransformer for GoogleTranslateTransformer {
    fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
        // Synchronous wrapper - not ideal but matches trait
        Err(Error::InvalidInput(
            "GoogleTranslateTransformer requires async operation. Use atransform_documents() instead.".to_string()
        ))
    }

    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        self.translate_documents_async(documents).await
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Test code uses unwrap for assertions
mod tests {
    use crate::test_prelude::*;

    #[test]
    fn test_google_translate_config_default() {
        let config = GoogleTranslateConfig::default();
        assert_eq!(config.location, "global");
        assert!(config.model_id.is_none());
        assert!(config.glossary_id.is_none());
    }

    #[test]
    fn test_translation_params_default() {
        let params = TranslationParams::default();
        assert_eq!(params.mime_type, "text/plain");
        assert!(params.source_language_code.is_none());
    }

    #[test]
    fn test_get_endpoint() {
        let config = GoogleTranslateConfig {
            project_id: "test-project".to_string(),
            location: "us-central1".to_string(),
            ..Default::default()
        };
        let params = TranslationParams::default();
        let transformer = GoogleTranslateTransformer::new(config, params);

        let endpoint = transformer.get_endpoint();
        assert!(endpoint.contains("test-project"));
        assert!(endpoint.contains("us-central1"));
        assert!(endpoint.contains(":translateText"));
    }

    #[test]
    fn test_get_model_path() {
        let config = GoogleTranslateConfig {
            project_id: "test-project".to_string(),
            location: "global".to_string(),
            model_id: Some("nmt".to_string()),
            ..Default::default()
        };
        let params = TranslationParams::default();
        let transformer = GoogleTranslateTransformer::new(config, params);

        let model_path = transformer.get_model_path();
        assert!(model_path.is_some());
        assert!(model_path.unwrap().contains("models/nmt"));
    }

    #[test]
    fn test_get_glossary_path() {
        let config = GoogleTranslateConfig {
            project_id: "test-project".to_string(),
            location: "us-central1".to_string(),
            glossary_id: Some("my-glossary".to_string()),
            ..Default::default()
        };
        let params = TranslationParams::default();
        let transformer = GoogleTranslateTransformer::new(config, params);

        let glossary_path = transformer.get_glossary_path();
        assert!(glossary_path.is_some());
        assert!(glossary_path.unwrap().contains("glossaries/my-glossary"));
    }

    #[tokio::test]
    async fn test_transform_documents_empty() {
        let config = GoogleTranslateConfig {
            project_id: "test-project".to_string(),
            ..Default::default()
        };
        let params = TranslationParams {
            target_language_code: "es".to_string(),
            ..Default::default()
        };
        let transformer = GoogleTranslateTransformer::new(config, params);

        let result = transformer.atransform_documents(vec![]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    // =========================================================================
    // GoogleTranslateConfig Builder Pattern Tests
    // =========================================================================

    #[test]
    fn test_google_translate_config_new() {
        let config = GoogleTranslateConfig::new("my-project");
        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.location, "global");
        assert!(config.model_id.is_none());
        assert!(config.glossary_id.is_none());
        assert!(config.api_endpoint.is_none());
    }

    #[test]
    fn test_google_translate_config_builder_project_id() {
        let config = GoogleTranslateConfig::default().with_project_id("new-project");
        assert_eq!(config.project_id, "new-project");
    }

    #[test]
    fn test_google_translate_config_builder_location() {
        let config = GoogleTranslateConfig::new("proj").with_location("us-east1");
        assert_eq!(config.location, "us-east1");
    }

    #[test]
    fn test_google_translate_config_builder_model_id() {
        let config = GoogleTranslateConfig::new("proj").with_model_id("nmt");
        assert_eq!(config.model_id, Some("nmt".to_string()));
    }

    #[test]
    fn test_google_translate_config_builder_glossary_id() {
        let config = GoogleTranslateConfig::new("proj").with_glossary_id("my-glossary");
        assert_eq!(config.glossary_id, Some("my-glossary".to_string()));
    }

    #[test]
    fn test_google_translate_config_builder_api_endpoint() {
        let config =
            GoogleTranslateConfig::new("proj").with_api_endpoint("https://custom.endpoint.com");
        assert_eq!(
            config.api_endpoint,
            Some("https://custom.endpoint.com".to_string())
        );
    }

    #[test]
    fn test_google_translate_config_builder_chaining() {
        let config = GoogleTranslateConfig::new("my-project")
            .with_location("europe-west1")
            .with_model_id("custom-model")
            .with_glossary_id("my-glossary")
            .with_api_endpoint("https://custom.api.com");

        assert_eq!(config.project_id, "my-project");
        assert_eq!(config.location, "europe-west1");
        assert_eq!(config.model_id, Some("custom-model".to_string()));
        assert_eq!(config.glossary_id, Some("my-glossary".to_string()));
        assert_eq!(
            config.api_endpoint,
            Some("https://custom.api.com".to_string())
        );
    }

    #[test]
    fn test_google_translate_config_validate_success() {
        let config = GoogleTranslateConfig::new("valid-project");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_google_translate_config_validate_empty_project_id() {
        let config = GoogleTranslateConfig::default();
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("project_id is required"));
    }

    // =========================================================================
    // TranslationParams Builder Pattern Tests
    // =========================================================================

    #[test]
    fn test_translation_params_new() {
        let params = TranslationParams::new("es");
        assert!(params.source_language_code.is_none());
        assert_eq!(params.target_language_code, "es");
        assert_eq!(params.mime_type, "text/plain");
    }

    #[test]
    fn test_translation_params_builder_source_language() {
        let params = TranslationParams::new("es").with_source_language("en");
        assert_eq!(params.source_language_code, Some("en".to_string()));
    }

    #[test]
    fn test_translation_params_builder_target_language() {
        let params = TranslationParams::default().with_target_language("fr");
        assert_eq!(params.target_language_code, "fr");
    }

    #[test]
    fn test_translation_params_builder_mime_type() {
        let params = TranslationParams::new("de").with_mime_type("text/html");
        assert_eq!(params.mime_type, "text/html");
    }

    #[test]
    fn test_translation_params_builder_chaining() {
        let params = TranslationParams::new("ja")
            .with_source_language("en")
            .with_mime_type("text/html");

        assert_eq!(params.source_language_code, Some("en".to_string()));
        assert_eq!(params.target_language_code, "ja");
        assert_eq!(params.mime_type, "text/html");
    }

    #[test]
    fn test_translation_params_validate_success() {
        let params = TranslationParams::new("es");
        assert!(params.validate().is_ok());
    }

    #[test]
    fn test_translation_params_validate_empty_target() {
        let params = TranslationParams::default();
        let result = params.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("target_language_code is required"));
    }
}
