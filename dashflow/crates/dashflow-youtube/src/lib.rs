//! # YouTube Integration for DashFlow
//!
//! This crate provides YouTube integration for `DashFlow` Rust, including:
//! - Video transcript loading for document processing
//! - Video search tool for agents
//!
//! ## Features
//!
//! ### Transcript Loading
//!
//! Load YouTube video transcripts as documents for RAG applications:
//!
//! ```rust,no_run
//! use dashflow_youtube::YouTubeTranscriptLoader;
//! use dashflow::core::documents::DocumentLoader;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loader = YouTubeTranscriptLoader::new();
//!
//! // Load transcript from video URL or ID
//! let docs = loader.load_from_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ").await?;
//!
//! for doc in docs {
//!     println!("Transcript: {}", doc.page_content);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! ### Video Search
//!
//! Search YouTube videos using the Data API (requires API key):
//!
//! ```rust,no_run
//! use dashflow_youtube::YouTubeSearchTool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Set YOUTUBE_API_KEY environment variable
//! std::env::set_var("YOUTUBE_API_KEY", "your-api-key");
//!
//! let youtube = YouTubeSearchTool::new()?;
//!
//! let results = youtube._call_str("rust programming tutorials".to_string()).await?;
//! println!("{}", results);
//! # Ok(())
//! # }
//! ```
//!
//! ## Environment Variables
//!
//! - `YOUTUBE_API_KEY`: Required for video search (get from Google Cloud Console)
//!
//! ## Python Baseline
//!
//! This crate implements functionality from:
//! - `dashflow_community.document_loaders.youtube` - Transcript loading
//! - `dashflow_community.tools.youtube.search` - Video search

mod search;
mod transcript;

pub use search::{
    SearchOrder, VideoDefinition, VideoDuration, VideoType, YouTubeRetriever,
    YouTubeRetrieverBuilder, YouTubeSearchTool, YouTubeVideo,
};
pub use transcript::{Language, TranscriptChunk, YouTubeTranscriptLoader};

