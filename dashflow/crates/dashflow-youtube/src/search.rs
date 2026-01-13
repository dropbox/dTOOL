//! YouTube video search tool
//!
//! Search for YouTube videos using the YouTube Data API v3.
//!
//! # Python Baseline
//!
//! This implements functionality from:
//! `dashflow_community.tools.youtube.search`

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::config::RunnableConfig;
use dashflow::core::documents::Document;
use dashflow::core::http_client::{json_with_limit, SEARCH_RESPONSE_SIZE_LIMIT};
use dashflow::core::retrievers::Retriever;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::config_loader::env_vars::{env_string, YOUTUBE_API_KEY};
use dashflow::core::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

const YOUTUBE_API_BASE: &str = "https://www.googleapis.com/youtube/v3";

/// YouTube video information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YouTubeVideo {
    /// Video ID
    pub video_id: String,
    /// Video title
    pub title: String,
    /// Video description
    pub description: String,
    /// Channel title
    pub channel_title: String,
    /// Channel ID
    pub channel_id: String,
    /// Publish date (ISO 8601)
    pub published_at: String,
    /// Thumbnail URL
    pub thumbnail_url: Option<String>,
    /// Video URL
    pub url: String,
}

/// YouTube video search tool for DashFlow agents
///
/// Searches YouTube for videos using the YouTube Data API v3.
///
/// # Environment Variables
///
/// - `YOUTUBE_API_KEY`: Required. Get from Google Cloud Console.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_youtube::YouTubeSearchTool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// std::env::set_var("YOUTUBE_API_KEY", "your-api-key");
///
/// let youtube = YouTubeSearchTool::new()?;
///
/// let results = youtube._call_str("rust programming".to_string()).await?;
/// println!("{}", results);
/// # Ok(())
/// # }
/// ```
pub struct YouTubeSearchTool {
    /// YouTube Data API key
    api_key: String,
    /// Maximum number of results to return
    max_results: usize,
    /// Filter by video type (video, channel, playlist)
    video_type: VideoType,
    /// Order results by (relevance, date, viewCount, rating)
    order: SearchOrder,
    /// Filter by video duration
    video_duration: Option<VideoDuration>,
    /// Filter by video definition (high, standard)
    video_definition: Option<VideoDefinition>,
    /// Filter by region code (ISO 3166-1 alpha-2)
    region_code: Option<String>,
    /// HTTP client
    client: reqwest::Client,
}

/// Type of content to search for
#[derive(Debug, Clone, Copy, Default)]
pub enum VideoType {
    /// Only videos
    #[default]
    Video,
    /// Only channels
    Channel,
    /// Only playlists
    Playlist,
}

impl VideoType {
    fn as_str(&self) -> &str {
        match self {
            VideoType::Video => "video",
            VideoType::Channel => "channel",
            VideoType::Playlist => "playlist",
        }
    }
}

/// Order for search results
#[derive(Debug, Clone, Copy, Default)]
pub enum SearchOrder {
    /// Order by relevance (default)
    #[default]
    Relevance,
    /// Order by date
    Date,
    /// Order by view count
    ViewCount,
    /// Order by rating
    Rating,
}

impl SearchOrder {
    fn as_str(&self) -> &str {
        match self {
            SearchOrder::Relevance => "relevance",
            SearchOrder::Date => "date",
            SearchOrder::ViewCount => "viewCount",
            SearchOrder::Rating => "rating",
        }
    }
}

/// Video duration filter
#[derive(Debug, Clone, Copy)]
pub enum VideoDuration {
    /// Videos less than 4 minutes
    Short,
    /// Videos between 4-20 minutes
    Medium,
    /// Videos longer than 20 minutes
    Long,
}

impl VideoDuration {
    fn as_str(&self) -> &str {
        match self {
            VideoDuration::Short => "short",
            VideoDuration::Medium => "medium",
            VideoDuration::Long => "long",
        }
    }
}

/// Video definition filter
#[derive(Debug, Clone, Copy)]
pub enum VideoDefinition {
    /// HD videos
    High,
    /// SD videos
    Standard,
}

impl VideoDefinition {
    fn as_str(&self) -> &str {
        match self {
            VideoDefinition::High => "high",
            VideoDefinition::Standard => "standard",
        }
    }
}

impl YouTubeSearchTool {
    /// Create a new YouTube search tool
    ///
    /// Reads API key from `YOUTUBE_API_KEY` environment variable.
    ///
    /// # Errors
    ///
    /// Returns an error if the API key is not set.
    pub fn new() -> Result<Self> {
        let api_key = env_string(YOUTUBE_API_KEY).ok_or_else(|| {
            dashflow::core::Error::tool_error(
                "YOUTUBE_API_KEY environment variable not set. \
                Get your API key from https://console.cloud.google.com/".to_string()
            )
        })?;

        Ok(Self {
            api_key,
            max_results: 5,
            video_type: VideoType::default(),
            order: SearchOrder::default(),
            video_duration: None,
            video_definition: None,
            region_code: None,
            client: create_http_client(),
        })
    }

