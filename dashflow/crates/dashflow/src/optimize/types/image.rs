// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Image type for vision models (GPT-4V, Claude 3, Gemini, etc.)

use super::{ImageSource, LlmContent, ToLlmContent};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Supported image formats for vision-capable LLMs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// PNG - Portable Network Graphics (lossless, supports transparency)
    Png,
    /// JPEG - Joint Photographic Experts Group (lossy, good for photos)
    Jpeg,
    /// GIF - Graphics Interchange Format (supports animation, limited colors)
    Gif,
    /// WebP - Modern format with good compression (supports transparency)
    Webp,
}

impl ImageFormat {
    /// Get MIME type for this format
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }

    /// Detect format from file extension
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    /// Detect format from MIME type
    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime.to_lowercase().as_str() {
            "image/png" => Some(Self::Png),
            "image/jpeg" | "image/jpg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }
}

/// Image input for vision models
///
/// Supports both URL-based and base64-encoded images for use with
/// vision-capable LLMs like GPT-4V, Claude 3, and Gemini.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::optimize::types::Image;
///
/// // From URL
/// let img = Image::from_url("https://example.com/photo.jpg")?;
///
/// // From file
/// let img = Image::from_file("screenshot.png").await?;
///
/// // From bytes
/// let img = Image::from_bytes(&bytes, ImageFormat::Png);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Image source (URL or base64 data)
    #[serde(flatten)]
    source: ImageSourceData,

    /// Optional detail level (for OpenAI: "low", "high", "auto")
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,

    /// Optional alt text for accessibility
    #[serde(skip_serializing_if = "Option::is_none")]
    alt: Option<String>,
}

/// Internal image source representation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum ImageSourceData {
    Url { url: String },
    Base64 { data: String, format: ImageFormat },
}

impl Image {
    /// Create image from URL
    ///
    /// # Arguments
    /// * `url` - URL to the image
    ///
    /// # Example
    /// ```rust,ignore
    /// let img = Image::from_url("https://example.com/image.png");
    /// ```
    pub fn from_url(url: impl Into<String>) -> Self {
        Self {
            source: ImageSourceData::Url { url: url.into() },
            detail: None,
            alt: None,
        }
    }

    /// Create image from base64-encoded data
    ///
    /// # Arguments
    /// * `data` - Base64-encoded image data
    /// * `format` - Image format
    pub fn from_base64(data: impl Into<String>, format: ImageFormat) -> Self {
        Self {
            source: ImageSourceData::Base64 {
                data: data.into(),
                format,
            },
            detail: None,
            alt: None,
        }
    }

    /// Create image from raw bytes
    ///
    /// # Arguments
    /// * `bytes` - Raw image bytes
    /// * `format` - Image format
    pub fn from_bytes(bytes: &[u8], format: ImageFormat) -> Self {
        let data = BASE64.encode(bytes);
        Self::from_base64(data, format)
    }

    /// Load image from file path
    ///
    /// Automatically detects format from file extension.
    ///
    /// # Arguments
    /// * `path` - Path to the image file
    ///
    /// # Returns
    /// Result with Image or error if file cannot be read or format unknown
    pub fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let path = path.as_ref();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or_default();

        let format = ImageFormat::from_extension(ext).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Unknown image format: {}", ext),
            )
        })?;

        let bytes = std::fs::read(path)?;
        Ok(Self::from_bytes(&bytes, format))
    }

    /// Set detail level (OpenAI-specific)
    ///
    /// # Arguments
    /// * `detail` - Detail level: "low", "high", or "auto"
    #[must_use]
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Set alt text
    ///
    /// # Arguments
    /// * `alt` - Alternative text description
    #[must_use]
    pub fn with_alt(mut self, alt: impl Into<String>) -> Self {
        self.alt = Some(alt.into());
        self
    }

    /// Get the image URL if this is a URL-based image
    pub fn url(&self) -> Option<&str> {
        match &self.source {
            ImageSourceData::Url { url } => Some(url),
            ImageSourceData::Base64 { .. } => None,
        }
    }

    /// Get the base64 data if this is a base64-encoded image
    pub fn base64_data(&self) -> Option<&str> {
        match &self.source {
            ImageSourceData::Url { .. } => None,
            ImageSourceData::Base64 { data, .. } => Some(data),
        }
    }

    /// Get the image format if this is a base64-encoded image
    pub fn format(&self) -> Option<ImageFormat> {
        match &self.source {
            ImageSourceData::Url { .. } => None,
            ImageSourceData::Base64 { format, .. } => Some(*format),
        }
    }

    /// Check if this is a URL-based image
    pub fn is_url(&self) -> bool {
        matches!(self.source, ImageSourceData::Url { .. })
    }

    /// Check if this is a base64-encoded image
    pub fn is_base64(&self) -> bool {
        matches!(self.source, ImageSourceData::Base64 { .. })
    }
}

