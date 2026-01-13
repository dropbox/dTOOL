// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! @dashflow-module
//! @name types
//! @category optimize
//! @status stable
//!
//! # DashOptimize Types - Multimodal & Specialized Field Types
//!
//! This module provides specialized types for multimodal LLM workflows,
//! enabling optimization of vision, audio, and other advanced capabilities.
//!
//! ## Core Types
//!
//! - [`Image`]: Image inputs for vision models (GPT-4V, Claude 3, etc.)
//! - [`Audio`]: Audio inputs for audio-capable LLMs (Gemini, GPT-4o-audio)
//! - [`File`]: File inputs (PDFs, documents) with base64 encoding
//! - [`Citation`]: Source citations for RAG (Anthropic Citations API)
//! - [`Document`]: Citation-enabled documents for retrieval
//! - [`Code`]: Code generation with language tagging
//! - [`History`]: Conversation history for multi-turn interactions
//! - [`Reasoning`]: Native reasoning support (o1-series models)
//! - [`ToolCall`]: Function calling for agentic workflows
//!
//! ## Example
//!
//! ```rust,ignore
//! use dashflow::optimize::types::*;
//!
//! // Image input for vision model
//! let image = Image::from_url("https://example.com/image.png")?;
//!
//! // Audio input
//! let audio = Audio::from_file("recording.wav")?;
//!
//! // Citation for RAG
//! let citation = Citation::new("Source Document", "https://source.com/doc.pdf")
//!     .with_page(42);
//! ```

mod audio;
mod citation;
mod code;
mod document;
mod file;
mod history;
mod image;
mod reasoning;
mod tool;

pub use audio::{Audio, AudioFormat};
pub use citation::Citation;
pub use code::{Code, Language};
pub use document::Document;
pub use file::{File, FileType};
pub use history::{History, Message, Role};
pub use image::{Image, ImageFormat};
pub use reasoning::{Reasoning, ReasoningEffort, ReasoningOutput, ReasoningStep};
pub use tool::{ToolCall, ToolCalls, ToolResult};

/// Trait for types that can be converted to LLM message content
pub trait ToLlmContent {
    /// Convert to a content block for LLM APIs
    fn to_content(&self) -> LlmContent;
}