    /// Create a new YouTube search tool with a specific API key
    #[must_use]
    pub fn with_api_key(api_key: String) -> Self {
        Self {
            api_key,
            max_results: 5,
            video_type: VideoType::default(),
            order: SearchOrder::default(),
            video_duration: None,
            video_definition: None,
            region_code: None,
            client: create_http_client(),
        }
    }

    /// Create a builder for `YouTubeSearchTool`
    pub fn builder() -> YouTubeSearchToolBuilder {
        YouTubeSearchToolBuilder::default()
    }

    /// Set the maximum number of results
    #[must_use]
    pub fn with_max_results(mut self, max_results: usize) -> Self {
        self.max_results = max_results;
        self
    }

    /// Set the video type filter
    #[must_use]
    pub fn with_video_type(mut self, video_type: VideoType) -> Self {
        self.video_type = video_type;
        self
    }

    /// Set the result order
    #[must_use]
    pub fn with_order(mut self, order: SearchOrder) -> Self {
        self.order = order;
        self
    }

    /// Set the video duration filter
    #[must_use]
    pub fn with_video_duration(mut self, duration: VideoDuration) -> Self {
        self.video_duration = Some(duration);
        self
    }

    /// Set the video definition filter
    #[must_use]
    pub fn with_video_definition(mut self, definition: VideoDefinition) -> Self {
        self.video_definition = Some(definition);
        self
    }

    /// Set the region code (ISO 3166-1 alpha-2)
    #[must_use]
    pub fn with_region_code(mut self, region_code: String) -> Self {
        self.region_code = Some(region_code);
        self
    }

    /// Search YouTube videos
    pub async fn search(&self, query: &str) -> Result<Vec<YouTubeVideo>> {
        let mut url = format!(
            "{YOUTUBE_API_BASE}/search?part=snippet&q={}&type={}&order={}&maxResults={}&key={}",
            urlencoding::encode(query),
            self.video_type.as_str(),
            self.order.as_str(),
            self.max_results,
            self.api_key
        );

        // Add optional filters
        if let Some(duration) = &self.video_duration {
            url.push_str(&format!("&videoDuration={}", duration.as_str()));
        }

        if let Some(definition) = &self.video_definition {
            url.push_str(&format!("&videoDefinition={}", definition.as_str()));
        }

        if let Some(region) = &self.region_code {
            url.push_str(&format!("&regionCode={region}"));
        }

        let response = self.client.get(&url).send().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("YouTube API request failed: {e}"))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(dashflow::core::Error::tool_error(format!(
                "YouTube API error ({}): {}",
                status, body
            )));
        }

        // M-216: Use size-limited JSON parsing to prevent memory exhaustion
        let data: serde_json::Value = json_with_limit(response, SEARCH_RESPONSE_SIZE_LIMIT)
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!(
                    "Failed to parse YouTube API response: {e}"
                ))
            })?;

        let items = data
            .get("items")
            .and_then(|i| i.as_array())
            .ok_or_else(|| {
                dashflow::core::Error::tool_error("Invalid YouTube API response format".to_string())
            })?;

        let videos: Vec<YouTubeVideo> = items
            .iter()
            .filter_map(|item| {
                let id = item.get("id")?;
                let snippet = item.get("snippet")?;

                let video_id = id.get("videoId").and_then(|v| v.as_str())?.to_string();

                let title = snippet
                    .get("title")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();

                let description = snippet
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or("")
                    .to_string();

                let channel_title = snippet
                    .get("channelTitle")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                let channel_id = snippet
                    .get("channelId")
                    .and_then(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                let published_at = snippet
                    .get("publishedAt")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .to_string();

                let thumbnail_url = snippet
                    .get("thumbnails")
                    .and_then(|t| t.get("high"))
                    .or_else(|| snippet.get("thumbnails").and_then(|t| t.get("default")))
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());

                Some(YouTubeVideo {
                    video_id: video_id.clone(),
                    title,
                    description,
                    channel_title,
                    channel_id,
                    published_at,
                    thumbnail_url,
                    url: format!("https://www.youtube.com/watch?v={video_id}"),
                })
            })
            .collect();

        Ok(videos)
    }

    /// Format search results as a human-readable string
    fn format_results(&self, videos: &[YouTubeVideo], query: &str) -> String {
        if videos.is_empty() {
            return format!("No videos found for query: {query}");
        }

        let mut output = format!("Found {} videos for query: {}\n\n", videos.len(), query);

        for (i, video) in videos.iter().enumerate() {
            output.push_str(&format!("Video {}:\n", i + 1));
            output.push_str(&format!("Title: {}\n", video.title));
            output.push_str(&format!("Channel: {}\n", video.channel_title));
            output.push_str(&format!("Published: {}\n", video.published_at));
            output.push_str(&format!("URL: {}\n", video.url));

            // Truncate description if too long
            let desc = if video.description.len() > 200 {
                format!("{}...", &video.description[..200])
            } else {
                video.description.clone()
            };
            output.push_str(&format!("Description: {}\n", desc));
            output.push('\n');
        }

        output
    }
}

