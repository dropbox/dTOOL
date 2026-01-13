// Allow clippy warnings for YouTube transcript loader
// - needless_pass_by_value: Video ID passed by value for API calls
// - unwrap_used: Regex matching on known valid patterns
#![allow(clippy::needless_pass_by_value, clippy::unwrap_used)]

//! YouTube transcript loader
//!
//! Load transcripts from YouTube videos as documents for RAG applications.
//! Uses YouTube's innertube API to fetch available transcripts.
//!
//! # Python Baseline
//!
//! This implements functionality from:
//! `dashflow_community.document_loaders.youtube`

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::documents::{Document, DocumentLoader};
use dashflow::core::Result;
use quick_xml::events::Event;
use quick_xml::Reader;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

use crate::extract_video_id;

/// Static regex for sentence splitting (compiled once).
static SENTENCE_SPLIT_REGEX: OnceLock<Regex> = OnceLock::new();

// SAFETY: Regex literal is hardcoded and compile-time valid
#[allow(clippy::expect_used)]
fn get_sentence_split_regex() -> &'static Regex {
    SENTENCE_SPLIT_REGEX
        .get_or_init(|| Regex::new(r"[.!?]+\s+").expect("SENTENCE_SPLIT_REGEX pattern is valid"))
}

/// A chunk of transcript text with timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptChunk {
    /// The transcript text
    pub text: String,
    /// Start time in seconds
    pub start: f64,
    /// Duration in seconds
    pub duration: f64,
}

/// Language information for a transcript
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Language {
    /// Language code (e.g., "en", "es", "fr")
    pub code: String,
    /// Language name (e.g., "English", "Spanish", "French")
    pub name: String,
    /// Whether this is an auto-generated transcript
    pub is_auto_generated: bool,
}

/// YouTube transcript loader for DashFlow Rust
///
/// Loads video transcripts as documents with timing metadata.
/// Supports both manual captions and auto-generated transcripts.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_youtube::YouTubeTranscriptLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = YouTubeTranscriptLoader::builder()
///     .language("en".to_string())
///     .chunk_by_sentences(true)
///     .build();
///
/// let docs = loader.load_from_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").await?;
///
/// for doc in docs {
///     println!("Content: {}", doc.page_content);
///     println!("Start: {:?}", doc.metadata.get("start_time"));
/// }
/// # Ok(())
/// # }
/// ```
pub struct YouTubeTranscriptLoader {
    /// Preferred language code for transcripts (e.g., "en")
    language: Option<String>,
    /// Whether to chunk the transcript by sentences
    chunk_by_sentences: bool,
    /// Whether to include timestamps in the document content
    include_timestamps: bool,
    /// Whether to prefer auto-generated transcripts when manual not available
    allow_auto_generated: bool,
}

impl YouTubeTranscriptLoader {
    /// Create a new transcript loader with default settings
    ///
    /// Default settings:
    /// - `language`: None (uses video's default language)
    /// - `chunk_by_sentences`: false
    /// - `include_timestamps`: false
    /// - `allow_auto_generated`: true
    #[must_use]
    pub fn new() -> Self {
        Self {
            language: None,
            chunk_by_sentences: false,
            include_timestamps: false,
            allow_auto_generated: true,
        }
    }

    /// Create a builder for `YouTubeTranscriptLoader`
    #[must_use]
    pub fn builder() -> YouTubeTranscriptLoaderBuilder {
        YouTubeTranscriptLoaderBuilder::default()
    }

    /// Load transcript from a YouTube video URL or video ID
    ///
    /// # Arguments
    ///
    /// * `url_or_id` - A YouTube video URL or video ID
    ///
    /// # Returns
    ///
    /// A vector of Documents containing the transcript
    pub async fn load_from_url(&self, url_or_id: &str) -> Result<Vec<Document>> {
        let video_id = extract_video_id(url_or_id).ok_or_else(|| {
            dashflow::core::Error::other(format!("Could not extract video ID from: {url_or_id}"))
        })?;

        self.load_transcript(&video_id).await
    }

    /// Load transcript for a specific video ID
    async fn load_transcript(&self, video_id: &str) -> Result<Vec<Document>> {
        let client = create_http_client();

        // First, get the video page to extract the initial player response
        let video_url = format!("https://www.youtube.com/watch?v={video_id}");
        let response = client
            .get(&video_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::other(format!("Failed to fetch video page: {e}"))
            })?;

        let html = response
            .text()
            .await
            .map_err(|e| dashflow::core::Error::other(format!("Failed to read video page: {e}")))?;

        // Extract video title from the page
        let title = Self::extract_title(&html).unwrap_or_else(|| format!("Video {video_id}"));

        // Extract the captions data from the initial player response
        let captions_data = Self::extract_captions_data(&html)?;

