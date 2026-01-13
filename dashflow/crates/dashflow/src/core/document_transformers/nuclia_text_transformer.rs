// Removed broad #![allow(clippy::expect_used)] - targeted allows used instead.

//! Nuclia Understanding API transformer for documents.
//!
//! This transformer uses the Nuclia Understanding API to analyze text,
//! extracting entities, generating summaries, and creating embeddings.

use crate::core::document_transformers::DocumentTransformer;
use crate::core::documents::Document;
use crate::core::error::{Error, Result};
use crate::core::http_client::create_basic_client;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for Nuclia Understanding API.
#[derive(Debug, Clone)]
pub struct NucliaConfig {
    /// Nuclia API key
    pub api_key: String,
    /// API endpoint URL (default: <https://nuclia.cloud/api/v1/understand>)
    pub api_url: String,
}

impl Default for NucliaConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            api_url: "https://nuclia.cloud/api/v1/understand".to_string(),
        }
    }
}

/// Request to Nuclia Understanding API.
#[derive(Debug, Serialize)]
struct NucliaRequest {
    action: String,
    id: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}

/// Response from Nuclia Understanding API.
#[derive(Debug, Deserialize)]
struct NucliaResponse {
    file_extracted_data: Vec<FileExtractedData>,
    field_metadata: Vec<FieldMetadata>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FileExtractedData {
    #[serde(default)]
    paragraphs: Vec<Paragraph>,
    #[serde(default)]
    sentences: Vec<Sentence>,
    #[serde(default)]
    entities: Vec<Entity>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Paragraph {
    text: String,
    #[serde(default)]
    start: usize,
    #[serde(default)]
    end: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Sentence {
    text: String,
    #[serde(default)]
    start: usize,
    #[serde(default)]
    end: usize,
    #[serde(default)]
    embeddings: Vec<f32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Entity {
    text: String,
    #[serde(rename = "type")]
    entity_type: String,
    #[serde(default)]
    start: usize,
    #[serde(default)]
    end: usize,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct FieldMetadata {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    language: String,
}

/// Document transformer that analyzes text using Nuclia Understanding API.
///
/// The Nuclia Understanding API provides:
/// - Text segmentation into paragraphs and sentences
/// - Named entity recognition
/// - Text summarization
/// - Sentence embeddings
///
/// This transformer adds all this information to document metadata.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::document_transformers::{DocumentTransformer, NucliaTextTransformer, NucliaConfig};
/// use dashflow::core::documents::Document;
///
/// let config = NucliaConfig {
///     api_key: "your-api-key".to_string(),
///     ..Default::default()
/// };
///
/// let transformer = NucliaTextTransformer::new(config);
/// let docs = vec![Document::new("Your text here")];
/// let analyzed = transformer.atransform_documents(docs).await?;
/// ```
pub struct NucliaTextTransformer {
    config: NucliaConfig,
    client: reqwest::Client,
}

impl NucliaTextTransformer {
    /// Create a new Nuclia text transformer.
    ///
    /// # Arguments
    ///
    /// * `config` - Nuclia API configuration including API key
    ///
    /// # Returns
    ///
    /// A new `NucliaTextTransformer` instance
    #[must_use]
    #[allow(clippy::expect_used)] // Fallback HTTP client builder - only fails if TLS backend missing
    pub fn new(config: NucliaConfig) -> Self {
        let client = create_basic_client().unwrap_or_else(|_| {
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("Failed to build HTTP client")
        });

        Self { config, client }
    }

    /// Analyze a single document using Nuclia Understanding API.
    async fn analyze_document(&self, document: &Document) -> Result<NucliaResponse> {
        let request = NucliaRequest {
            action: "push".to_string(),
            id: Uuid::new_v4().to_string(),
            text: document.page_content.clone(),
            path: None,
        };

        let response = self
            .client
            .post(&self.config.api_url)
            .header("X-NUCLIA-APIKEY", &self.config.api_key)
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
                "Nuclia API error ({status}): {error_text}"
            )));
        }

        let response_body: NucliaResponse = response
            .json()
            .await
            .map_err(|e| Error::Http(format!("Failed to parse response: {e}")))?;

        Ok(response_body)
    }

    /// Transform documents using Nuclia Understanding API.
    async fn transform_documents_async(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        if documents.is_empty() {
            return Ok(Vec::new());
        }

        // Process all documents concurrently
        let mut tasks = Vec::new();
        for doc in &documents {
            tasks.push(self.analyze_document(doc));
        }

        let results = futures::future::join_all(tasks).await;

        // Combine results with original documents
        let mut transformed_docs = Vec::new();
        for (mut doc, result) in documents.into_iter().zip(results.into_iter()) {
            match result {
                Ok(response) => {
                    // Add Nuclia analysis to metadata
                    if !response.file_extracted_data.is_empty() {
                        let file_data = &response.file_extracted_data[0];
                        doc.metadata.insert(
                            "nuclia_file_data".to_string(),
                            serde_json::to_value(file_data).unwrap_or(serde_json::Value::Null),
                        );
                    }

                    if !response.field_metadata.is_empty() {
                        let field_meta = &response.field_metadata[0];
                        doc.metadata.insert(
                            "nuclia_metadata".to_string(),
                            serde_json::to_value(field_meta).unwrap_or(serde_json::Value::Null),
                        );

                        // Also add summary and language as top-level metadata
                        if !field_meta.summary.is_empty() {
                            doc.metadata
                                .insert("summary".to_string(), field_meta.summary.clone().into());
                        }
                        if !field_meta.language.is_empty() {
                            doc.metadata
                                .insert("language".to_string(), field_meta.language.clone().into());
                        }
                    }

                    transformed_docs.push(doc);
                }
                Err(e) => {
                    // On error, include original document with error in metadata
                    doc.metadata
                        .insert("nuclia_error".to_string(), e.to_string().into());
                    transformed_docs.push(doc);
                }
            }
        }

        Ok(transformed_docs)
    }
}

#[async_trait]
impl DocumentTransformer for NucliaTextTransformer {
    fn transform_documents(&self, _documents: Vec<Document>) -> Result<Vec<Document>> {
        // Nuclia API is async-only
        Err(Error::InvalidInput(
            "NucliaTextTransformer requires async operation. Use atransform_documents() instead."
                .to_string(),
        ))
    }

    async fn atransform_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>> {
        self.transform_documents_async(documents).await
    }
}

#[cfg(test)]
mod tests {
    use super::NucliaConfig;
    use crate::test_prelude::*;

    #[test]
    fn test_nuclia_config_default() {
        let config = NucliaConfig::default();
        assert!(config.api_url.contains("nuclia.cloud"));
        assert_eq!(config.api_key, "");
    }

    #[test]
    fn test_nuclia_transformer_creation() {
        let config = NucliaConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let transformer = NucliaTextTransformer::new(config);
        assert_eq!(transformer.config.api_key, "test-key");
    }

    #[tokio::test]
    async fn test_transform_documents_empty() {
        let config = NucliaConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let transformer = NucliaTextTransformer::new(config);

        let result = transformer.atransform_documents(vec![]).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 0);
    }

    #[test]
    fn test_sync_transform_returns_error() {
        let config = NucliaConfig {
            api_key: "test-key".to_string(),
            ..Default::default()
        };
        let transformer = NucliaTextTransformer::new(config);

        let result = transformer.transform_documents(vec![Document::new("test")]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("requires async operation"));
    }
}