#[async_trait]
impl Tool for YouTubeSearchTool {
    fn name(&self) -> &'static str {
        "youtube_search"
    }

    fn description(&self) -> &'static str {
        "Search YouTube for videos. Returns video titles, descriptions, channel names, \
         publication dates, and URLs. Best for finding educational content, tutorials, \
         music, entertainment, and other video content. \
         Input should be a search query (e.g., 'rust programming tutorial', 'machine learning explained')."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query for finding YouTube videos"
                }
            },
            "required": ["query"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let query = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("query")
                .and_then(|q| q.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error(
                        "Missing 'query' field in structured input".to_string(),
                    )
                })?
                .to_string(),
        };

        let videos = self.search(&query).await?;
        Ok(self.format_results(&videos, &query))
    }
}

/// Builder for `YouTubeSearchTool`
#[derive(Default)]
pub struct YouTubeSearchToolBuilder {
    api_key: Option<String>,
    max_results: Option<usize>,
    video_type: Option<VideoType>,
    order: Option<SearchOrder>,
    video_duration: Option<VideoDuration>,
    video_definition: Option<VideoDefinition>,
    region_code: Option<String>,
}

impl YouTubeSearchToolBuilder {
    /// Set the API key
    #[must_use]
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set the maximum number of results
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Set the video type filter
    #[must_use]
    pub fn video_type(mut self, video_type: VideoType) -> Self {
        self.video_type = Some(video_type);
        self
    }

    /// Set the result order
    #[must_use]
    pub fn order(mut self, order: SearchOrder) -> Self {
        self.order = Some(order);
        self
    }

    /// Set the video duration filter
    #[must_use]
    pub fn video_duration(mut self, duration: VideoDuration) -> Self {
        self.video_duration = Some(duration);
        self
    }

    /// Set the video definition filter
    #[must_use]
    pub fn video_definition(mut self, definition: VideoDefinition) -> Self {
        self.video_definition = Some(definition);
        self
    }

    /// Set the region code
    #[must_use]
    pub fn region_code(mut self, region_code: String) -> Self {
        self.region_code = Some(region_code);
        self
    }

    /// Build the `YouTubeSearchTool`
    ///
    /// # Errors
    ///
    /// Returns an error if no API key is provided and `YOUTUBE_API_KEY` is not set.
    pub fn build(self) -> Result<YouTubeSearchTool> {
        let api_key = self
            .api_key
            .or_else(|| env_string(YOUTUBE_API_KEY))
            .ok_or_else(|| {
                dashflow::core::Error::tool_error(
                    "YOUTUBE_API_KEY not provided. Set it via builder or environment variable."
                        .to_string(),
                )
            })?;

        Ok(YouTubeSearchTool {
            api_key,
            max_results: self.max_results.unwrap_or(5),
            video_type: self.video_type.unwrap_or_default(),
            order: self.order.unwrap_or_default(),
            video_duration: self.video_duration,
            video_definition: self.video_definition,
            region_code: self.region_code,
            client: create_http_client(),
        })
    }
}

/// YouTube retriever for document retrieval from YouTube video search
///
/// Wraps `YouTubeSearchTool` and converts search results into Documents
/// suitable for use in retrieval chains and RAG applications.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_youtube::YouTubeRetriever;
/// use dashflow::core::retrievers::Retriever;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// std::env::set_var("YOUTUBE_API_KEY", "your-api-key");
///
/// let retriever = YouTubeRetriever::new()?;
///
/// let docs = retriever._get_relevant_documents("rust programming", None).await?;
///
/// for doc in docs {
///     println!("Title: {}", doc.metadata.get("title").unwrap());
/// }
/// # Ok(())
/// # }
/// ```
pub struct YouTubeRetriever {
    /// Internal search tool
    tool: YouTubeSearchTool,
}

impl YouTubeRetriever {
    /// Create a new `YouTubeRetriever`
    ///
    /// Reads API key from `YOUTUBE_API_KEY` environment variable.
    pub fn new() -> Result<Self> {
        Ok(Self {
            tool: YouTubeSearchTool::new()?,
        })
    }

    /// Create a new `YouTubeRetriever` with a specific API key
    #[must_use]
    pub fn with_api_key(api_key: String) -> Self {
        Self {
            tool: YouTubeSearchTool::with_api_key(api_key),
        }
    }

    /// Create a builder for `YouTubeRetriever`
    pub fn builder() -> YouTubeRetrieverBuilder {
        YouTubeRetrieverBuilder::default()
    }