        // Find the appropriate transcript URL
        let transcript_url = self.find_transcript_url(&captions_data)?;

        // Fetch the transcript XML
        let transcript_response = client
            .get(&transcript_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::other(format!("Failed to fetch transcript: {e}"))
            })?;

        let transcript_xml = transcript_response
            .text()
            .await
            .map_err(|e| dashflow::core::Error::other(format!("Failed to read transcript: {e}")))?;

        // Parse the transcript XML
        let chunks = Self::parse_transcript_xml(&transcript_xml)?;

        if chunks.is_empty() {
            return Err(dashflow::core::Error::other(
                "No transcript content found".to_string(),
            ));
        }

        // Convert to documents
        let documents = self.chunks_to_documents(chunks, video_id, &title);

        Ok(documents)
    }

    /// Extract video title from HTML
    fn extract_title(html: &str) -> Option<String> {
        let re = Regex::new(r#"<title>([^<]+)</title>"#).ok()?;
        let captures = re.captures(html)?;
        let title = captures.get(1)?.as_str();
        // Remove " - YouTube" suffix
        let title = title.trim_end_matches(" - YouTube").trim();
        Some(html_escape::decode_html_entities(title).to_string())
    }

    /// Extract captions data from the initial player response JSON
    fn extract_captions_data(html: &str) -> Result<serde_json::Value> {
        // Look for the ytInitialPlayerResponse variable
        let re = Regex::new(r"ytInitialPlayerResponse\s*=\s*(\{.+?\});")
            .map_err(|e| dashflow::core::Error::other(format!("Regex error: {e}")))?;

        let captures = re.captures(html).ok_or_else(|| {
            dashflow::core::Error::other(
                "Could not find player response in video page. Video may be unavailable or age-restricted.".to_string(),
            )
        })?;

        let json_str = captures.get(1).ok_or_else(|| {
            dashflow::core::Error::other("Could not extract player response JSON".to_string())
        })?;

        let player_response: serde_json::Value =
            serde_json::from_str(json_str.as_str()).map_err(|e| {
                dashflow::core::Error::other(format!("Failed to parse player response: {e}"))
            })?;

        // Navigate to captions data
        let captions = player_response
            .get("captions")
            .and_then(|c| c.get("playerCaptionsTracklistRenderer"))
            .cloned()
            .ok_or_else(|| {
                dashflow::core::Error::other(
                    "No captions available for this video. The video may not have transcripts enabled.".to_string(),
                )
            })?;

        Ok(captions)
    }

    /// Find the appropriate transcript URL based on language preferences
    fn find_transcript_url(&self, captions_data: &serde_json::Value) -> Result<String> {
        let caption_tracks = captions_data
            .get("captionTracks")
            .and_then(|t| t.as_array())
            .ok_or_else(|| dashflow::core::Error::other("No caption tracks found".to_string()))?;

        if caption_tracks.is_empty() {
            return Err(dashflow::core::Error::other(
                "No caption tracks available".to_string(),
            ));
        }

        // Try to find the preferred language
        if let Some(preferred_lang) = &self.language {
            for track in caption_tracks {
                let lang_code = track
                    .get("languageCode")
                    .and_then(|l| l.as_str())
                    .unwrap_or("");

                if lang_code == preferred_lang {
                    let is_auto = track.get("kind").and_then(|k| k.as_str()) == Some("asr");

                    if !is_auto || self.allow_auto_generated {
                        if let Some(url) = track.get("baseUrl").and_then(|u| u.as_str()) {
                            return Ok(url.to_string());
                        }
                    }
                }
            }
        }

        // Fall back to first available track
        for track in caption_tracks {
            let is_auto = track.get("kind").and_then(|k| k.as_str()) == Some("asr");

            if !is_auto || self.allow_auto_generated {
                if let Some(url) = track.get("baseUrl").and_then(|u| u.as_str()) {
                    return Ok(url.to_string());
                }
            }
        }

        // Last resort: use first track regardless of type
        caption_tracks
            .first()
            .and_then(|t| t.get("baseUrl"))
            .and_then(|u| u.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| {
                dashflow::core::Error::other("No valid caption track URL found".to_string())
            })
    }

    /// Parse YouTube transcript XML format
    fn parse_transcript_xml(xml: &str) -> Result<Vec<TranscriptChunk>> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);

        let mut chunks = Vec::new();
        let mut current_start: Option<f64> = None;
        let mut current_duration: Option<f64> = None;
        let mut text_buffer = String::new();

        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                    if e.name().as_ref() == b"text" {
                        // Extract start and dur attributes
                        for attr in e.attributes().flatten() {
                            let key = String::from_utf8_lossy(attr.key.as_ref());
                            let value = String::from_utf8_lossy(&attr.value);

                            match key.as_ref() {
                                "start" => {
                                    current_start = value.parse().ok();
                                }
                                "dur" => {
                                    current_duration = value.parse().ok();
                                }
                                _ => {}
                            }
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    text_buffer.push_str(&text);
                }
                Ok(Event::End(e)) => {
                    if e.name().as_ref() == b"text" && !text_buffer.is_empty() {
                        let chunk = TranscriptChunk {
                            text: html_escape::decode_html_entities(&text_buffer).to_string(),
                            start: current_start.unwrap_or(0.0),
                            duration: current_duration.unwrap_or(0.0),
                        };
                        chunks.push(chunk);

                        text_buffer.clear();
                        current_start = None;
                        current_duration = None;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    return Err(dashflow::core::Error::other(format!(
                        "XML parsing error: {e}"
                    )));
                }
                _ => {}
            }
        }

        Ok(chunks)
    }

    /// Convert transcript chunks to documents
    fn chunks_to_documents(
        &self,
        chunks: Vec<TranscriptChunk>,
        video_id: &str,
        title: &str,
    ) -> Vec<Document> {
        if self.chunk_by_sentences {
            // Combine all text and split by sentences
            let full_text: String = chunks
                .iter()
                .map(|c| c.text.clone())
                .collect::<Vec<_>>()
                .join(" ");
            let sentences = self.split_into_sentences(&full_text);

            sentences
                .into_iter()
                .enumerate()
                .map(|(i, sentence)| {
                    let mut metadata = HashMap::new();
                    metadata.insert(
                        "source".to_string(),
                        serde_json::Value::String(format!(
                            "https://www.youtube.com/watch?v={video_id}"
                        )),
                    );
                    metadata.insert(
                        "video_id".to_string(),
                        serde_json::Value::String(video_id.to_string()),
                    );
                    metadata.insert(
                        "title".to_string(),
                        serde_json::Value::String(title.to_string()),
                    );
                    metadata.insert(
                        "chunk_index".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(i)),
                    );

                    Document {
                        page_content: sentence,
                        metadata,
                        id: Some(format!("{video_id}_{i}")),
                    }
                })
                .collect()
        } else {
            // Return one document per chunk with timing info
            chunks
                .into_iter()
                .enumerate()
                .map(|(i, chunk)| {
                    let mut metadata = HashMap::new();
                    metadata.insert(
                        "source".to_string(),
                        serde_json::Value::String(format!(
                            "https://www.youtube.com/watch?v={video_id}"
                        )),
                    );
                    metadata.insert(
                        "video_id".to_string(),
                        serde_json::Value::String(video_id.to_string()),
                    );
                    metadata.insert(
                        "title".to_string(),
                        serde_json::Value::String(title.to_string()),
                    );
                    metadata.insert(
                        "start_time".to_string(),
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(chunk.start)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ),
                    );
                    metadata.insert(
                        "duration".to_string(),
                        serde_json::Value::Number(
                            serde_json::Number::from_f64(chunk.duration)
                                .unwrap_or_else(|| serde_json::Number::from(0)),
                        ),
                    );
                    metadata.insert(
                        "chunk_index".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(i)),
                    );

                    let content = if self.include_timestamps {
                        format!("[{:.2}s] {}", chunk.start, chunk.text)
                    } else {
                        chunk.text
                    };

                    Document {
                        page_content: content,
                        metadata,
                        id: Some(format!("{video_id}_{i}")),
                    }
                })
                .collect()
        }
    }

    /// Split text into sentences
    fn split_into_sentences(&self, text: &str) -> Vec<String> {
        // Simple sentence splitting by common terminators
        let sentences: Vec<String> = get_sentence_split_regex()
            .split(text)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if sentences.is_empty() {
            vec![text.to_string()]
        } else {
            sentences
        }
    }

    /// Get available languages for a video
    pub async fn get_available_languages(&self, url_or_id: &str) -> Result<Vec<Language>> {
        let video_id = extract_video_id(url_or_id).ok_or_else(|| {
            dashflow::core::Error::other(format!("Could not extract video ID from: {url_or_id}"))
        })?;

        let client = create_http_client();
        let video_url = format!("https://www.youtube.com/watch?v={video_id}");

        let response = client
            .get(&video_url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
            )
            .send()
            .await
            .map_err(|e| {
                dashflow::core::Error::other(format!("Failed to fetch video page: {e}"))
            })?;

        let html = response
            .text()
            .await
            .map_err(|e| dashflow::core::Error::other(format!("Failed to read video page: {e}")))?;

        let captions_data = Self::extract_captions_data(&html)?;

        let caption_tracks = captions_data
            .get("captionTracks")
            .and_then(|t| t.as_array())
            .ok_or_else(|| dashflow::core::Error::other("No caption tracks found".to_string()))?;

        let languages: Vec<Language> = caption_tracks
            .iter()
            .filter_map(|track| {
                let code = track.get("languageCode")?.as_str()?.to_string();
                let name = track
                    .get("name")
                    .and_then(|n| n.get("simpleText"))
                    .and_then(|s| s.as_str())
                    .unwrap_or(&code)
                    .to_string();
                let is_auto_generated = track.get("kind").and_then(|k| k.as_str()) == Some("asr");

                Some(Language {
                    code,
                    name,
                    is_auto_generated,
                })
            })
            .collect();

        Ok(languages)
    }
}