/// LLM content block (text, image, audio, etc.)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum LlmContent {
    /// Plain text content
    #[serde(rename = "text")]
    Text {
        /// The text content string.
        text: String,
    },

    /// Image content (base64 or URL)
    #[serde(rename = "image")]
    Image {
        /// The image source (URL or base64 data).
        source: ImageSource,
        /// Optional detail level for vision models (e.g., "high", "low").
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },

    /// Audio content (base64)
    #[serde(rename = "audio")]
    Audio {
        /// Base64-encoded audio data.
        data: String,
        /// Audio format (e.g., "mp3", "wav", "ogg").
        format: String,
    },

    /// File content (base64)
    #[serde(rename = "file")]
    File {
        /// Base64-encoded file data.
        data: String,
        /// MIME type of the file (e.g., "application/pdf").
        media_type: String,
        /// Optional original filename for display.
        #[serde(skip_serializing_if = "Option::is_none")]
        filename: Option<String>,
    },

    /// Tool use request
    #[serde(rename = "tool_use")]
    ToolUse {
        /// Unique identifier for this tool invocation.
        id: String,
        /// Name of the tool to invoke.
        name: String,
        /// JSON input arguments for the tool.
        input: serde_json::Value,
    },

    /// Tool result
    #[serde(rename = "tool_result")]
    ToolResult {
        /// ID of the tool use request this result corresponds to.
        tool_use_id: String,
        /// The string content returned by the tool.
        content: String,
        /// Whether this result represents an error.
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Image source (URL or base64)
///
/// Specifies how image data is provided to the LLM - either as a URL reference
/// or as inline base64-encoded data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type")]
pub enum ImageSource {
    /// Image referenced by URL.
    ///
    /// The LLM will fetch the image from the provided URL.
    #[serde(rename = "url")]
    Url {
        /// The URL pointing to the image resource.
        url: String,
    },

    /// Image provided as base64-encoded data.
    ///
    /// The image data is embedded directly in the request.
    #[serde(rename = "base64")]
    Base64 {
        /// The MIME type of the image (e.g., "image/png", "image/jpeg").
        media_type: String,
        /// The base64-encoded image data.
        data: String,
    },
}

impl LlmContent {
    /// Create text content
    pub fn text(text: impl Into<String>) -> Self {
        Self::Text { text: text.into() }
    }

    /// Create image content from URL
    pub fn image_url(url: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Url { url: url.into() },
            detail: None,
        }
    }

    /// Create image content from base64
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Image {
            source: ImageSource::Base64 {
                media_type: media_type.into(),
                data: data.into(),
            },
            detail: None,
        }
    }

    /// Set image detail level (for OpenAI: "low", "high", "auto")
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        if let Self::Image {
            detail: ref mut d, ..
        } = self
        {
            *d = Some(detail.into());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llm_content_text() {
        let content = LlmContent::text("Hello, world!");
        match content {
            LlmContent::Text { text } => assert_eq!(text, "Hello, world!"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_image_url() {
        let content = LlmContent::image_url("https://example.com/image.png");
        match content {
            LlmContent::Image { source, .. } => match source {
                ImageSource::Url { url } => assert_eq!(url, "https://example.com/image.png"),
                _ => panic!("Expected Url variant"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_serialization() {
        let content = LlmContent::text("Test");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        assert!(json.contains("\"text\":\"Test\""));
    }

    // ============================================================================
    // Additional comprehensive tests
    // ============================================================================

    #[test]
    fn test_llm_content_text_empty() {
        let content = LlmContent::text("");
        match content {
            LlmContent::Text { text } => assert_eq!(text, ""),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_text_unicode() {
        let content = LlmContent::text("こんにちは 🌍 مرحبا");
        match content {
            LlmContent::Text { text } => assert_eq!(text, "こんにちは 🌍 مرحبا"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_text_special_chars() {
        let content = LlmContent::text("Line1\nLine2\tTabbed");
        match content {
            LlmContent::Text { text } => assert_eq!(text, "Line1\nLine2\tTabbed"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_text_from_string() {
        let s = String::from("Dynamic string");
        let content = LlmContent::text(s);
        match content {
            LlmContent::Text { text } => assert_eq!(text, "Dynamic string"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_image_base64() {
        let content = LlmContent::image_base64("image/png", "iVBORw0KGgoAAAANS...");
        match content {
            LlmContent::Image { source, detail } => {
                match source {
                    ImageSource::Base64 { media_type, data } => {
                        assert_eq!(media_type, "image/png");
                        assert_eq!(data, "iVBORw0KGgoAAAANS...");
                    }
                    _ => panic!("Expected Base64 variant"),
                }
                assert!(detail.is_none());
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_image_with_detail_low() {
        let content = LlmContent::image_url("https://example.com/image.png").with_detail("low");
        match content {
            LlmContent::Image { detail, .. } => {
                assert_eq!(detail, Some("low".to_string()));
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_image_with_detail_high() {
        let content = LlmContent::image_url("https://example.com/image.png").with_detail("high");
        match content {
            LlmContent::Image { detail, .. } => {
                assert_eq!(detail, Some("high".to_string()));
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_image_with_detail_auto() {
        let content = LlmContent::image_url("https://example.com/image.png").with_detail("auto");
        match content {
            LlmContent::Image { detail, .. } => {
                assert_eq!(detail, Some("auto".to_string()));
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_with_detail_on_non_image() {
        // with_detail should be no-op for non-Image variants
        let content = LlmContent::text("Hello").with_detail("high");
        match content {
            LlmContent::Text { text } => assert_eq!(text, "Hello"),
            _ => panic!("Expected Text variant to remain unchanged"),
        }
    }

    #[test]
    fn test_llm_content_clone() {
        let original = LlmContent::text("Test");
        let cloned = original.clone();
        match cloned {
            LlmContent::Text { text } => assert_eq!(text, "Test"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_debug() {
        let content = LlmContent::text("Debug test");
        let debug_str = format!("{:?}", content);
        assert!(debug_str.contains("Text"));
        assert!(debug_str.contains("Debug test"));
    }

    #[test]
    fn test_llm_content_image_url_serialization() {
        let content = LlmContent::image_url("https://example.com/photo.jpg");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"url\":\"https://example.com/photo.jpg\""));
    }

    #[test]
    fn test_llm_content_image_base64_serialization() {
        let content = LlmContent::image_base64("image/jpeg", "base64data");
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"image\""));
        assert!(json.contains("\"media_type\":\"image/jpeg\""));
        assert!(json.contains("\"data\":\"base64data\""));
    }

    #[test]
    fn test_llm_content_text_deserialization() {
        let json = r#"{"type":"text","text":"Deserialized"}"#;
        let content: LlmContent = serde_json::from_str(json).unwrap();
        match content {
            LlmContent::Text { text } => assert_eq!(text, "Deserialized"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_image_url_deserialization() {
        let json = r#"{"type":"image","source":{"type":"url","url":"https://test.com/img.png"}}"#;
        let content: LlmContent = serde_json::from_str(json).unwrap();
        match content {
            LlmContent::Image { source, .. } => match source {
                ImageSource::Url { url } => assert_eq!(url, "https://test.com/img.png"),
                _ => panic!("Expected Url variant"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_image_base64_deserialization() {
        let json = r#"{"type":"image","source":{"type":"base64","media_type":"image/png","data":"abc123"}}"#;
        let content: LlmContent = serde_json::from_str(json).unwrap();
        match content {
            LlmContent::Image { source, .. } => match source {
                ImageSource::Base64 { media_type, data } => {
                    assert_eq!(media_type, "image/png");
                    assert_eq!(data, "abc123");
                }
                _ => panic!("Expected Base64 variant"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_audio() {
        let content = LlmContent::Audio {
            data: "audio_base64_data".to_string(),
            format: "wav".to_string(),
        };
        match content {
            LlmContent::Audio { data, format } => {
                assert_eq!(data, "audio_base64_data");
                assert_eq!(format, "wav");
            }
            _ => panic!("Expected Audio variant"),
        }
    }

    #[test]
    fn test_llm_content_audio_serialization() {
        let content = LlmContent::Audio {
            data: "encoded_audio".to_string(),
            format: "mp3".to_string(),
        };
        let json = serde_json::to_string(&content).unwrap();
        assert!(json.contains("\"type\":\"audio\""));
        assert!(json.contains("\"data\":\"encoded_audio\""));
        assert!(json.contains("\"format\":\"mp3\""));
    }

    #[test]
    fn test_llm_content_file() {
        let content = LlmContent::File {
            data: "pdf_base64_data".to_string(),
            media_type: "application/pdf".to_string(),
            filename: Some("document.pdf".to_string()),
        };
        match content {
            LlmContent::File {
                data,
                media_type,
                filename,
            } => {
                assert_eq!(data, "pdf_base64_data");
                assert_eq!(media_type, "application/pdf");
                assert_eq!(filename, Some("document.pdf".to_string()));
            }
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_llm_content_file_without_filename() {
        let content = LlmContent::File {
            data: "data".to_string(),
            media_type: "application/json".to_string(),
            filename: None,
        };
        match content {
            LlmContent::File { filename, .. } => {
                assert!(filename.is_none());
            }
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_llm_content_tool_use() {
        let content = LlmContent::ToolUse {
            id: "tool_123".to_string(),
            name: "get_weather".to_string(),
            input: serde_json::json!({"location": "San Francisco"}),
        };
        match content {
            LlmContent::ToolUse { id, name, input } => {
                assert_eq!(id, "tool_123");
                assert_eq!(name, "get_weather");
                assert_eq!(input["location"], "San Francisco");
            }
            _ => panic!("Expected ToolUse variant"),
        }
    }

    #[test]
    fn test_llm_content_tool_result_success() {
        let content = LlmContent::ToolResult {
            tool_use_id: "tool_123".to_string(),
            content: "72°F and sunny".to_string(),
            is_error: None,
        };
        match content {
            LlmContent::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => {
                assert_eq!(tool_use_id, "tool_123");
                assert_eq!(content, "72°F and sunny");
                assert!(is_error.is_none());
            }
            _ => panic!("Expected ToolResult variant"),
        }
    }

    #[test]
    fn test_llm_content_tool_result_error() {
        let content = LlmContent::ToolResult {
            tool_use_id: "tool_456".to_string(),
            content: "Location not found".to_string(),
            is_error: Some(true),
        };
        match content {
            LlmContent::ToolResult {
                tool_use_id,
                is_error,
                ..
            } => {
                assert_eq!(tool_use_id, "tool_456");
                assert_eq!(is_error, Some(true));
            }
            _ => panic!("Expected ToolResult variant"),
        }
    }

    #[test]
    fn test_image_source_url_clone() {
        let source = ImageSource::Url {
            url: "https://test.com/img.png".to_string(),
        };
        let cloned = source.clone();
        match cloned {
            ImageSource::Url { url } => assert_eq!(url, "https://test.com/img.png"),
            _ => panic!("Expected Url variant"),
        }
    }

    #[test]
    fn test_image_source_base64_clone() {
        let source = ImageSource::Base64 {
            media_type: "image/gif".to_string(),
            data: "R0lGODlh".to_string(),
        };
        let cloned = source.clone();
        match cloned {
            ImageSource::Base64 { media_type, data } => {
                assert_eq!(media_type, "image/gif");
                assert_eq!(data, "R0lGODlh");
            }
            _ => panic!("Expected Base64 variant"),
        }
    }

    #[test]
    fn test_image_source_debug() {
        let source = ImageSource::Url {
            url: "https://debug.com".to_string(),
        };
        let debug_str = format!("{:?}", source);
        assert!(debug_str.contains("Url"));
        assert!(debug_str.contains("https://debug.com"));
    }

    #[test]
    fn test_llm_content_roundtrip_text() {
        let original = LlmContent::text("Roundtrip test");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: LlmContent = serde_json::from_str(&json).unwrap();
        match deserialized {
            LlmContent::Text { text } => assert_eq!(text, "Roundtrip test"),
            _ => panic!("Expected Text variant"),
        }
    }

    #[test]
    fn test_llm_content_roundtrip_image_with_detail() {
        let original = LlmContent::image_base64("image/webp", "webp_data").with_detail("high");
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: LlmContent = serde_json::from_str(&json).unwrap();
        match deserialized {
            LlmContent::Image { source, detail } => {
                match source {
                    ImageSource::Base64 { media_type, data } => {
                        assert_eq!(media_type, "image/webp");
                        assert_eq!(data, "webp_data");
                    }
                    _ => panic!("Expected Base64 variant"),
                }
                assert_eq!(detail, Some("high".to_string()));
            }
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_llm_content_all_variants_serializable() {
        // Ensure all variants can be serialized without panic
        let variants = vec![
            LlmContent::text("text"),
            LlmContent::image_url("https://url.com"),
            LlmContent::image_base64("image/png", "data"),
            LlmContent::Audio {
                data: "audio".to_string(),
                format: "wav".to_string(),
            },
            LlmContent::File {
                data: "file".to_string(),
                media_type: "application/pdf".to_string(),
                filename: None,
            },
            LlmContent::ToolUse {
                id: "id".to_string(),
                name: "name".to_string(),
                input: serde_json::json!({}),
            },
            LlmContent::ToolResult {
                tool_use_id: "id".to_string(),
                content: "result".to_string(),
                is_error: None,
            },
        ];

        for variant in variants {
            let json = serde_json::to_string(&variant);
            assert!(json.is_ok(), "Failed to serialize variant: {:?}", variant);
        }
    }
}