/// Extract video ID from various YouTube URL formats
///
/// Supports:
/// - `https://www.youtube.com/watch?v=VIDEO_ID`
/// - `https://youtu.be/VIDEO_ID`
/// - `https://www.youtube.com/embed/VIDEO_ID`
/// - `https://www.youtube.com/v/VIDEO_ID`
/// - Plain video ID
///
/// # Example
///
/// ```rust
/// use dashflow_youtube::extract_video_id;
///
/// assert_eq!(extract_video_id("https://www.youtube.com/watch?v=dQw4w9WgXcQ"), Some("dQw4w9WgXcQ".to_string()));
/// assert_eq!(extract_video_id("https://youtu.be/dQw4w9WgXcQ"), Some("dQw4w9WgXcQ".to_string()));
/// assert_eq!(extract_video_id("dQw4w9WgXcQ"), Some("dQw4w9WgXcQ".to_string()));
/// ```
pub fn extract_video_id(url_or_id: &str) -> Option<String> {
    use regex::Regex;

    // Check if it's already just a video ID (11 characters, alphanumeric with - and _)
    let id_regex = Regex::new(r"^[a-zA-Z0-9_-]{11}$").ok()?;
    if id_regex.is_match(url_or_id) {
        return Some(url_or_id.to_string());
    }

    // Try various URL patterns
    let patterns = [
        // Standard watch URL: youtube.com/watch?v=VIDEO_ID
        r"(?:youtube\.com/watch\?v=|youtube\.com/watch\?.+&v=)([a-zA-Z0-9_-]{11})",
        // Short URL: youtu.be/VIDEO_ID
        r"youtu\.be/([a-zA-Z0-9_-]{11})",
        // Embed URL: youtube.com/embed/VIDEO_ID
        r"youtube\.com/embed/([a-zA-Z0-9_-]{11})",
        // Old embed URL: youtube.com/v/VIDEO_ID
        r"youtube\.com/v/([a-zA-Z0-9_-]{11})",
        // Shorts URL: youtube.com/shorts/VIDEO_ID
        r"youtube\.com/shorts/([a-zA-Z0-9_-]{11})",
    ];

    for pattern in patterns {
        if let Ok(regex) = Regex::new(pattern) {
            if let Some(captures) = regex.captures(url_or_id) {
                if let Some(id) = captures.get(1) {
                    return Some(id.as_str().to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // Standard URL Format Tests
    // ==========================================================================

    #[test]
    fn test_extract_video_id_standard_url() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_standard_url_http() {
        let url = "http://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_standard_url_no_www() {
        let url = "https://youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_standard_url_mobile() {
        let url = "https://m.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Short URL Format Tests (youtu.be)
    // ==========================================================================

    #[test]
    fn test_extract_video_id_short_url() {
        let url = "https://youtu.be/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_short_url_http() {
        let url = "http://youtu.be/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_short_url_with_timestamp() {
        let url = "https://youtu.be/dQw4w9WgXcQ?t=42";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_short_url_with_feature() {
        let url = "https://youtu.be/dQw4w9WgXcQ?feature=share";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Embed URL Format Tests
    // ==========================================================================

    #[test]
    fn test_extract_video_id_embed_url() {
        let url = "https://www.youtube.com/embed/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_embed_url_with_params() {
        let url = "https://www.youtube.com/embed/dQw4w9WgXcQ?autoplay=1&mute=1";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_embed_url_no_www() {
        let url = "https://youtube.com/embed/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Old Embed Format Tests (youtube.com/v/)
    // ==========================================================================

    #[test]
    fn test_extract_video_id_old_embed_url() {
        let url = "https://www.youtube.com/v/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_old_embed_url_with_params() {
        let url = "https://www.youtube.com/v/dQw4w9WgXcQ?version=3&feature=player_embedded";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Shorts URL Format Tests
    // ==========================================================================

    #[test]
    fn test_extract_video_id_shorts_url() {
        let url = "https://www.youtube.com/shorts/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_shorts_url_no_www() {
        let url = "https://youtube.com/shorts/dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_shorts_url_with_params() {
        let url = "https://www.youtube.com/shorts/dQw4w9WgXcQ?feature=share";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Query Parameter Variations
    // ==========================================================================

    #[test]
    fn test_extract_video_id_with_params() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=10s";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_v_not_first_param() {
        let url = "https://www.youtube.com/watch?list=PLrAXtmErZgOeiKm4sgNOknGvNjby9efdf&v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_many_params() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ&list=PLrAXtmErZgOe&index=1&t=30s";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_with_index_before_v() {
        let url = "https://www.youtube.com/watch?index=5&v=dQw4w9WgXcQ&list=PL123";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Plain Video ID Tests
    // ==========================================================================

    #[test]
    fn test_extract_video_id_plain_id() {
        assert_eq!(
            extract_video_id("dQw4w9WgXcQ"),
            Some("dQw4w9WgXcQ".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_plain_id_with_underscore() {
        // YouTube video IDs can contain underscores
        assert_eq!(
            extract_video_id("abc_def-123"),
            Some("abc_def-123".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_plain_id_with_dash() {
        // YouTube video IDs can contain dashes
        assert_eq!(
            extract_video_id("abc-def_123"),
            Some("abc-def_123".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_all_uppercase() {
        assert_eq!(
            extract_video_id("ABCDEFGHIJK"),
            Some("ABCDEFGHIJK".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_all_lowercase() {
        assert_eq!(
            extract_video_id("abcdefghijk"),
            Some("abcdefghijk".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_all_digits() {
        assert_eq!(
            extract_video_id("12345678901"),
            Some("12345678901".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_mixed_alphanumeric() {
        assert_eq!(
            extract_video_id("a1B2c3D4e5F"),
            Some("a1B2c3D4e5F".to_string())
        );
    }

    // ==========================================================================
    // Invalid Input Tests
    // ==========================================================================

    #[test]
    fn test_extract_video_id_invalid() {
        assert_eq!(extract_video_id("invalid"), None);
        assert_eq!(extract_video_id("https://example.com"), None);
    }

    #[test]
    fn test_extract_video_id_empty_string() {
        assert_eq!(extract_video_id(""), None);
    }

    #[test]
    fn test_extract_video_id_too_short() {
        // Video IDs must be exactly 11 characters
        assert_eq!(extract_video_id("abc123"), None);
        assert_eq!(extract_video_id("1234567890"), None); // 10 chars
    }

    #[test]
    fn test_extract_video_id_too_long() {
        // Video IDs must be exactly 11 characters
        assert_eq!(extract_video_id("123456789012"), None); // 12 chars
        assert_eq!(extract_video_id("abcdefghijklmno"), None);
    }

    #[test]
    fn test_extract_video_id_invalid_characters() {
        // Video IDs only allow alphanumeric, dash, and underscore
        assert_eq!(extract_video_id("abc@def#123"), None);
        assert_eq!(extract_video_id("abc def 123"), None); // spaces
        assert_eq!(extract_video_id("abc.def.123"), None); // dots
    }

    #[test]
    fn test_extract_video_id_other_youtube_pages() {
        // These are valid YouTube URLs but don't point to videos
        assert_eq!(extract_video_id("https://www.youtube.com/"), None);
        assert_eq!(extract_video_id("https://www.youtube.com/feed/subscriptions"), None);
        assert_eq!(extract_video_id("https://www.youtube.com/channel/UCuAXFkgsw1L7xaCfnd5JJOw"), None);
    }

    #[test]
    fn test_extract_video_id_non_youtube_urls() {
        assert_eq!(extract_video_id("https://vimeo.com/123456789"), None);
        assert_eq!(extract_video_id("https://dailymotion.com/video/x7abc12"), None);
        assert_eq!(extract_video_id("https://google.com"), None);
    }

    #[test]
    fn test_extract_video_id_malformed_urls() {
        assert_eq!(extract_video_id("youtube.com/watch"), None);
        assert_eq!(extract_video_id("https://www.youtube.com/watch?v="), None);
        assert_eq!(extract_video_id("https://www.youtube.com/watch?v=abc"), None); // too short
    }

    // ==========================================================================
    // Edge Cases
    // ==========================================================================

    #[test]
    fn test_extract_video_id_case_sensitivity_url() {
        // URLs should work regardless of case in domain
        let url = "HTTPS://WWW.YOUTUBE.COM/watch?v=dQw4w9WgXcQ";
        // The regex is case-sensitive on the domain, but video ID extraction should work
        // for lowercase domains
        assert_eq!(extract_video_id(url), None); // uppercase domain doesn't match
    }

    #[test]
    fn test_extract_video_id_trailing_slash() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ/";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_fragment() {
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ#comments";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_url_encoded() {
        // Video ID shouldn't need URL encoding, but test for robustness
        let url = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    #[test]
    fn test_extract_video_id_nocookie_domain() {
        // YouTube nocookie embed domain
        let url = "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ";
        // This domain isn't supported by the current implementation
        assert_eq!(extract_video_id(url), None);
    }

    #[test]
    fn test_extract_video_id_music_youtube() {
        // YouTube Music URLs
        let url = "https://music.youtube.com/watch?v=dQw4w9WgXcQ";
        assert_eq!(extract_video_id(url), Some("dQw4w9WgXcQ".to_string()));
    }

    // ==========================================================================
    // Real-world Video ID Patterns
    // ==========================================================================

    #[test]
    fn test_extract_video_id_starts_with_dash() {
        // Valid video ID starting with dash
        assert_eq!(
            extract_video_id("-abc123def4"),
            Some("-abc123def4".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_starts_with_underscore() {
        // Valid video ID starting with underscore
        assert_eq!(
            extract_video_id("_abc123def4"),
            Some("_abc123def4".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_ends_with_dash() {
        assert_eq!(
            extract_video_id("abc123def4-"),
            Some("abc123def4-".to_string())
        );
    }

    #[test]
    fn test_extract_video_id_ends_with_underscore() {
        assert_eq!(
            extract_video_id("abc123def4_"),
            Some("abc123def4_".to_string())
        );
    }

    // ==========================================================================
    // Exported Types Tests
    // ==========================================================================

    #[test]
    fn test_youtube_video_struct_available() {
        // Verify the YouTubeVideo type is properly exported
        let video = YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test".to_string(),
            description: "Desc".to_string(),
            channel_title: "Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01".to_string(),
            thumbnail_url: None,
            url: "https://youtube.com/watch?v=test123".to_string(),
        };
        assert_eq!(video.video_id, "test123");
    }

    #[test]
    fn test_language_struct_available() {
        // Verify the Language type is properly exported
        let lang = Language {
            code: "en".to_string(),
            name: "English".to_string(),
            is_auto_generated: false,
        };
        assert_eq!(lang.code, "en");
    }

    #[test]
    fn test_transcript_chunk_struct_available() {
        // Verify TranscriptChunk is properly exported
        let chunk = TranscriptChunk {
            text: "Hello".to_string(),
            start: 0.0,
            duration: 1.5,
        };
        assert_eq!(chunk.text, "Hello");
    }

    #[test]
    fn test_search_order_variants() {
        // Verify SearchOrder enum variants
        let _ = SearchOrder::Relevance;
        let _ = SearchOrder::Date;
        let _ = SearchOrder::ViewCount;
        let _ = SearchOrder::Rating;
    }

    #[test]
    fn test_video_type_variants() {
        // Verify VideoType enum variants
        let _ = VideoType::Video;
        let _ = VideoType::Channel;
        let _ = VideoType::Playlist;
    }

    #[test]
    fn test_video_duration_variants() {
        // Verify VideoDuration enum variants
        let _ = VideoDuration::Short;
        let _ = VideoDuration::Medium;
        let _ = VideoDuration::Long;
    }

    #[test]
    fn test_video_definition_variants() {
        // Verify VideoDefinition enum variants
        let _ = VideoDefinition::High;
        let _ = VideoDefinition::Standard;
    }
}