impl Default for YouTubeTranscriptLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DocumentLoader for YouTubeTranscriptLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        Err(dashflow::core::Error::other(
            "YouTubeTranscriptLoader requires a video URL. Use load_from_url() instead."
                .to_string(),
        ))
    }
}

/// Builder for `YouTubeTranscriptLoader`
#[derive(Default)]
pub struct YouTubeTranscriptLoaderBuilder {
    language: Option<String>,
    chunk_by_sentences: Option<bool>,
    include_timestamps: Option<bool>,
    allow_auto_generated: Option<bool>,
}

impl YouTubeTranscriptLoaderBuilder {
    /// Set the preferred language code (e.g., "en", "es", "fr")
    #[must_use]
    pub fn language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    /// Set whether to chunk the transcript by sentences
    ///
    /// Default: false
    #[must_use]
    pub fn chunk_by_sentences(mut self, chunk: bool) -> Self {
        self.chunk_by_sentences = Some(chunk);
        self
    }

    /// Set whether to include timestamps in document content
    ///
    /// Default: false
    #[must_use]
    pub fn include_timestamps(mut self, include: bool) -> Self {
        self.include_timestamps = Some(include);
        self
    }

    /// Set whether to allow auto-generated transcripts
    ///
    /// Default: true
    #[must_use]
    pub fn allow_auto_generated(mut self, allow: bool) -> Self {
        self.allow_auto_generated = Some(allow);
        self
    }

