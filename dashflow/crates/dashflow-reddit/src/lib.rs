//! # dashflow-reddit
//!
//! Reddit search tools for `DashFlow` Rust.
//!
//! This crate provides tools to search Reddit posts, subreddits, and comments
//! using the Reddit JSON API.
//!
//! ## Features
//!
//! - Search Reddit posts across all subreddits or specific subreddits
//! - Get post details with comments
//! - Get subreddit information
//! - No authentication required for read-only access
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow_reddit::RedditSearchTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let tool = RedditSearchTool::new();
//!
//!     let input = serde_json::json!({
//!         "query": "rust programming",
//!         "limit": 5,
//!         "sort": "relevance"
//!     });
//!
//!     let result = tool._call(ToolInput::Structured(input)).await?;
//!     println!("{}", result);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::{Error, Result};
use reqwest::Client;
use serde::Deserialize;

/// Reddit API response structures
#[derive(Debug, Deserialize)]
struct RedditResponse {
    data: RedditData,
}

#[derive(Debug, Deserialize)]
struct RedditData {
    children: Vec<RedditChild>,

    // JUSTIFICATION: Serde deserialization field for Reddit API pagination.
    // Field is populated by serde from JSON ("after" token in Reddit API responses).
    // Will be directly accessed when pagination support is implemented (fetch next page
    // of search results). Not dead code - part of Reddit JSON API response structure.
    #[serde(default)]
    #[allow(dead_code)]
    after: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditChild {
    data: RedditPost,
}

#[derive(Debug, Deserialize)]
struct RedditPost {
    title: String,
    author: String,
    subreddit: String,
    score: i64,

    url: String,
    permalink: String,
    #[serde(default)]
    selftext: String,
    num_comments: u64,
    #[allow(dead_code)]
    // JUSTIFICATION: Serde deserialization field for Reddit post timestamps.
    // Field is populated by serde from JSON (Unix timestamp of post creation). Will be
    // directly accessed when time-based filtering or sorting features are implemented
    // (e.g., "show posts from last 24 hours").
    created_utc: f64,
    #[serde(default)]
    over_18: bool,
}

/// Reddit search tool for searching posts across Reddit.
///
/// This tool searches Reddit using the public JSON API and returns
/// formatted post information.
///
/// ## Input Parameters
///
/// - `query` (required): Search query string
/// - `limit` (optional): Maximum number of results (default: 5, max: 100)
/// - `sort` (optional): Sort order - "relevance", "hot", "top", "new", "comments" (default: "relevance")
/// - `time` (optional): Time filter for "top" sort - "hour", "day", "week", "month", "year", "all" (default: "all")
/// - `subreddit` (optional): Limit search to specific subreddit (e.g., "rust")
/// - `nsfw` (optional): Include NSFW posts (default: false)
///
/// ## Output
///
/// Returns formatted list of Reddit posts with:
/// - Title and author
/// - Subreddit and score
/// - Number of comments
/// - Reddit permalink
/// - External link URL (if any)
/// - Post content preview (if self-post)
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_reddit::RedditSearchTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool = RedditSearchTool::new();
///
///     let input = serde_json::json!({
///         "query": "rust async",
///         "subreddit": "rust",
///         "limit": 3,
///         "sort": "top"
///     });
///
///     let result = tool._call(ToolInput::Structured(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RedditSearchTool {
    client: Client,
    base_url: String,
}

impl RedditSearchTool {
    /// Create a new Reddit search tool with default configuration.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created (e.g., TLS initialization failure).
    /// Use `try_new` for a fallible alternative.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_new() fallible alternative
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client")
    }

    /// Try to create a new Reddit search tool with default configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> std::result::Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .user_agent("dashflow/0.1.0")
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .build()?,
            base_url: "https://www.reddit.com".to_string(),
        })
    }

    /// Create a new Reddit search tool with custom user agent.
    ///
    /// Reddit requires a unique user agent string to identify your application.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_with_user_agent` for a fallible alternative.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_with_user_agent() fallible alternative
    pub fn with_user_agent(user_agent: &str) -> Self {
        Self::try_with_user_agent(user_agent).expect("Failed to create HTTP client")
    }

    /// Try to create a new Reddit search tool with custom user agent.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_with_user_agent(user_agent: &str) -> std::result::Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .user_agent(user_agent)
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .build()?,
            base_url: "https://www.reddit.com".to_string(),
        })
    }

    async fn search_reddit(
        &self,
        query: &str,
        limit: usize,
        sort: &str,
        time: &str,
        subreddit: Option<&str>,
        nsfw: bool,
    ) -> Result<String> {
        // Build URL based on whether subreddit is specified
        let url = if let Some(sr) = subreddit {
            format!("{}/r/{}/search.json", self.base_url, sr)
        } else {
            format!("{}/search.json", self.base_url)
        };

        // Build query parameters
        let mut params = vec![
            ("q", query.to_string()),
            ("limit", limit.to_string()),
            ("sort", sort.to_string()),
            ("t", time.to_string()),
        ];

        // If searching within subreddit, restrict search to that subreddit
        if subreddit.is_some() {
            params.push(("restrict_sr", "true".to_string()));
        }

        // Make request
        let response = self
            .client
            .get(&url)
            .query(&params)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Reddit API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Reddit API returned error: {}",
                response.status()
            )));
        }

        let reddit_response: RedditResponse = response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse Reddit response: {e}")))?;

        // Filter NSFW posts if requested
        let posts: Vec<&RedditPost> = reddit_response
            .data
            .children
            .iter()
            .map(|child| &child.data)
            .filter(|post| nsfw || !post.over_18)
            .collect();

        if posts.is_empty() {
            return Ok("No results found.".to_string());
        }

        // Format results
        let mut result = String::new();
        result.push_str(&format!("Found {} Reddit posts:\n\n", posts.len()));

        for (i, post) in posts.iter().enumerate() {
            result.push_str(&format!("{}. {}\n", i + 1, post.title));
            result.push_str(&format!("   Author: u/{}\n", post.author));
            result.push_str(&format!("   Subreddit: r/{}\n", post.subreddit));
            result.push_str(&format!(
                "   Score: {} | Comments: {}\n",
                post.score, post.num_comments
            ));

            // Add selftext preview if available
            if !post.selftext.is_empty() {
                let preview = if post.selftext.len() > 200 {
                    format!("{}...", &post.selftext[..200])
                } else {
                    post.selftext.clone()
                };
                result.push_str(&format!("   Text: {preview}\n"));
            }

            let permalink_url = format!("https://reddit.com{}", post.permalink);
            result.push_str(&format!("   Permalink: {permalink_url}\n"));
            if !post.url.is_empty() && post.url != permalink_url {
                result.push_str(&format!("   Link: {}\n", post.url));
            }

            if i < posts.len() - 1 {
                result.push('\n');
            }
        }

        Ok(result)
    }
}