impl ToLlmContent for Image {
    fn to_content(&self) -> LlmContent {
        match &self.source {
            ImageSourceData::Url { url } => LlmContent::Image {
                source: ImageSource::Url { url: url.clone() },
                detail: self.detail.clone(),
            },
            ImageSourceData::Base64 { data, format } => LlmContent::Image {
                source: ImageSource::Base64 {
                    media_type: format.mime_type().to_string(),
                    data: data.clone(),
                },
                detail: self.detail.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_from_url() {
        let img = Image::from_url("https://example.com/image.png");
        assert!(img.is_url());
        assert!(!img.is_base64());
        assert_eq!(img.url(), Some("https://example.com/image.png"));
    }

    #[test]
    fn test_image_from_base64() {
        let img = Image::from_base64("aGVsbG8=", ImageFormat::Png);
        assert!(img.is_base64());
        assert!(!img.is_url());
        assert_eq!(img.base64_data(), Some("aGVsbG8="));
        assert_eq!(img.format(), Some(ImageFormat::Png));
    }

    #[test]
    fn test_image_from_bytes() {
        let bytes = b"test image data";
        let img = Image::from_bytes(bytes, ImageFormat::Jpeg);
        assert!(img.is_base64());
        assert_eq!(img.format(), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn test_image_with_detail() {
        let img = Image::from_url("https://example.com/img.png").with_detail("high");
        assert_eq!(img.detail, Some("high".to_string()));
    }

    #[test]
    fn test_image_with_alt() {
        let img = Image::from_url("https://example.com/img.png").with_alt("A test image");
        assert_eq!(img.alt, Some("A test image".to_string()));
    }

    #[test]
    fn test_image_format_detection() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::Webp));
        assert_eq!(ImageFormat::from_extension("bmp"), None);
    }

    #[test]
    fn test_image_format_mime() {
        assert_eq!(ImageFormat::Png.mime_type(), "image/png");
        assert_eq!(ImageFormat::Jpeg.mime_type(), "image/jpeg");
        assert_eq!(ImageFormat::Gif.mime_type(), "image/gif");
        assert_eq!(ImageFormat::Webp.mime_type(), "image/webp");
    }

    #[test]
    fn test_to_llm_content_url() {
        let img = Image::from_url("https://example.com/img.png");
        let content = img.to_content();
        match content {
            LlmContent::Image { source, .. } => match source {
                ImageSource::Url { url } => assert_eq!(url, "https://example.com/img.png"),
                _ => panic!("Expected Url variant"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_to_llm_content_base64() {
        let img = Image::from_base64("dGVzdA==", ImageFormat::Png);
        let content = img.to_content();
        match content {
            LlmContent::Image { source, .. } => match source {
                ImageSource::Base64 { media_type, data } => {
                    assert_eq!(media_type, "image/png");
                    assert_eq!(data, "dGVzdA==");
                }
                _ => panic!("Expected Base64 variant"),
            },
            _ => panic!("Expected Image variant"),
        }
    }

    #[test]
    fn test_serialization() {
        let img = Image::from_url("https://example.com/img.png");
        let json = serde_json::to_string(&img).unwrap();
        assert!(json.contains("https://example.com/img.png"));

        let deserialized: Image = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.url(), Some("https://example.com/img.png"));
    }
}