    /// Build the `YouTubeTranscriptLoader`
    #[must_use]
    pub fn build(self) -> YouTubeTranscriptLoader {
        YouTubeTranscriptLoader {
            language: self.language,
            chunk_by_sentences: self.chunk_by_sentences.unwrap_or(false),
            include_timestamps: self.include_timestamps.unwrap_or(false),
            allow_auto_generated: self.allow_auto_generated.unwrap_or(true),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // YouTubeTranscriptLoader Creation Tests
    // ==========================================================================

    #[test]
    fn test_loader_creation() {
        let loader = YouTubeTranscriptLoader::new();
        assert!(loader.language.is_none());
        assert!(!loader.chunk_by_sentences);
        assert!(!loader.include_timestamps);
        assert!(loader.allow_auto_generated);
    }

    #[test]
    fn test_loader_default() {
        let loader = YouTubeTranscriptLoader::default();
        assert!(loader.language.is_none());
        assert!(!loader.chunk_by_sentences);
        assert!(!loader.include_timestamps);
        assert!(loader.allow_auto_generated);
    }

    #[test]
    fn test_loader_default_equals_new() {
        let new = YouTubeTranscriptLoader::new();
        let default = YouTubeTranscriptLoader::default();

        assert_eq!(new.language, default.language);
        assert_eq!(new.chunk_by_sentences, default.chunk_by_sentences);
        assert_eq!(new.include_timestamps, default.include_timestamps);
        assert_eq!(new.allow_auto_generated, default.allow_auto_generated);
    }

    // ==========================================================================
    // YouTubeTranscriptLoaderBuilder Tests
    // ==========================================================================

    #[test]
    fn test_loader_builder() {
        let loader = YouTubeTranscriptLoader::builder()
            .language("en".to_string())
            .chunk_by_sentences(true)
            .include_timestamps(true)
            .allow_auto_generated(false)
            .build();

        assert_eq!(loader.language, Some("en".to_string()));
        assert!(loader.chunk_by_sentences);
        assert!(loader.include_timestamps);
        assert!(!loader.allow_auto_generated);
    }

    #[test]
    fn test_loader_builder_default() {
        let loader = YouTubeTranscriptLoaderBuilder::default().build();

        assert!(loader.language.is_none());
        assert!(!loader.chunk_by_sentences);
        assert!(!loader.include_timestamps);
        assert!(loader.allow_auto_generated);
    }

    #[test]
    fn test_loader_builder_language_only() {
        let loader = YouTubeTranscriptLoader::builder()
            .language("es".to_string())
            .build();

        assert_eq!(loader.language, Some("es".to_string()));
        assert!(!loader.chunk_by_sentences);
    }

    #[test]
    fn test_loader_builder_chunk_by_sentences_only() {
        let loader = YouTubeTranscriptLoader::builder()
            .chunk_by_sentences(true)
            .build();

        assert!(loader.language.is_none());
        assert!(loader.chunk_by_sentences);
    }

    #[test]
    fn test_loader_builder_timestamps_only() {
        let loader = YouTubeTranscriptLoader::builder()
            .include_timestamps(true)
            .build();

        assert!(loader.include_timestamps);
        assert!(!loader.chunk_by_sentences);
    }

    #[test]
    fn test_loader_builder_disallow_auto_generated() {
        let loader = YouTubeTranscriptLoader::builder()
            .allow_auto_generated(false)
            .build();

        assert!(!loader.allow_auto_generated);
    }

    #[test]
    fn test_loader_builder_various_languages() {
        let languages = ["en", "es", "fr", "de", "ja", "ko", "zh", "pt", "ru", "ar"];

        for lang in languages {
            let loader = YouTubeTranscriptLoader::builder()
                .language(lang.to_string())
                .build();
            assert_eq!(loader.language, Some(lang.to_string()));
        }
    }

    // ==========================================================================
    // Sentence Splitting Tests
    // ==========================================================================

    #[test]
    fn test_sentence_splitting() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "Hello world. This is a test! How are you? I am fine.";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 4);
        assert_eq!(sentences[0], "Hello world");
        assert_eq!(sentences[1], "This is a test");
    }