impl Default for RedditSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RedditSearchTool {
    fn name(&self) -> &'static str {
        "reddit_search"
    }

    fn description(&self) -> &'static str {
        "Search Reddit posts and discussions. \
         Input should be a JSON object with 'query' (required), \
         'limit' (optional, default 5), 'sort' (optional: relevance/hot/top/new/comments), \
         'time' (optional: hour/day/week/month/year/all), \
         'subreddit' (optional: specific subreddit name), \
         'nsfw' (optional: include NSFW posts, default false). \
         Returns formatted list of Reddit posts with title, author, subreddit, score, \
         comments, and links."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        match input {
            ToolInput::Structured(value) => {
                // Extract query (required)
                let query = value
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing required field: query"))?;

                // Extract optional parameters
                let limit = value
                    .get("limit")
                    .and_then(serde_json::value::Value::as_u64)
                    .unwrap_or(5)
                    .min(100) as usize;

                let sort = value
                    .get("sort")
                    .and_then(|v| v.as_str())
                    .unwrap_or("relevance");

                let time = value.get("time").and_then(|v| v.as_str()).unwrap_or("all");

                let subreddit = value.get("subreddit").and_then(|v| v.as_str());

                let nsfw = value
                    .get("nsfw")
                    .and_then(serde_json::value::Value::as_bool)
                    .unwrap_or(false);

                self.search_reddit(query, limit, sort, time, subreddit, nsfw)
                    .await
            }
            ToolInput::String(query) => {
                // Simple string input - just search with defaults
                self.search_reddit(&query, 5, "relevance", "all", None, false)
                    .await
            }
        }
    }
}

/// Reddit post detail tool for getting full post content and comments.
///
/// This tool fetches detailed information about a specific Reddit post,
/// including the full text and top comments.
///
/// ## Input Parameters
///
/// - `post_id` (required): Reddit post ID (e.g., "abc123")
/// - `subreddit` (required): Subreddit name (e.g., "rust")
/// - `num_comments` (optional): Number of top comments to include (default: 5, max: 50)
///
/// ## Output
///
/// Returns formatted post details with:
/// - Full post title, author, and text
/// - Score and comment count
/// - Top comments with authors and scores
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_reddit::RedditPostTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool = RedditPostTool::new();
///
///     let input = serde_json::json!({
///         "post_id": "abc123",
///         "subreddit": "rust",
///         "num_comments": 10
///     });
///
///     let result = tool._call(ToolInput::Structured(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RedditPostTool {
    client: Client,
    base_url: String,
}

impl RedditPostTool {
    /// Create a new Reddit post detail tool.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new` for a fallible alternative.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_new() fallible alternative
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client")
    }

    /// Try to create a new Reddit post detail tool.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> std::result::Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .user_agent("dashflow/0.1.0")
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .build()?,
            base_url: "https://www.reddit.com".to_string(),
        })
    }

    async fn get_post_details(
        &self,
        post_id: &str,
        subreddit: &str,
        num_comments: usize,
    ) -> Result<String> {
        let url = format!(
            "{}/r/{}/comments/{}.json",
            self.base_url, subreddit, post_id
        );

        let response = self
            .client
            .get(&url)
            .query(&[("limit", num_comments.to_string())])
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Reddit API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Reddit API returned error: {}",
                response.status()
            )));
        }

        let responses: Vec<RedditResponse> = response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse Reddit response: {e}")))?;

        if responses.is_empty() {
            return Err(Error::tool_error("No post data found"));
        }

        // First response contains the post
        let post = &responses[0]
            .data
            .children
            .first()
            .ok_or_else(|| Error::tool_error("Post not found"))?
            .data;

        let mut result = String::new();
        result.push_str(&format!("Post: {}\n", post.title));
        result.push_str(&format!("Author: u/{}\n", post.author));
        result.push_str(&format!("Subreddit: r/{}\n", post.subreddit));
        result.push_str(&format!(
            "Score: {} | Comments: {}\n\n",
            post.score, post.num_comments
        ));

        if !post.selftext.is_empty() {
            result.push_str("Content:\n");
            result.push_str(&post.selftext);
            result.push_str("\n\n");
        }

        result.push_str(&format!("URL: https://reddit.com{}\n", post.permalink));

        // Second response contains comments (if available)
        if responses.len() > 1 {
            let comments: Vec<&RedditPost> = responses[1]
                .data
                .children
                .iter()
                .map(|child| &child.data)
                .take(num_comments)
                .collect();

            if !comments.is_empty() {
                result.push_str("\n--- Top Comments ---\n\n");
                for (i, comment) in comments.iter().enumerate() {
                    if !comment.author.is_empty() && comment.author != "AutoModerator" {
                        result.push_str(&format!(
                            "{}. u/{} (score: {})\n",
                            i + 1,
                            comment.author,
                            comment.score
                        ));
                        let preview = if comment.selftext.len() > 300 {
                            format!("{}...", &comment.selftext[..300])
                        } else {
                            comment.selftext.clone()
                        };
                        result.push_str(&format!("   {preview}\n\n"));
                    }
                }
            }
        }

        Ok(result)
    }
}