    /// Convert `YouTubeVideo` to Document
    fn video_to_document(video: &YouTubeVideo) -> Document {
        let mut metadata = HashMap::new();

        metadata.insert(
            "source".to_string(),
            serde_json::Value::String(video.url.clone()),
        );
        metadata.insert(
            "video_id".to_string(),
            serde_json::Value::String(video.video_id.clone()),
        );
        metadata.insert(
            "title".to_string(),
            serde_json::Value::String(video.title.clone()),
        );
        metadata.insert(
            "channel_title".to_string(),
            serde_json::Value::String(video.channel_title.clone()),
        );
        metadata.insert(
            "channel_id".to_string(),
            serde_json::Value::String(video.channel_id.clone()),
        );
        metadata.insert(
            "published_at".to_string(),
            serde_json::Value::String(video.published_at.clone()),
        );

        if let Some(thumbnail) = &video.thumbnail_url {
            metadata.insert(
                "thumbnail_url".to_string(),
                serde_json::Value::String(thumbnail.clone()),
            );
        }

        let page_content = format!(
            "Title: {}\nChannel: {}\nPublished: {}\n\n{}",
            video.title, video.channel_title, video.published_at, video.description
        );

        Document {
            page_content,
            metadata,
            id: Some(video.video_id.clone()),
        }
    }
}

#[async_trait]
impl Retriever for YouTubeRetriever {
    async fn _get_relevant_documents(
        &self,
        query: &str,
        _config: Option<&RunnableConfig>,
    ) -> Result<Vec<Document>> {
        let videos = self.tool.search(query).await?;

        Ok(videos.iter().map(Self::video_to_document).collect())
    }
}

/// Builder for `YouTubeRetriever`
#[derive(Default)]
pub struct YouTubeRetrieverBuilder {
    api_key: Option<String>,
    max_results: Option<usize>,
    video_type: Option<VideoType>,
    order: Option<SearchOrder>,
    video_duration: Option<VideoDuration>,
    region_code: Option<String>,
}

impl YouTubeRetrieverBuilder {
    /// Set the API key
    #[must_use]
    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    /// Set the maximum number of results
    #[must_use]
    pub fn max_results(mut self, max_results: usize) -> Self {
        self.max_results = Some(max_results);
        self
    }

    /// Set the video type filter
    #[must_use]
    pub fn video_type(mut self, video_type: VideoType) -> Self {
        self.video_type = Some(video_type);
        self
    }

    /// Set the result order
    #[must_use]
    pub fn order(mut self, order: SearchOrder) -> Self {
        self.order = Some(order);
        self
    }

    /// Set the video duration filter
    #[must_use]
    pub fn video_duration(mut self, duration: VideoDuration) -> Self {
        self.video_duration = Some(duration);
        self
    }

    /// Set the region code
    #[must_use]
    pub fn region_code(mut self, region_code: String) -> Self {
        self.region_code = Some(region_code);
        self
    }

    /// Build the `YouTubeRetriever`
    pub fn build(self) -> Result<YouTubeRetriever> {
        let tool_builder = YouTubeSearchToolBuilder::default();

        let tool_builder = if let Some(api_key) = self.api_key {
            tool_builder.api_key(api_key)
        } else {
            tool_builder
        };

        let tool_builder = if let Some(max_results) = self.max_results {
            tool_builder.max_results(max_results)
        } else {
            tool_builder
        };

        let tool_builder = if let Some(video_type) = self.video_type {
            tool_builder.video_type(video_type)
        } else {
            tool_builder
        };

        let tool_builder = if let Some(order) = self.order {
            tool_builder.order(order)
        } else {
            tool_builder
        };

        let tool_builder = if let Some(duration) = self.video_duration {
            tool_builder.video_duration(duration)
        } else {
            tool_builder
        };

        let tool_builder = if let Some(region) = self.region_code {
            tool_builder.region_code(region)
        } else {
            tool_builder
        };

        Ok(YouTubeRetriever {
            tool: tool_builder.build()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // ==========================================================================
    // YouTubeSearchTool Creation Tests
    // ==========================================================================

    #[test]
    fn test_search_tool_creation_fails_without_api_key() {
        // Ensure env var is not set
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeSearchTool::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_search_tool_with_api_key() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        assert_eq!(tool.name(), "youtube_search");
        assert!(tool.description().contains("YouTube"));
        assert_eq!(tool.max_results, 5);
    }

    #[test]
    fn test_search_tool_default_values() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        assert_eq!(tool.max_results, 5);
        assert!(tool.video_duration.is_none());
        assert!(tool.video_definition.is_none());
        assert!(tool.region_code.is_none());
    }

    #[test]
    fn test_search_tool_with_max_results() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_max_results(20);
        assert_eq!(tool.max_results, 20);
    }

    #[test]
    fn test_search_tool_with_video_type() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_video_type(VideoType::Channel);
        assert!(matches!(tool.video_type, VideoType::Channel));
    }