    #[test]
    fn test_sentence_splitting_no_terminators() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "Hello world this is a test";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "Hello world this is a test");
    }

    #[test]
    fn test_sentence_splitting_empty_text() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 1);
        assert_eq!(sentences[0], "");
    }

    #[test]
    fn test_sentence_splitting_only_whitespace() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "   ";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 1);
    }

    #[test]
    fn test_sentence_splitting_multiple_periods() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "First sentence... Second sentence.";
        let sentences = loader.split_into_sentences(text);

        // "..." followed by space should split
        assert!(sentences.len() >= 1);
    }

    #[test]
    fn test_sentence_splitting_exclamation() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "Wow! Amazing!";
        let sentences = loader.split_into_sentences(text);

        assert!(sentences.len() >= 1);
    }

    #[test]
    fn test_sentence_splitting_question() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "How are you? I am fine.";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 2);
    }

    #[test]
    fn test_sentence_splitting_mixed_punctuation() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "Hello. How are you? Great! Thanks.";
        let sentences = loader.split_into_sentences(text);

        assert_eq!(sentences.len(), 4);
    }

    #[test]
    fn test_sentence_splitting_preserves_trimmed_content() {
        let loader = YouTubeTranscriptLoader::new();

        let text = "  First sentence.   Second sentence.  ";
        let sentences = loader.split_into_sentences(text);

        // Should trim whitespace from sentences
        for sentence in &sentences {
            assert!(!sentence.starts_with(' '));
            assert!(!sentence.ends_with(' '));
        }
    }

    // ==========================================================================
    // XML Parsing Tests
    // ==========================================================================

    #[test]
    fn test_parse_transcript_xml() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="0.0" dur="1.5">Hello world</text>
    <text start="1.5" dur="2.0">This is a test</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "Hello world");
        assert!((chunks[0].start - 0.0).abs() < f64::EPSILON);
        assert!((chunks[0].duration - 1.5).abs() < f64::EPSILON);
        assert_eq!(chunks[1].text, "This is a test");
        assert!((chunks[1].start - 1.5).abs() < f64::EPSILON);
        assert!((chunks[1].duration - 2.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_transcript_xml_with_entities() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="0.0" dur="1.5">Hello &amp; goodbye</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Hello & goodbye");
    }

    #[test]
    fn test_parse_transcript_xml_multiple_entities() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="0.0" dur="1.5">&lt;script&gt; &amp; &quot;test&quot;</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.contains("<script>"));
        assert!(chunks[0].text.contains("&"));
    }

    #[test]
    fn test_parse_transcript_xml_empty_transcript() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_parse_transcript_xml_single_chunk() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="5.5" dur="3.2">Single chunk only</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "Single chunk only");
        assert!((chunks[0].start - 5.5).abs() < f64::EPSILON);
        assert!((chunks[0].duration - 3.2).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_transcript_xml_fractional_times() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="123.456" dur="7.89">Precise timing</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!((chunks[0].start - 123.456).abs() < 0.001);
        assert!((chunks[0].duration - 7.89).abs() < 0.001);
    }

    #[test]
    fn test_parse_transcript_xml_zero_duration() {
        let xml = r#"<?xml version="1.0" encoding="utf-8"?>
<transcript>
    <text start="10.0" dur="0.0">Zero duration</text>
</transcript>"#;

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(xml).unwrap();

        assert_eq!(chunks.len(), 1);
        assert!((chunks[0].duration - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_transcript_xml_many_chunks() {
        let mut xml = String::from(r#"<?xml version="1.0" encoding="utf-8"?><transcript>"#);
        for i in 0..100 {
            xml.push_str(&format!(
                r#"<text start="{}.0" dur="1.0">Chunk {}</text>"#,
                i, i
            ));
        }
        xml.push_str("</transcript>");

        let chunks = YouTubeTranscriptLoader::parse_transcript_xml(&xml).unwrap();

        assert_eq!(chunks.len(), 100);
        assert_eq!(chunks[0].text, "Chunk 0");
        assert_eq!(chunks[99].text, "Chunk 99");
    }

    // ==========================================================================
    // Title Extraction Tests
    // ==========================================================================

    #[test]
    fn test_extract_title() {
        let html = "<html><head><title>Test Video - YouTube</title></head></html>";
        let title = YouTubeTranscriptLoader::extract_title(html);
        assert_eq!(title, Some("Test Video".to_string()));
    }

    #[test]
    fn test_extract_title_with_entities() {
        let html = "<html><head><title>Test &amp; Video - YouTube</title></head></html>";
        let title = YouTubeTranscriptLoader::extract_title(html);
        assert_eq!(title, Some("Test & Video".to_string()));
    }

    #[test]
    fn test_extract_title_no_youtube_suffix() {
        let html = "<html><head><title>Just A Title</title></head></html>";
        let title = YouTubeTranscriptLoader::extract_title(html);
        assert_eq!(title, Some("Just A Title".to_string()));
    }

    #[test]
    fn test_extract_title_missing() {
        let html = "<html><head></head></html>";
        let title = YouTubeTranscriptLoader::extract_title(html);
        assert!(title.is_none());
    }

    #[test]
    fn test_extract_title_empty() {
        let html = "<html><head><title></title></head></html>";
        let title = YouTubeTranscriptLoader::extract_title(html);
        // Empty title might return None or empty string depending on implementation
        assert!(title.is_none() || title == Some("".to_string()));
    }

    #[test]
    fn test_extract_title_long_title() {
        let long_title = "A".repeat(500);
        let html = format!(
            "<html><head><title>{} - YouTube</title></head></html>",
            long_title
        );
        let title = YouTubeTranscriptLoader::extract_title(&html);
        assert!(title.is_some());
        assert!(title.unwrap().len() >= 500);
    }

    #[test]
    fn test_extract_title_special_characters() {
        let html = r#"<html><head><title>Test "Video" 'Title' - YouTube</title></head></html>"#;
        let title = YouTubeTranscriptLoader::extract_title(html);
        assert!(title.is_some());
        assert!(title.unwrap().contains("Video"));
    }

    // ==========================================================================
    // Chunks to Documents Tests
    // ==========================================================================

    #[test]
    fn test_chunks_to_documents_with_timestamps() {
        let loader = YouTubeTranscriptLoader::builder()
            .include_timestamps(true)
            .build();

        let chunks = vec![
            TranscriptChunk {
                text: "Hello world".to_string(),
                start: 0.0,
                duration: 1.5,
            },
            TranscriptChunk {
                text: "Test content".to_string(),
                start: 1.5,
                duration: 2.0,
            },
        ];

        let docs = loader.chunks_to_documents(chunks, "test123", "Test Video");

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "[0.00s] Hello world");
        assert_eq!(docs[1].page_content, "[1.50s] Test content");
    }

    #[test]
    fn test_chunks_to_documents_without_timestamps() {
        let loader = YouTubeTranscriptLoader::new();

        let chunks = vec![TranscriptChunk {
            text: "Hello world".to_string(),
            start: 0.0,
            duration: 1.5,
        }];

        let docs = loader.chunks_to_documents(chunks, "test123", "Test Video");

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello world");
        assert_eq!(
            docs[0].metadata.get("video_id"),
            Some(&serde_json::Value::String("test123".to_string()))
        );
    }

    #[test]
    fn test_chunks_to_documents_metadata_complete() {
        let loader = YouTubeTranscriptLoader::new();

        let chunks = vec![TranscriptChunk {
            text: "Content".to_string(),
            start: 10.5,
            duration: 2.5,
        }];

        let docs = loader.chunks_to_documents(chunks, "vid123", "My Video Title");

        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("video_id"));
        assert!(docs[0].metadata.contains_key("title"));
        assert!(docs[0].metadata.contains_key("start_time"));
        assert!(docs[0].metadata.contains_key("duration"));
        assert!(docs[0].metadata.contains_key("chunk_index"));
    }

    #[test]
    fn test_chunks_to_documents_document_id() {
        let loader = YouTubeTranscriptLoader::new();

        let chunks = vec![
            TranscriptChunk {
                text: "First".to_string(),
                start: 0.0,
                duration: 1.0,
            },
            TranscriptChunk {
                text: "Second".to_string(),
                start: 1.0,
                duration: 1.0,
            },
        ];

        let docs = loader.chunks_to_documents(chunks, "vid456", "Title");

        assert_eq!(docs[0].id, Some("vid456_0".to_string()));
        assert_eq!(docs[1].id, Some("vid456_1".to_string()));
    }

    #[test]
    fn test_chunks_to_documents_source_url() {
        let loader = YouTubeTranscriptLoader::new();

        let chunks = vec![TranscriptChunk {
            text: "Content".to_string(),
            start: 0.0,
            duration: 1.0,
        }];

        let docs = loader.chunks_to_documents(chunks, "abc123xyz", "Title");

        let source = docs[0].metadata.get("source").unwrap();
        assert!(source
            .as_str()
            .unwrap()
            .contains("youtube.com/watch?v=abc123xyz"));
    }

    #[test]
    fn test_chunks_to_documents_chunk_by_sentences() {
        let loader = YouTubeTranscriptLoader::builder()
            .chunk_by_sentences(true)
            .build();

        let chunks = vec![
            TranscriptChunk {
                text: "Hello world.".to_string(),
                start: 0.0,
                duration: 1.0,
            },
            TranscriptChunk {
                text: " This is another sentence.".to_string(),
                start: 1.0,
                duration: 1.0,
            },
        ];

        let docs = loader.chunks_to_documents(chunks, "vid", "Title");

        // When chunking by sentences, the chunks are combined and re-split
        assert!(!docs.is_empty());
    }

    #[test]
    fn test_chunks_to_documents_empty_chunks() {
        let loader = YouTubeTranscriptLoader::new();

        let chunks: Vec<TranscriptChunk> = vec![];
        let docs = loader.chunks_to_documents(chunks, "vid", "Title");

        assert!(docs.is_empty());
    }

    #[test]
    fn test_chunks_to_documents_timestamp_format() {
        let loader = YouTubeTranscriptLoader::builder()
            .include_timestamps(true)
            .build();

        let chunks = vec![
            TranscriptChunk {
                text: "A".to_string(),
                start: 0.0,
                duration: 1.0,
            },
            TranscriptChunk {
                text: "B".to_string(),
                start: 65.5,
                duration: 1.0,
            },
            TranscriptChunk {
                text: "C".to_string(),
                start: 3600.0,
                duration: 1.0,
            },
        ];

        let docs = loader.chunks_to_documents(chunks, "vid", "Title");

        assert!(docs[0].page_content.starts_with("[0.00s]"));
        assert!(docs[1].page_content.starts_with("[65.50s]"));
        assert!(docs[2].page_content.starts_with("[3600.00s]"));
    }

    // ==========================================================================
    // TranscriptChunk Tests
    // ==========================================================================

    #[test]
    fn test_transcript_chunk_serialization() {
        let chunk = TranscriptChunk {
            text: "Test text".to_string(),
            start: 10.5,
            duration: 3.2,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        assert!(json.contains("Test text"));
        assert!(json.contains("10.5"));
        assert!(json.contains("3.2"));
    }

    #[test]
    fn test_transcript_chunk_deserialization() {
        let json = r#"{"text": "Deserialized", "start": 5.0, "duration": 2.5}"#;
        let chunk: TranscriptChunk = serde_json::from_str(json).unwrap();

        assert_eq!(chunk.text, "Deserialized");
        assert!((chunk.start - 5.0).abs() < f64::EPSILON);
        assert!((chunk.duration - 2.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_transcript_chunk_roundtrip() {
        let original = TranscriptChunk {
            text: "Roundtrip test".to_string(),
            start: 123.456,
            duration: 7.89,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: TranscriptChunk = serde_json::from_str(&json).unwrap();

        assert_eq!(original.text, deserialized.text);
        assert!((original.start - deserialized.start).abs() < 0.001);
        assert!((original.duration - deserialized.duration).abs() < 0.001);
    }

    #[test]
    fn test_transcript_chunk_debug() {
        let chunk = TranscriptChunk {
            text: "Debug".to_string(),
            start: 1.0,
            duration: 2.0,
        };

        let debug = format!("{:?}", chunk);
        assert!(debug.contains("TranscriptChunk"));
        assert!(debug.contains("Debug"));
    }

    #[test]
    fn test_transcript_chunk_clone() {
        let original = TranscriptChunk {
            text: "Clone me".to_string(),
            start: 5.0,
            duration: 3.0,
        };

        let cloned = original.clone();
        assert_eq!(original.text, cloned.text);
        assert!((original.start - cloned.start).abs() < f64::EPSILON);
    }

    // ==========================================================================
    // Language Struct Tests
    // ==========================================================================

    #[test]
    fn test_language_serialization() {
        let lang = Language {
            code: "en".to_string(),
            name: "English".to_string(),
            is_auto_generated: false,
        };

        let json = serde_json::to_string(&lang).unwrap();
        assert!(json.contains("en"));
        assert!(json.contains("English"));
        assert!(json.contains("false"));
    }

    #[test]
    fn test_language_deserialization() {
        let json = r#"{"code": "es", "name": "Spanish", "is_auto_generated": true}"#;
        let lang: Language = serde_json::from_str(json).unwrap();

        assert_eq!(lang.code, "es");
        assert_eq!(lang.name, "Spanish");
        assert!(lang.is_auto_generated);
    }

    #[test]
    fn test_language_roundtrip() {
        let original = Language {
            code: "fr".to_string(),
            name: "French".to_string(),
            is_auto_generated: false,
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: Language = serde_json::from_str(&json).unwrap();

        assert_eq!(original.code, deserialized.code);
        assert_eq!(original.name, deserialized.name);
        assert_eq!(original.is_auto_generated, deserialized.is_auto_generated);
    }

    #[test]
    fn test_language_auto_generated_flag() {
        let manual = Language {
            code: "en".to_string(),
            name: "English".to_string(),
            is_auto_generated: false,
        };

        let auto = Language {
            code: "en".to_string(),
            name: "English (auto-generated)".to_string(),
            is_auto_generated: true,
        };

        assert!(!manual.is_auto_generated);
        assert!(auto.is_auto_generated);
    }

    #[test]
    fn test_language_debug() {
        let lang = Language {
            code: "de".to_string(),
            name: "German".to_string(),
            is_auto_generated: false,
        };

        let debug = format!("{:?}", lang);
        assert!(debug.contains("Language"));
        assert!(debug.contains("de"));
    }

    #[test]
    fn test_language_clone() {
        let original = Language {
            code: "ja".to_string(),
            name: "Japanese".to_string(),
            is_auto_generated: true,
        };

        let cloned = original.clone();
        assert_eq!(original.code, cloned.code);
        assert_eq!(original.name, cloned.name);
        assert_eq!(original.is_auto_generated, cloned.is_auto_generated);
    }

    // ==========================================================================
    // DocumentLoader Trait Tests
    // ==========================================================================

    #[tokio::test]
    async fn test_document_loader_load_returns_error() {
        let loader = YouTubeTranscriptLoader::new();
        let result = loader.load().await;

        // The generic load() should return an error since we need a URL
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_document_loader_error_message() {
        let loader = YouTubeTranscriptLoader::new();
        let result = loader.load().await;

        assert!(result.is_err());
        // Check error message via match
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("load_from_url") || err_msg.contains("URL"));
        }
    }

    // ==========================================================================
    // Integration Tests (require network access)
    // ==========================================================================

    #[tokio::test]
    #[ignore = "requires network access and may fail due to YouTube bot detection"]
    async fn test_load_transcript_integration() {
        let loader = YouTubeTranscriptLoader::builder()
            .language("en".to_string())
            .build();

        // Use a video known to have transcripts (TED talk)
        let result = loader
            .load_from_url("https://www.youtube.com/watch?v=8jPQjjsBbIc")
            .await;

        let docs = result.expect("YouTube transcript load failed");
        assert!(!docs.is_empty());
        assert!(docs[0].metadata.contains_key("video_id"));
        assert!(docs[0].metadata.contains_key("title"));
    }

    #[tokio::test]
    #[ignore = "requires network access and may fail due to YouTube bot detection"]
    async fn test_load_transcript_with_timestamps() {
        let loader = YouTubeTranscriptLoader::builder()
            .include_timestamps(true)
            .build();

        let result = loader
            .load_from_url("https://www.youtube.com/watch?v=8jPQjjsBbIc")
            .await;

        let docs = result.expect("YouTube transcript load failed");
        assert!(!docs.is_empty());
        // Check timestamp format in content
        assert!(docs[0].page_content.starts_with('['));
    }

    #[tokio::test]
    #[ignore = "requires network access and may fail due to YouTube bot detection"]
    async fn test_get_available_languages() {
        let loader = YouTubeTranscriptLoader::new();

        let result = loader
            .get_available_languages("https://www.youtube.com/watch?v=8jPQjjsBbIc")
            .await;

        let languages = result.expect("Getting languages failed");
        assert!(!languages.is_empty());

        // At least one language should be available
        let codes: Vec<&str> = languages.iter().map(|l| l.code.as_str()).collect();
        assert!(!codes.is_empty());
    }
}