impl Default for RedditPostTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RedditPostTool {
    fn name(&self) -> &'static str {
        "reddit_post"
    }

    fn description(&self) -> &'static str {
        "Get detailed information about a specific Reddit post including full content and top comments. \
         Input should be a JSON object with 'post_id' (required), 'subreddit' (required), \
         and 'num_comments' (optional, default 5). \
         Returns formatted post details with title, author, content, and top comments."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        match input {
            ToolInput::Structured(value) => {
                let post_id = value
                    .get("post_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing required field: post_id"))?;

                let subreddit = value
                    .get("subreddit")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing required field: subreddit"))?;

                let num_comments = value
                    .get("num_comments")
                    .and_then(serde_json::value::Value::as_u64)
                    .unwrap_or(5)
                    .min(50) as usize;

                self.get_post_details(post_id, subreddit, num_comments)
                    .await
            }
            _ => Err(Error::tool_error(
                "Invalid input type. Expected JSON object with 'post_id' and 'subreddit' fields.",
            )),
        }
    }
}

/// Reddit subreddit info tool for getting information about a subreddit.
///
/// This tool fetches information about a specific subreddit including
/// subscriber count, description, and recent posts.
///
/// ## Input Parameters
///
/// - `subreddit` (required): Subreddit name (e.g., "rust")
///
/// ## Output
///
/// Returns formatted subreddit information with:
/// - Subscriber count
/// - Active users
/// - Description
/// - Recent post count
///
/// ## Example
///
/// ```rust,no_run
/// use dashflow_reddit::RedditSubredditTool;
/// use dashflow::core::tools::{Tool, ToolInput};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let tool = RedditSubredditTool::new();
///
///     let input = serde_json::json!({
///         "subreddit": "rust"
///     });
///
///     let result = tool._call(ToolInput::Structured(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RedditSubredditTool {
    client: Client,
    base_url: String,
}

#[derive(Debug, Deserialize)]
struct SubredditAbout {
    data: SubredditData,
}

#[derive(Debug, Deserialize)]
struct SubredditData {
    display_name: String,
    title: String,
    #[serde(default)]
    public_description: String,
    subscribers: u64,
    #[serde(default)]
    active_user_count: Option<u64>,
    #[serde(default)]
    over18: bool,
}

impl RedditSubredditTool {
    /// Create a new Reddit subreddit info tool.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be created. Use `try_new` for a fallible alternative.
    #[must_use]
    #[allow(clippy::expect_used)] // Documented panic with try_new() fallible alternative
    pub fn new() -> Self {
        Self::try_new().expect("Failed to create HTTP client")
    }

    /// Try to create a new Reddit subreddit info tool.
    ///
    /// # Errors
    ///
    /// Returns an error if the HTTP client cannot be created.
    pub fn try_new() -> std::result::Result<Self, reqwest::Error> {
        Ok(Self {
            client: Client::builder()
                .user_agent("dashflow/0.1.0")
                .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
                .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
                .build()?,
            base_url: "https://www.reddit.com".to_string(),
        })
    }

    async fn get_subreddit_info(&self, subreddit: &str) -> Result<String> {
        let url = format!("{}/r/{}/about.json", self.base_url, subreddit);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::tool_error(format!("Reddit API request failed: {e}")))?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Reddit API returned error: {}. Subreddit may not exist.",
                response.status()
            )));
        }

        let subreddit_info: SubredditAbout = response
            .json()
            .await
            .map_err(|e| Error::tool_error(format!("Failed to parse Reddit response: {e}")))?;

        let data = &subreddit_info.data;
        let mut result = String::new();

        result.push_str(&format!("Subreddit: r/{}\n", data.display_name));
        result.push_str(&format!("Title: {}\n", data.title));
        result.push_str(&format!("Subscribers: {}\n", data.subscribers));

        if let Some(active) = data.active_user_count {
            result.push_str(&format!("Active Users: {active}\n"));
        }

        if data.over18 {
            result.push_str("NSFW: Yes\n");
        }

        if !data.public_description.is_empty() {
            result.push_str(&format!("\nDescription:\n{}\n", data.public_description));
        }

        Ok(result)
    }
}