    #[test]
    fn test_search_tool_with_order() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_order(SearchOrder::Date);
        assert!(matches!(tool.order, SearchOrder::Date));
    }

    #[test]
    fn test_search_tool_with_video_duration() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_video_duration(VideoDuration::Long);
        assert!(matches!(tool.video_duration, Some(VideoDuration::Long)));
    }

    #[test]
    fn test_search_tool_with_video_definition() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_video_definition(VideoDefinition::High);
        assert!(matches!(tool.video_definition, Some(VideoDefinition::High)));
    }

    #[test]
    fn test_search_tool_with_region_code() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_region_code("US".to_string());
        assert_eq!(tool.region_code, Some("US".to_string()));
    }

    #[test]
    fn test_search_tool_chained_builders() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
            .with_max_results(15)
            .with_video_type(VideoType::Video)
            .with_order(SearchOrder::ViewCount)
            .with_video_duration(VideoDuration::Medium)
            .with_video_definition(VideoDefinition::High)
            .with_region_code("GB".to_string());

        assert_eq!(tool.max_results, 15);
        assert!(matches!(tool.video_type, VideoType::Video));
        assert!(matches!(tool.order, SearchOrder::ViewCount));
        assert!(matches!(tool.video_duration, Some(VideoDuration::Medium)));
        assert!(matches!(tool.video_definition, Some(VideoDefinition::High)));
        assert_eq!(tool.region_code, Some("GB".to_string()));
    }

    // ==========================================================================
    // YouTubeSearchToolBuilder Tests
    // ==========================================================================

    #[test]
    fn test_search_tool_builder_with_api_key() {
        let result = YouTubeSearchTool::builder()
            .api_key("test-key".to_string())
            .max_results(10)
            .video_type(VideoType::Video)
            .order(SearchOrder::ViewCount)
            .build();

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert_eq!(tool.max_results, 10);
    }

    #[test]
    fn test_search_tool_builder_fails_without_api_key() {
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeSearchTool::builder().max_results(10).build();

        assert!(result.is_err());
    }

    #[test]
    fn test_search_tool_builder_default() {
        let builder = YouTubeSearchToolBuilder::default();
        // Default builder has no API key set
        env::remove_var("YOUTUBE_API_KEY");
        let result = builder.build();
        assert!(result.is_err());
    }

    #[test]
    fn test_search_tool_builder_all_options() {
        let result = YouTubeSearchTool::builder()
            .api_key("test-key".to_string())
            .max_results(25)
            .video_type(VideoType::Playlist)
            .order(SearchOrder::Rating)
            .video_duration(VideoDuration::Short)
            .video_definition(VideoDefinition::Standard)
            .region_code("JP".to_string())
            .build();

        assert!(result.is_ok());
        let tool = result.unwrap();
        assert_eq!(tool.max_results, 25);
        assert!(matches!(tool.video_type, VideoType::Playlist));
        assert!(matches!(tool.order, SearchOrder::Rating));
        assert!(matches!(tool.video_duration, Some(VideoDuration::Short)));
        assert!(matches!(tool.video_definition, Some(VideoDefinition::Standard)));
        assert_eq!(tool.region_code, Some("JP".to_string()));
    }

    // ==========================================================================
    // Enum String Conversion Tests
    // ==========================================================================

    #[test]
    fn test_video_type_as_str() {
        assert_eq!(VideoType::Video.as_str(), "video");
        assert_eq!(VideoType::Channel.as_str(), "channel");
        assert_eq!(VideoType::Playlist.as_str(), "playlist");
    }

    #[test]
    fn test_video_type_default() {
        let default_type = VideoType::default();
        assert!(matches!(default_type, VideoType::Video));
    }

    #[test]
    fn test_search_order_as_str() {
        assert_eq!(SearchOrder::Relevance.as_str(), "relevance");
        assert_eq!(SearchOrder::Date.as_str(), "date");
        assert_eq!(SearchOrder::ViewCount.as_str(), "viewCount");
        assert_eq!(SearchOrder::Rating.as_str(), "rating");
    }

    #[test]
    fn test_search_order_default() {
        let default_order = SearchOrder::default();
        assert!(matches!(default_order, SearchOrder::Relevance));
    }

    #[test]
    fn test_video_duration_as_str() {
        assert_eq!(VideoDuration::Short.as_str(), "short");
        assert_eq!(VideoDuration::Medium.as_str(), "medium");
        assert_eq!(VideoDuration::Long.as_str(), "long");
    }

    #[test]
    fn test_video_definition_as_str() {
        assert_eq!(VideoDefinition::High.as_str(), "high");
        assert_eq!(VideoDefinition::Standard.as_str(), "standard");
    }

    // ==========================================================================
    // Tool Trait Implementation Tests
    // ==========================================================================

    #[test]
    fn test_tool_name() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        assert_eq!(tool.name(), "youtube_search");
    }

    #[test]
    fn test_tool_description_content() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let desc = tool.description();
        assert!(desc.contains("YouTube"));
        assert!(desc.contains("video"));
        assert!(desc.contains("search"));
    }

    #[test]
    fn test_args_schema() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let schema = tool.args_schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
        assert_eq!(schema["required"][0], "query");
    }

    #[test]
    fn test_args_schema_query_description() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let schema = tool.args_schema();

        let query_desc = schema["properties"]["query"]["description"].as_str();
        assert!(query_desc.is_some());
        assert!(query_desc.unwrap().contains("search"));
    }

    // ==========================================================================
    // Format Results Tests
    // ==========================================================================

    #[test]
    fn test_format_results_empty() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let result = tool.format_results(&[], "test query");
        assert!(result.contains("No videos found"));
        assert!(result.contains("test query"));
    }

    #[test]
    fn test_format_results_with_videos() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let videos = vec![YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test Video".to_string(),
            description: "Test description".to_string(),
            channel_title: "Test Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            url: "https://www.youtube.com/watch?v=test123".to_string(),
        }];

        let result = tool.format_results(&videos, "test query");

        assert!(result.contains("Found 1 videos"));
        assert!(result.contains("Test Video"));
        assert!(result.contains("Test Channel"));
        assert!(result.contains("test123"));
    }

    #[test]
    fn test_format_results_multiple_videos() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let videos = vec![
            YouTubeVideo {
                video_id: "video1".to_string(),
                title: "First Video".to_string(),
                description: "First description".to_string(),
                channel_title: "Channel One".to_string(),
                channel_id: "UC001".to_string(),
                published_at: "2024-01-01T00:00:00Z".to_string(),
                thumbnail_url: None,
                url: "https://www.youtube.com/watch?v=video1".to_string(),
            },
            YouTubeVideo {
                video_id: "video2".to_string(),
                title: "Second Video".to_string(),
                description: "Second description".to_string(),
                channel_title: "Channel Two".to_string(),
                channel_id: "UC002".to_string(),
                published_at: "2024-02-01T00:00:00Z".to_string(),
                thumbnail_url: None,
                url: "https://www.youtube.com/watch?v=video2".to_string(),
            },
        ];

        let result = tool.format_results(&videos, "multi query");

        assert!(result.contains("Found 2 videos"));
        assert!(result.contains("Video 1:"));
        assert!(result.contains("Video 2:"));
        assert!(result.contains("First Video"));
        assert!(result.contains("Second Video"));
    }

    #[test]
    fn test_format_results_long_description_truncated() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let long_description = "A".repeat(300);
        let videos = vec![YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test Video".to_string(),
            description: long_description.clone(),
            channel_title: "Test Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            thumbnail_url: None,
            url: "https://www.youtube.com/watch?v=test123".to_string(),
        }];

        let result = tool.format_results(&videos, "test query");

        // Description should be truncated to 200 chars + "..."
        assert!(result.contains("..."));
        assert!(!result.contains(&long_description));
    }

    #[test]
    fn test_format_results_short_description_not_truncated() {
        let tool = YouTubeSearchTool::with_api_key("test-key".to_string());
        let short_description = "Short description here.";
        let videos = vec![YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test Video".to_string(),
            description: short_description.to_string(),
            channel_title: "Test Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            thumbnail_url: None,
            url: "https://www.youtube.com/watch?v=test123".to_string(),
        }];

        let result = tool.format_results(&videos, "test query");

        assert!(result.contains(short_description));
    }

    // ==========================================================================
    // YouTubeVideo Tests
    // ==========================================================================

    #[test]
    fn test_youtube_video_serialization() {
        let video = YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test Video".to_string(),
            description: "A description".to_string(),
            channel_title: "Test Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            url: "https://www.youtube.com/watch?v=test123".to_string(),
        };

        let json = serde_json::to_string(&video).unwrap();
        assert!(json.contains("test123"));
        assert!(json.contains("Test Video"));
    }

    #[test]
    fn test_youtube_video_deserialization() {
        let json = r#"{
            "video_id": "abc123",
            "title": "Deserialized Video",
            "description": "Desc",
            "channel_title": "Channel",
            "channel_id": "UC456",
            "published_at": "2024-06-15T12:00:00Z",
            "thumbnail_url": null,
            "url": "https://youtube.com/watch?v=abc123"
        }"#;

        let video: YouTubeVideo = serde_json::from_str(json).unwrap();
        assert_eq!(video.video_id, "abc123");
        assert_eq!(video.title, "Deserialized Video");
        assert!(video.thumbnail_url.is_none());
    }

    #[test]
    fn test_youtube_video_roundtrip() {
        let original = YouTubeVideo {
            video_id: "xyz789".to_string(),
            title: "Roundtrip Test".to_string(),
            description: "Testing roundtrip".to_string(),
            channel_title: "RT Channel".to_string(),
            channel_id: "UC789".to_string(),
            published_at: "2024-03-15T08:30:00Z".to_string(),
            thumbnail_url: Some("https://img.youtube.com/vi/xyz789/default.jpg".to_string()),
            url: "https://www.youtube.com/watch?v=xyz789".to_string(),
        };

        let json = serde_json::to_string(&original).unwrap();
        let deserialized: YouTubeVideo = serde_json::from_str(&json).unwrap();

        assert_eq!(original.video_id, deserialized.video_id);
        assert_eq!(original.title, deserialized.title);
        assert_eq!(original.thumbnail_url, deserialized.thumbnail_url);
    }

    #[test]
    fn test_youtube_video_debug() {
        let video = YouTubeVideo {
            video_id: "debug123".to_string(),
            title: "Debug Test".to_string(),
            description: "".to_string(),
            channel_title: "".to_string(),
            channel_id: "".to_string(),
            published_at: "".to_string(),
            thumbnail_url: None,
            url: "".to_string(),
        };

        let debug = format!("{:?}", video);
        assert!(debug.contains("YouTubeVideo"));
        assert!(debug.contains("debug123"));
    }

    #[test]
    fn test_youtube_video_clone() {
        let original = YouTubeVideo {
            video_id: "clone123".to_string(),
            title: "Clone Test".to_string(),
            description: "Testing clone".to_string(),
            channel_title: "Clone Channel".to_string(),
            channel_id: "UCclone".to_string(),
            published_at: "2024-01-01".to_string(),
            thumbnail_url: Some("https://thumb.jpg".to_string()),
            url: "https://youtube.com/watch?v=clone123".to_string(),
        };

        let cloned = original.clone();
        assert_eq!(original.video_id, cloned.video_id);
        assert_eq!(original.title, cloned.title);
    }

    // ==========================================================================
    // YouTubeRetriever Tests
    // ==========================================================================

    #[test]
    fn test_video_to_document() {
        let video = YouTubeVideo {
            video_id: "test123".to_string(),
            title: "Test Video".to_string(),
            description: "Test description".to_string(),
            channel_title: "Test Channel".to_string(),
            channel_id: "UC123".to_string(),
            published_at: "2024-01-01T00:00:00Z".to_string(),
            thumbnail_url: Some("https://example.com/thumb.jpg".to_string()),
            url: "https://www.youtube.com/watch?v=test123".to_string(),
        };

        let doc = YouTubeRetriever::video_to_document(&video);

        assert!(doc.page_content.contains("Test Video"));
        assert!(doc.page_content.contains("Test Channel"));
        assert_eq!(
            doc.metadata.get("video_id"),
            Some(&serde_json::Value::String("test123".to_string()))
        );
        assert_eq!(doc.id, Some("test123".to_string()));
    }

    #[test]
    fn test_video_to_document_metadata_complete() {
        let video = YouTubeVideo {
            video_id: "meta123".to_string(),
            title: "Metadata Test".to_string(),
            description: "Testing metadata".to_string(),
            channel_title: "Meta Channel".to_string(),
            channel_id: "UCmeta".to_string(),
            published_at: "2024-05-20T15:30:00Z".to_string(),
            thumbnail_url: Some("https://thumb.example.com/img.jpg".to_string()),
            url: "https://www.youtube.com/watch?v=meta123".to_string(),
        };

        let doc = YouTubeRetriever::video_to_document(&video);

        assert!(doc.metadata.contains_key("source"));
        assert!(doc.metadata.contains_key("video_id"));
        assert!(doc.metadata.contains_key("title"));
        assert!(doc.metadata.contains_key("channel_title"));
        assert!(doc.metadata.contains_key("channel_id"));
        assert!(doc.metadata.contains_key("published_at"));
        assert!(doc.metadata.contains_key("thumbnail_url"));
    }

    #[test]
    fn test_video_to_document_no_thumbnail() {
        let video = YouTubeVideo {
            video_id: "nothumb123".to_string(),
            title: "No Thumbnail".to_string(),
            description: "No thumb".to_string(),
            channel_title: "Channel".to_string(),
            channel_id: "UC".to_string(),
            published_at: "2024-01-01".to_string(),
            thumbnail_url: None,
            url: "https://youtube.com/watch?v=nothumb123".to_string(),
        };

        let doc = YouTubeRetriever::video_to_document(&video);

        // thumbnail_url should not be in metadata when None
        assert!(!doc.metadata.contains_key("thumbnail_url"));
    }

    #[test]
    fn test_video_to_document_page_content_format() {
        let video = YouTubeVideo {
            video_id: "format123".to_string(),
            title: "Content Format".to_string(),
            description: "Description here".to_string(),
            channel_title: "Format Channel".to_string(),
            channel_id: "UCformat".to_string(),
            published_at: "2024-07-04T00:00:00Z".to_string(),
            thumbnail_url: None,
            url: "https://youtube.com/watch?v=format123".to_string(),
        };

        let doc = YouTubeRetriever::video_to_document(&video);

        // Check the page content format
        assert!(doc.page_content.contains("Title: Content Format"));
        assert!(doc.page_content.contains("Channel: Format Channel"));
        assert!(doc.page_content.contains("Published: 2024-07-04T00:00:00Z"));
        assert!(doc.page_content.contains("Description here"));
    }

    #[test]
    fn test_retriever_creation_fails_without_api_key() {
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeRetriever::new();
        assert!(result.is_err());
    }

    #[test]
    fn test_retriever_with_api_key() {
        let retriever = YouTubeRetriever::with_api_key("test-key".to_string());
        assert_eq!(retriever.tool.max_results, 5);
    }

    #[test]
    fn test_retriever_builder() {
        let result = YouTubeRetriever::builder()
            .api_key("test-key".to_string())
            .max_results(10)
            .order(SearchOrder::Date)
            .build();

        assert!(result.is_ok());
        let retriever = result.unwrap();
        // Verify the retriever was built with expected properties
        assert_eq!(retriever.tool.max_results, 10);
    }

    #[test]
    fn test_retriever_builder_all_options() {
        let result = YouTubeRetriever::builder()
            .api_key("test-key".to_string())
            .max_results(15)
            .video_type(VideoType::Video)
            .order(SearchOrder::ViewCount)
            .video_duration(VideoDuration::Medium)
            .region_code("DE".to_string())
            .build();

        assert!(result.is_ok());
        let retriever = result.unwrap();
        assert_eq!(retriever.tool.max_results, 15);
    }

    #[test]
    fn test_retriever_builder_fails_without_api_key() {
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeRetriever::builder()
            .max_results(10)
            .build();

        assert!(result.is_err());
    }

    // ==========================================================================
    // API URL Construction Tests (via indirect observation)
    // ==========================================================================

    #[test]
    fn test_api_base_url_constant() {
        // Verify the API base URL is correct
        assert_eq!(YOUTUBE_API_BASE, "https://www.googleapis.com/youtube/v3");
    }

    // ==========================================================================
    // Region Code Tests
    // ==========================================================================

    #[test]
    fn test_region_codes_common() {
        // Test common ISO 3166-1 alpha-2 region codes
        let regions = ["US", "GB", "CA", "AU", "DE", "FR", "JP", "KR", "BR", "IN"];

        for region in regions {
            let tool = YouTubeSearchTool::with_api_key("test-key".to_string())
                .with_region_code(region.to_string());
            assert_eq!(tool.region_code, Some(region.to_string()));
        }
    }

    // ==========================================================================
    // Error Message Tests
    // ==========================================================================

    #[test]
    fn test_error_message_no_api_key() {
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeSearchTool::new();
        assert!(result.is_err());
        // Check error message via match
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("YOUTUBE_API_KEY") || err_msg.contains("API key"));
        }
    }

    #[test]
    fn test_builder_error_message_no_api_key() {
        env::remove_var("YOUTUBE_API_KEY");

        let result = YouTubeSearchTool::builder().build();
        assert!(result.is_err());
        // Check error message via match
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("YOUTUBE_API_KEY") || err_msg.contains("API key"));
        }
    }

    // ==========================================================================
    // Integration Tests (require API key and network access)
    // ==========================================================================

    #[tokio::test]
    #[ignore = "requires YOUTUBE_API_KEY environment variable"]
    async fn test_search_integration() {
        let tool = YouTubeSearchTool::new().unwrap();
        let videos = tool
            .search("rust programming")
            .await
            .expect("YouTube search failed");
        assert!(!videos.is_empty());
        assert!(!videos[0].title.is_empty());
        assert!(!videos[0].video_id.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires YOUTUBE_API_KEY environment variable"]
    async fn test_search_with_options() {
        let tool = YouTubeSearchTool::new()
            .unwrap()
            .with_max_results(3)
            .with_order(SearchOrder::ViewCount)
            .with_video_duration(VideoDuration::Medium);

        let videos = tool
            .search("rust programming tutorial")
            .await
            .expect("YouTube search failed");

        assert!(!videos.is_empty());
        assert!(videos.len() <= 3);
    }

    #[tokio::test]
    #[ignore = "requires YOUTUBE_API_KEY environment variable"]
    async fn test_retriever_integration() {
        let retriever = YouTubeRetriever::new().unwrap();
        let docs = retriever
            ._get_relevant_documents("rust programming", None)
            .await
            .expect("Retriever failed");

        assert!(!docs.is_empty());
        assert!(docs[0].metadata.contains_key("video_id"));
    }
}