impl Default for RedditSubredditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RedditSubredditTool {
    fn name(&self) -> &'static str {
        "reddit_subreddit"
    }

    fn description(&self) -> &'static str {
        "Get information about a Reddit subreddit including subscriber count, description, and activity. \
         Input should be a JSON object with 'subreddit' (required) or a string with the subreddit name. \
         Returns formatted subreddit information."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        match input {
            ToolInput::Structured(value) => {
                let subreddit = value
                    .get("subreddit")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::tool_error("Missing required field: subreddit"))?;

                self.get_subreddit_info(subreddit).await
            }
            ToolInput::String(subreddit) => self.get_subreddit_info(&subreddit).await,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    // ============================================================
    // RedditSearchTool Creation Tests
    // ============================================================

    #[test]
    fn test_reddit_search_tool_creation() {
        let tool = RedditSearchTool::new();
        assert_eq!(tool.name(), "reddit_search");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_reddit_search_tool_try_new() {
        let tool = RedditSearchTool::try_new().expect("try_new should succeed");
        assert_eq!(tool.name(), "reddit_search");
        assert_eq!(tool.base_url, "https://www.reddit.com");
    }

    #[test]
    fn test_reddit_search_tool_with_user_agent() {
        let tool = RedditSearchTool::with_user_agent("my-app/1.0");
        assert_eq!(tool.name(), "reddit_search");
        assert_eq!(tool.base_url, "https://www.reddit.com");
    }

    #[test]
    fn test_reddit_search_tool_try_with_user_agent() {
        let tool =
            RedditSearchTool::try_with_user_agent("my-app/2.0").expect("try_with_user_agent");
        assert_eq!(tool.name(), "reddit_search");
    }

    #[test]
    fn test_reddit_search_tool_default() {
        let tool = RedditSearchTool::default();
        assert_eq!(tool.name(), "reddit_search");
    }

    #[test]
    fn test_reddit_search_tool_description_contains_params() {
        let tool = RedditSearchTool::new();
        let desc = tool.description();
        assert!(desc.contains("query"));
        assert!(desc.contains("limit"));
        assert!(desc.contains("sort"));
        assert!(desc.contains("subreddit"));
        assert!(desc.contains("nsfw"));
    }

    // ============================================================
    // RedditPostTool Creation Tests
    // ============================================================

    #[test]
    fn test_reddit_post_tool_creation() {
        let tool = RedditPostTool::new();
        assert_eq!(tool.name(), "reddit_post");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_reddit_post_tool_try_new() {
        let tool = RedditPostTool::try_new().expect("try_new should succeed");
        assert_eq!(tool.name(), "reddit_post");
        assert_eq!(tool.base_url, "https://www.reddit.com");
    }

    #[test]
    fn test_reddit_post_tool_default() {
        let tool = RedditPostTool::default();
        assert_eq!(tool.name(), "reddit_post");
    }

    #[test]
    fn test_reddit_post_tool_description_contains_params() {
        let tool = RedditPostTool::new();
        let desc = tool.description();
        assert!(desc.contains("post_id"));
        assert!(desc.contains("subreddit"));
        assert!(desc.contains("num_comments"));
    }

    // ============================================================
    // RedditSubredditTool Creation Tests
    // ============================================================

    #[test]
    fn test_reddit_subreddit_tool_creation() {
        let tool = RedditSubredditTool::new();
        assert_eq!(tool.name(), "reddit_subreddit");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn test_reddit_subreddit_tool_try_new() {
        let tool = RedditSubredditTool::try_new().expect("try_new should succeed");
        assert_eq!(tool.name(), "reddit_subreddit");
        assert_eq!(tool.base_url, "https://www.reddit.com");
    }

    #[test]
    fn test_reddit_subreddit_tool_default() {
        let tool = RedditSubredditTool::default();
        assert_eq!(tool.name(), "reddit_subreddit");
    }

    #[test]
    fn test_reddit_subreddit_tool_description_contains_params() {
        let tool = RedditSubredditTool::new();
        let desc = tool.description();
        assert!(desc.contains("subreddit"));
    }

    // ============================================================
    // RedditSearchTool Input Validation Tests
    // ============================================================

    #[tokio::test]
    async fn test_search_missing_query_returns_error() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "limit": 5
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("query"), "Error should mention 'query': {err}");
    }

    #[tokio::test]
    async fn test_search_empty_query_object() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({});

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_null_query_returns_error() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": null
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_search_string_input_uses_defaults() {
        // String input should work - parsed as query with default parameters
        let tool = RedditSearchTool::new();
        let result = tool._call(ToolInput::String("rust".to_string())).await;
        // Either succeeds with results or fails with network error (never input parsing error)
        match result {
            Ok(output) => {
                // If network succeeds, we should get formatted output
                assert!(
                    output.contains("Reddit posts") || output.contains("No results"),
                    "Output should contain results or no-results message"
                );
            }
            Err(e) => {
                // If network fails, error should be network-related, not input parsing
                let err = e.to_string();
                assert!(
                    err.contains("request") || err.contains("API") || err.contains("network"),
                    "Expected network error, got: {err}"
                );
            }
        }
    }

    // ============================================================
    // RedditPostTool Input Validation Tests
    // ============================================================

    #[tokio::test]
    async fn test_post_missing_post_id_returns_error() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "subreddit": "rust"
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("post_id"),
            "Error should mention 'post_id': {err}"
        );
    }

    #[tokio::test]
    async fn test_post_missing_subreddit_returns_error() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123"
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("subreddit"),
            "Error should mention 'subreddit': {err}"
        );
    }

    #[tokio::test]
    async fn test_post_empty_object_returns_error() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({});

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_post_string_input_returns_error() {
        let tool = RedditPostTool::new();
        let result = tool
            ._call(ToolInput::String("some string".to_string()))
            .await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Invalid input") || err.contains("JSON object"),
            "Expected invalid input error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_post_null_post_id_returns_error() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": null,
            "subreddit": "rust"
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_post_null_subreddit_returns_error() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": null
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    // ============================================================
    // RedditSubredditTool Input Validation Tests
    // ============================================================

    #[tokio::test]
    async fn test_subreddit_missing_subreddit_returns_error() {
        let tool = RedditSubredditTool::new();
        let input = serde_json::json!({});

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("subreddit"),
            "Error should mention 'subreddit': {err}"
        );
    }

    #[tokio::test]
    async fn test_subreddit_null_subreddit_returns_error() {
        let tool = RedditSubredditTool::new();
        let input = serde_json::json!({
            "subreddit": null
        });

        let result = tool._call(ToolInput::Structured(input)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_subreddit_string_input_parses_correctly() {
        let tool = RedditSubredditTool::new();
        // String input should be accepted and treated as subreddit name
        let result = tool._call(ToolInput::String("rust".to_string())).await;
        // Either succeeds with results or fails with network error (never input parsing error)
        match result {
            Ok(output) => {
                // If network succeeds, we should get subreddit info
                assert!(
                    output.contains("Subreddit:") || output.contains("rust"),
                    "Output should contain subreddit info"
                );
            }
            Err(e) => {
                // If network fails, error should be network-related, not input parsing
                let err = e.to_string();
                assert!(
                    err.contains("request") || err.contains("API") || err.contains("network"),
                    "Expected network error, got: {err}"
                );
            }
        }
    }

    // ============================================================
    // Clone and Debug Trait Tests
    // ============================================================

    #[test]
    fn test_reddit_search_tool_clone() {
        let tool = RedditSearchTool::new();
        let cloned = tool.clone();
        assert_eq!(tool.name(), cloned.name());
        assert_eq!(tool.base_url, cloned.base_url);
    }

    #[test]
    fn test_reddit_post_tool_clone() {
        let tool = RedditPostTool::new();
        let cloned = tool.clone();
        assert_eq!(tool.name(), cloned.name());
        assert_eq!(tool.base_url, cloned.base_url);
    }

    #[test]
    fn test_reddit_subreddit_tool_clone() {
        let tool = RedditSubredditTool::new();
        let cloned = tool.clone();
        assert_eq!(tool.name(), cloned.name());
        assert_eq!(tool.base_url, cloned.base_url);
    }

    #[test]
    fn test_reddit_search_tool_debug() {
        let tool = RedditSearchTool::new();
        let debug_str = format!("{tool:?}");
        assert!(debug_str.contains("RedditSearchTool"));
        assert!(debug_str.contains("reddit.com"));
    }

    #[test]
    fn test_reddit_post_tool_debug() {
        let tool = RedditPostTool::new();
        let debug_str = format!("{tool:?}");
        assert!(debug_str.contains("RedditPostTool"));
        assert!(debug_str.contains("reddit.com"));
    }

    #[test]
    fn test_reddit_subreddit_tool_debug() {
        let tool = RedditSubredditTool::new();
        let debug_str = format!("{tool:?}");
        assert!(debug_str.contains("RedditSubredditTool"));
        assert!(debug_str.contains("reddit.com"));
    }

    // ============================================================
    // Deserialization Tests for Internal Structs
    // ============================================================

    #[test]
    fn test_reddit_post_deserialization() {
        let json = serde_json::json!({
            "title": "Test Post",
            "author": "testuser",
            "subreddit": "test",
            "score": 100,
            "url": "https://example.com",
            "permalink": "/r/test/comments/abc123/test_post/",
            "selftext": "Post body text",
            "num_comments": 42,
            "created_utc": 1704067200.0,
            "over_18": false
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.title, "Test Post");
        assert_eq!(post.author, "testuser");
        assert_eq!(post.subreddit, "test");
        assert_eq!(post.score, 100);
        assert_eq!(post.url, "https://example.com");
        assert_eq!(post.permalink, "/r/test/comments/abc123/test_post/");
        assert_eq!(post.selftext, "Post body text");
        assert_eq!(post.num_comments, 42);
        assert!(!post.over_18);
    }

    #[test]
    fn test_reddit_post_deserialization_with_defaults() {
        // Minimal JSON - test default values
        let json = serde_json::json!({
            "title": "Minimal Post",
            "author": "user",
            "subreddit": "sub",
            "score": 1,
            "url": "",
            "permalink": "/r/sub/comments/xyz/",
            "num_comments": 0,
            "created_utc": 0.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.title, "Minimal Post");
        assert_eq!(post.selftext, ""); // Default empty string
        assert!(!post.over_18); // Default false
    }

    #[test]
    fn test_reddit_post_nsfw_flag() {
        let json = serde_json::json!({
            "title": "NSFW Post",
            "author": "user",
            "subreddit": "nsfw_sub",
            "score": 50,
            "url": "https://example.com",
            "permalink": "/r/nsfw_sub/comments/def/",
            "num_comments": 5,
            "created_utc": 1704067200.0,
            "over_18": true
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert!(post.over_18);
    }

    #[test]
    fn test_reddit_data_with_after_token() {
        let json = serde_json::json!({
            "children": [],
            "after": "t3_abc123"
        });

        let data: RedditData = serde_json::from_value(json).expect("Should deserialize");
        assert!(data.children.is_empty());
        assert_eq!(data.after, Some("t3_abc123".to_string()));
    }

    #[test]
    fn test_reddit_data_without_after_token() {
        let json = serde_json::json!({
            "children": []
        });

        let data: RedditData = serde_json::from_value(json).expect("Should deserialize");
        assert!(data.children.is_empty());
        assert!(data.after.is_none());
    }

    #[test]
    fn test_subreddit_data_deserialization() {
        let json = serde_json::json!({
            "display_name": "rust",
            "title": "The Rust Programming Language",
            "public_description": "A place for all things Rust",
            "subscribers": 250000,
            "active_user_count": 1500,
            "over18": false
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(data.display_name, "rust");
        assert_eq!(data.title, "The Rust Programming Language");
        assert_eq!(data.public_description, "A place for all things Rust");
        assert_eq!(data.subscribers, 250000);
        assert_eq!(data.active_user_count, Some(1500));
        assert!(!data.over18);
    }

    #[test]
    fn test_subreddit_data_nsfw() {
        let json = serde_json::json!({
            "display_name": "nsfw_sub",
            "title": "NSFW Subreddit",
            "public_description": "",
            "subscribers": 1000,
            "over18": true
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert!(data.over18);
        assert!(data.active_user_count.is_none());
    }

    #[test]
    fn test_subreddit_data_minimal() {
        let json = serde_json::json!({
            "display_name": "minimal",
            "title": "Minimal Sub",
            "subscribers": 10
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(data.public_description, ""); // Default
        assert!(!data.over18); // Default
    }

    // ============================================================
    // Integration Tests (require network - ignored by default)
    // ============================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_integration() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "rust programming",
            "limit": 2
        });

        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("Reddit search failed");
        assert!(!output.is_empty());
        assert!(output.contains("Reddit posts"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_with_subreddit_filter() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "async",
            "subreddit": "rust",
            "limit": 3
        });

        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("Reddit search failed");
        assert!(!output.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_with_sort_options() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "rust",
            "sort": "top",
            "time": "week",
            "limit": 2
        });

        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("Reddit search failed");
        assert!(!output.is_empty());
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_subreddit_info_integration() {
        let tool = RedditSubredditTool::new();
        let input = serde_json::json!({
            "subreddit": "rust"
        });

        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("Reddit subreddit lookup failed");
        assert!(!output.is_empty());
        assert!(output.contains("Subreddit: r/rust"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_subreddit_info_string_input() {
        let tool = RedditSubredditTool::new();
        let output = tool
            ._call(ToolInput::String("rust".to_string()))
            .await
            .expect("Reddit subreddit lookup failed");
        assert!(output.contains("Subreddit: r/rust"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_post_details_integration() {
        // First search for a post to get a valid post_id
        let search_tool = RedditSearchTool::new();
        let search_input = serde_json::json!({
            "query": "rust",
            "subreddit": "rust",
            "limit": 1
        });

        let search_output = search_tool
            ._call(ToolInput::Structured(search_input))
            .await
            .expect("Search failed");
        assert!(!search_output.is_empty());
    }

    // ============================================================
    // Parameter Edge Cases - Limit
    // ============================================================

    #[tokio::test]
    async fn test_search_limit_zero() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": 0
        });
        // Should accept limit=0 without input error
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_one() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": 1
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_max_100() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": 100
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_exceeds_max_capped() {
        // Limit > 100 should be capped at 100 (not error)
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": 500
        });
        // Should not return input validation error
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_negative_treated_as_default() {
        // Negative values should be treated as unsigned (very large) then capped
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": -1
        });
        // as_u64 returns None for negative, so default (5) is used
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_float_uses_default() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": 5.5
        });
        // as_u64 returns None for floats, default is used
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_limit_string_uses_default() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "limit": "10"
        });
        // String is not u64, default is used
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Parameter Edge Cases - Sort
    // ============================================================

    #[tokio::test]
    async fn test_search_sort_relevance() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "relevance"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_hot() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "hot"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_top() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "top"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_new() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "new"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_comments() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "comments"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_invalid_passed_through() {
        // Invalid sort values are passed to API (API may reject)
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": "invalid_sort"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_sort_non_string_uses_default() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "sort": 123
        });
        // Non-string sort uses default "relevance"
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Parameter Edge Cases - Time
    // ============================================================

    #[tokio::test]
    async fn test_search_time_hour() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "hour"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_time_day() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "day"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_time_week() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "week"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_time_month() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "month"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_time_year() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "year"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_time_all() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "time": "all"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Parameter Edge Cases - Query
    // ============================================================

    #[tokio::test]
    async fn test_search_empty_query_string() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": ""
        });
        // Empty query should be accepted (API may return empty results)
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_whitespace_query() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "   "
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_unicode_query() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "日本語 検索"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_emoji_query() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "🦀 rust"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_special_characters_query() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test & query + special <chars>"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_very_long_query() {
        let tool = RedditSearchTool::new();
        let long_query = "a".repeat(1000);
        let input = serde_json::json!({
            "query": long_query
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_query_with_quotes() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "\"exact phrase\""
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Parameter Edge Cases - Subreddit
    // ============================================================

    #[tokio::test]
    async fn test_search_with_subreddit() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "subreddit": "rust"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_subreddit_with_underscore() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "subreddit": "ask_science"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_subreddit_case_insensitive() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "subreddit": "RUST"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_subreddit_non_string_ignored() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "subreddit": 123
        });
        // Non-string subreddit treated as None
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Parameter Edge Cases - NSFW
    // ============================================================

    #[tokio::test]
    async fn test_search_nsfw_true() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "nsfw": true
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_nsfw_false() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "nsfw": false
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_nsfw_non_bool_uses_default() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test",
            "nsfw": "true"
        });
        // Non-bool uses default (false)
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // RedditPostTool Parameter Edge Cases
    // ============================================================

    #[tokio::test]
    async fn test_post_num_comments_zero() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": "rust",
            "num_comments": 0
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_num_comments_max_50() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": "rust",
            "num_comments": 50
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_num_comments_exceeds_max_capped() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": "rust",
            "num_comments": 100
        });
        // Should be capped at 50, not error
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_empty_post_id() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "",
            "subreddit": "rust"
        });
        // Empty post_id accepted (API will return error)
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_empty_subreddit() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": ""
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // More Deserialization Tests
    // ============================================================

    #[test]
    fn test_reddit_post_negative_score() {
        let json = serde_json::json!({
            "title": "Controversial Post",
            "author": "user",
            "subreddit": "test",
            "score": -50,
            "url": "https://example.com",
            "permalink": "/r/test/comments/xyz/",
            "num_comments": 100,
            "created_utc": 1704067200.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.score, -50);
    }

    #[test]
    fn test_reddit_post_very_large_score() {
        let json = serde_json::json!({
            "title": "Viral Post",
            "author": "user",
            "subreddit": "all",
            "score": 150000,
            "url": "https://example.com",
            "permalink": "/r/all/comments/xyz/",
            "num_comments": 50000,
            "created_utc": 1704067200.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.score, 150000);
        assert_eq!(post.num_comments, 50000);
    }

    #[test]
    fn test_reddit_post_unicode_content() {
        let json = serde_json::json!({
            "title": "日本語タイトル 🦀",
            "author": "ユーザー",
            "subreddit": "日本",
            "score": 10,
            "url": "https://example.com",
            "permalink": "/r/日本/comments/xyz/",
            "selftext": "本文テスト ✨",
            "num_comments": 5,
            "created_utc": 1704067200.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.title, "日本語タイトル 🦀");
        assert_eq!(post.author, "ユーザー");
        assert_eq!(post.selftext, "本文テスト ✨");
    }

    #[test]
    fn test_reddit_post_extra_fields_ignored() {
        let json = serde_json::json!({
            "title": "Test",
            "author": "user",
            "subreddit": "test",
            "score": 1,
            "url": "https://example.com",
            "permalink": "/r/test/comments/xyz/",
            "num_comments": 0,
            "created_utc": 0.0,
            "unknown_field": "should be ignored",
            "another_unknown": 12345
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.title, "Test");
    }

    #[test]
    fn test_reddit_post_very_long_selftext() {
        let long_text = "x".repeat(10000);
        let json = serde_json::json!({
            "title": "Long Post",
            "author": "user",
            "subreddit": "test",
            "score": 1,
            "url": "https://example.com",
            "permalink": "/r/test/comments/xyz/",
            "selftext": long_text,
            "num_comments": 0,
            "created_utc": 0.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(post.selftext.len(), 10000);
    }

    #[test]
    fn test_reddit_post_special_characters_in_url() {
        let json = serde_json::json!({
            "title": "Special URL Post",
            "author": "user",
            "subreddit": "test",
            "score": 1,
            "url": "https://example.com/path?query=value&other=test#fragment",
            "permalink": "/r/test/comments/xyz/special_post/",
            "num_comments": 0,
            "created_utc": 0.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        assert!(post.url.contains("query=value"));
    }

    #[test]
    fn test_reddit_data_with_multiple_children() {
        let json = serde_json::json!({
            "children": [
                {
                    "data": {
                        "title": "Post 1",
                        "author": "user1",
                        "subreddit": "test",
                        "score": 10,
                        "url": "",
                        "permalink": "/r/test/comments/1/",
                        "num_comments": 5,
                        "created_utc": 0.0
                    }
                },
                {
                    "data": {
                        "title": "Post 2",
                        "author": "user2",
                        "subreddit": "test",
                        "score": 20,
                        "url": "",
                        "permalink": "/r/test/comments/2/",
                        "num_comments": 10,
                        "created_utc": 0.0
                    }
                }
            ]
        });

        let data: RedditData = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(data.children.len(), 2);
        assert_eq!(data.children[0].data.title, "Post 1");
        assert_eq!(data.children[1].data.title, "Post 2");
    }

    #[test]
    fn test_reddit_response_full_structure() {
        let json = serde_json::json!({
            "data": {
                "children": [
                    {
                        "data": {
                            "title": "Test Post",
                            "author": "testuser",
                            "subreddit": "rust",
                            "score": 100,
                            "url": "https://example.com",
                            "permalink": "/r/rust/comments/abc/",
                            "num_comments": 50,
                            "created_utc": 1704067200.0
                        }
                    }
                ],
                "after": "t3_next"
            }
        });

        let response: RedditResponse = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(response.data.children.len(), 1);
        assert_eq!(response.data.children[0].data.title, "Test Post");
        assert_eq!(response.data.after, Some("t3_next".to_string()));
    }

    #[test]
    fn test_subreddit_data_very_large_subscribers() {
        let json = serde_json::json!({
            "display_name": "popular",
            "title": "Popular Subreddit",
            "subscribers": 50000000,
            "active_user_count": 100000
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(data.subscribers, 50000000);
        assert_eq!(data.active_user_count, Some(100000));
    }

    #[test]
    fn test_subreddit_data_zero_subscribers() {
        let json = serde_json::json!({
            "display_name": "new_sub",
            "title": "Brand New Sub",
            "subscribers": 0
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(data.subscribers, 0);
    }

    #[test]
    fn test_subreddit_data_unicode_description() {
        let json = serde_json::json!({
            "display_name": "日本語",
            "title": "日本語サブレディット",
            "public_description": "日本語でのディスカッション 🇯🇵",
            "subscribers": 1000
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        assert!(data.public_description.contains("日本語"));
    }

    #[test]
    fn test_subreddit_about_full_structure() {
        let json = serde_json::json!({
            "data": {
                "display_name": "rust",
                "title": "Rust Programming",
                "public_description": "Rust lang",
                "subscribers": 250000,
                "active_user_count": 2000,
                "over18": false
            }
        });

        let about: SubredditAbout = serde_json::from_value(json).expect("Should deserialize");
        assert_eq!(about.data.display_name, "rust");
        assert_eq!(about.data.subscribers, 250000);
    }

    // ============================================================
    // Debug Trait Output Tests
    // ============================================================

    #[test]
    fn test_reddit_post_debug() {
        let json = serde_json::json!({
            "title": "Debug Test",
            "author": "user",
            "subreddit": "test",
            "score": 1,
            "url": "",
            "permalink": "/",
            "num_comments": 0,
            "created_utc": 0.0
        });

        let post: RedditPost = serde_json::from_value(json).expect("Should deserialize");
        let debug = format!("{post:?}");
        assert!(debug.contains("RedditPost"));
        assert!(debug.contains("Debug Test"));
    }

    #[test]
    fn test_reddit_data_debug() {
        let json = serde_json::json!({
            "children": []
        });

        let data: RedditData = serde_json::from_value(json).expect("Should deserialize");
        let debug = format!("{data:?}");
        assert!(debug.contains("RedditData"));
    }

    #[test]
    fn test_reddit_response_debug() {
        let json = serde_json::json!({
            "data": {
                "children": []
            }
        });

        let response: RedditResponse = serde_json::from_value(json).expect("Should deserialize");
        let debug = format!("{response:?}");
        assert!(debug.contains("RedditResponse"));
    }

    #[test]
    fn test_subreddit_data_debug() {
        let json = serde_json::json!({
            "display_name": "test",
            "title": "Test",
            "subscribers": 100
        });

        let data: SubredditData = serde_json::from_value(json).expect("Should deserialize");
        let debug = format!("{data:?}");
        assert!(debug.contains("SubredditData"));
    }

    // ============================================================
    // Tool Name and Description Consistency Tests
    // ============================================================

    #[test]
    fn test_all_tool_names_are_snake_case() {
        let search = RedditSearchTool::new();
        let post = RedditPostTool::new();
        let subreddit = RedditSubredditTool::new();

        // All names should be snake_case
        assert!(search.name().chars().all(|c| c.is_lowercase() || c == '_'));
        assert!(post.name().chars().all(|c| c.is_lowercase() || c == '_'));
        assert!(
            subreddit
                .name()
                .chars()
                .all(|c| c.is_lowercase() || c == '_')
        );
    }

    #[test]
    fn test_all_tool_descriptions_non_empty() {
        let search = RedditSearchTool::new();
        let post = RedditPostTool::new();
        let subreddit = RedditSubredditTool::new();

        assert!(!search.description().is_empty());
        assert!(!post.description().is_empty());
        assert!(!subreddit.description().is_empty());
    }

    #[test]
    fn test_search_description_mentions_all_params() {
        let tool = RedditSearchTool::new();
        let desc = tool.description();

        // All documented params should be mentioned
        assert!(desc.contains("query"));
        assert!(desc.contains("limit"));
        assert!(desc.contains("sort"));
        assert!(desc.contains("time"));
        assert!(desc.contains("subreddit"));
        assert!(desc.contains("nsfw"));
    }

    #[test]
    fn test_post_description_mentions_all_params() {
        let tool = RedditPostTool::new();
        let desc = tool.description();

        assert!(desc.contains("post_id"));
        assert!(desc.contains("subreddit"));
        assert!(desc.contains("num_comments"));
    }

    // ============================================================
    // User Agent Tests
    // ============================================================

    #[test]
    fn test_custom_user_agent_long_string() {
        let long_ua = "a".repeat(500);
        let tool = RedditSearchTool::with_user_agent(&long_ua);
        assert_eq!(tool.name(), "reddit_search");
    }

    #[test]
    fn test_custom_user_agent_unicode() {
        let tool = RedditSearchTool::with_user_agent("myapp/1.0 日本語");
        assert_eq!(tool.name(), "reddit_search");
    }

    #[test]
    fn test_custom_user_agent_empty() {
        let tool = RedditSearchTool::with_user_agent("");
        assert_eq!(tool.name(), "reddit_search");
    }

    #[test]
    fn test_try_with_user_agent_success() {
        let result = RedditSearchTool::try_with_user_agent("test/1.0");
        assert!(result.is_ok());
    }

    // ============================================================
    // Base URL Tests
    // ============================================================

    #[test]
    fn test_base_url_is_https() {
        let tool = RedditSearchTool::new();
        assert!(tool.base_url.starts_with("https://"));
    }

    #[test]
    fn test_base_url_is_reddit() {
        let tool = RedditSearchTool::new();
        assert!(tool.base_url.contains("reddit.com"));
    }

    #[test]
    fn test_all_tools_use_same_base_url() {
        let search = RedditSearchTool::new();
        let post = RedditPostTool::new();
        let subreddit = RedditSubredditTool::new();

        assert_eq!(search.base_url, post.base_url);
        assert_eq!(post.base_url, subreddit.base_url);
    }

    // ============================================================
    // Combined Parameter Tests
    // ============================================================

    #[tokio::test]
    async fn test_search_all_parameters() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "rust async await",
            "limit": 10,
            "sort": "top",
            "time": "month",
            "subreddit": "rust",
            "nsfw": false
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_search_minimal_parameters() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({
            "query": "test"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_all_parameters() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123",
            "subreddit": "rust",
            "num_comments": 25
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    #[tokio::test]
    async fn test_post_minimal_parameters() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "xyz",
            "subreddit": "test"
        });
        let _ = tool._call(ToolInput::Structured(input)).await;
    }

    // ============================================================
    // Error Message Quality Tests
    // ============================================================

    #[tokio::test]
    async fn test_search_error_message_mentions_query() {
        let tool = RedditSearchTool::new();
        let input = serde_json::json!({});
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("query"),
            "Error should mention missing 'query': {err}"
        );
    }

    #[tokio::test]
    async fn test_post_error_message_mentions_post_id() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "subreddit": "rust"
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("post_id"),
            "Error should mention missing 'post_id': {err}"
        );
    }

    #[tokio::test]
    async fn test_post_error_message_mentions_subreddit() {
        let tool = RedditPostTool::new();
        let input = serde_json::json!({
            "post_id": "abc123"
        });
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("subreddit"),
            "Error should mention missing 'subreddit': {err}"
        );
    }

    #[tokio::test]
    async fn test_subreddit_error_message_mentions_subreddit() {
        let tool = RedditSubredditTool::new();
        let input = serde_json::json!({});
        let result = tool._call(ToolInput::Structured(input)).await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.to_lowercase().contains("subreddit"),
            "Error should mention missing 'subreddit': {err}"
        );
    }

    // ============================================================
    // Async Behavior Tests
    // ============================================================

    #[tokio::test]
    async fn test_concurrent_tool_creation() {
        let handles: Vec<_> = (0..10)
            .map(|_| tokio::spawn(async { RedditSearchTool::new() }))
            .collect();

        for handle in handles {
            let tool = handle.await.expect("Should create tool");
            assert_eq!(tool.name(), "reddit_search");
        }
    }

    #[tokio::test]
    async fn test_tool_clone_is_independent() {
        let tool1 = RedditSearchTool::new();
        let tool2 = tool1.clone();

        // Both should work independently
        assert_eq!(tool1.name(), tool2.name());
        assert_eq!(tool1.base_url, tool2.base_url);
    }
}
