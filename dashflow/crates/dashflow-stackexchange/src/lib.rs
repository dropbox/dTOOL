//! Stack Exchange API integration for `DashFlow` Rust.
//!
//! This crate provides tools for interacting with the Stack Exchange API,
//! allowing you to search questions, get answers, and retrieve user information
//! from Stack Overflow and other Stack Exchange sites.
//!
//! # Stack Exchange API
//!
//! The Stack Exchange API provides access to questions, answers, comments, and
//! user data across the Stack Exchange network of Q&A sites. No authentication
//! is required for basic read operations.
//!
//! API Documentation: <https://api.stackexchange.com/docs>
//!
//! # Available Tools
//!
//! - **`StackExchangeSearchTool`**: Search for questions on a Stack Exchange site
//! - **`StackExchangeQuestionTool`**: Get detailed information about a specific question
//! - **`StackExchangeUserTool`**: Get information about a Stack Exchange user
//!
//! # Example
//!
//! ```ignore
//! use dashflow_stackexchange::StackExchangeSearchTool;
//! use dashflow::core::tools::{Tool, ToolInput};
//! use std::collections::HashMap;
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
//!
//!     let mut input = HashMap::new();
//!     input.insert("query".to_string(), "rust async trait".to_string());
//!     input.insert("max_results".to_string(), "5".to_string());
//!
//!     let result = tool._call(ToolInput::Map(input)).await?;
//!     println!("Search results: {}", result);
//!
//!     Ok(())
//! }
//! ```

use async_trait::async_trait;
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::{Error, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Stack Exchange API base URL
const STACKEXCHANGE_API_BASE: &str = "https://api.stackexchange.com/2.3";

/// Helper function to extract string value from `ToolInput`
fn get_string_input(input: &ToolInput, key: &str) -> Result<String> {
    match input {
        ToolInput::Structured(map) => map
            .get(key)
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::tool_error(format!("Missing required field: {key}")))
            .map(std::string::ToString::to_string),
        ToolInput::String(s) => {
            if key == "query" {
                Ok(s.clone())
            } else {
                Err(Error::tool_error(format!(
                    "Expected structured input with key: {key}"
                )))
            }
        }
    }
}

/// Helper function to get optional string input
fn get_optional_string_input(input: &ToolInput, key: &str) -> Option<String> {
    match input {
        ToolInput::Structured(map) => map
            .get(key)
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string),
        _ => None,
    }
}

/// Stack Exchange API question response
#[derive(Debug, Deserialize, Serialize)]
struct Question {
    question_id: u64,
    title: String,
    #[serde(default)]
    body: Option<String>,
    link: String,
    score: i32,
    view_count: u64,
    answer_count: u64,
    is_answered: bool,
    #[serde(default)]
    tags: Vec<String>,
    creation_date: u64,
    #[serde(default)]
    accepted_answer_id: Option<u64>,
    owner: Owner,
}

/// Stack Exchange API owner (user) information
#[derive(Debug, Deserialize, Serialize)]
struct Owner {
    #[serde(default)]
    user_id: Option<u64>,
    display_name: String,
    #[serde(default)]
    reputation: Option<u64>,
    #[serde(default)]
    link: Option<String>,
}

/// Stack Exchange API response wrapper
#[derive(Debug, Deserialize)]
struct StackExchangeResponse<T> {
    items: Vec<T>,
    /// Pagination flag from Stack Exchange API indicating more results available
    #[allow(dead_code)] // Deserialize: Stack Exchange pagination - reserved for multi-page fetches
    has_more: bool,
    #[serde(default)]
    quota_remaining: Option<u64>,
}

/// Stack Exchange API user response
#[derive(Debug, Deserialize, Serialize)]
struct User {
    user_id: u64,
    display_name: String,
    reputation: u64,
    link: String,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    badge_counts: Option<BadgeCounts>,
    creation_date: u64,
}

/// Badge counts for a Stack Exchange user
#[derive(Debug, Deserialize, Serialize)]
struct BadgeCounts {
    bronze: u64,
    silver: u64,
    gold: u64,
}

/// Tool for searching questions on Stack Exchange sites.
///
/// This tool searches for questions matching a query string on a specified
/// Stack Exchange site (e.g., stackoverflow, serverfault, superuser).
///
/// # Input Parameters
///
/// - `query` (required): The search query string
/// - `max_results` (optional): Maximum number of results to return (default: 5)
/// - `sort` (optional): Sort order - "relevance", "votes", "activity", "creation" (default: "relevance")
///
/// # Example
///
/// ```ignore
/// use dashflow_stackexchange::StackExchangeSearchTool;
/// use dashflow::core::tools::{Tool, ToolInput};
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
///
///     let mut input = HashMap::new();
///     input.insert("query".to_string(), "rust borrowing".to_string());
///     input.insert("max_results".to_string(), "3".to_string());
///
///     let result = tool._call(ToolInput::Map(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StackExchangeSearchTool {
    client: Client,
    site: String,
}

impl StackExchangeSearchTool {
    /// Create a new `StackExchangeSearchTool`.
    ///
    /// # Arguments
    ///
    /// * `site` - The Stack Exchange site to search (e.g., "stackoverflow", "serverfault")
    #[must_use]
    pub fn new(site: String) -> Self {
        Self {
            client: Client::new(),
            site,
        }
    }
}

#[async_trait]
impl Tool for StackExchangeSearchTool {
    fn name(&self) -> &'static str {
        "stack_exchange_search"
    }

    fn description(&self) -> &'static str {
        "Search for questions on Stack Exchange sites (e.g., Stack Overflow). \
         Input should be a map with 'query' (required), 'max_results' (optional, default 5), \
         and 'sort' (optional: relevance/votes/activity/creation). \
         Returns titles, links, scores, and answer counts."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let query = get_string_input(&input, "query")?;
        let max_results = get_optional_string_input(&input, "max_results")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(5);
        let sort =
            get_optional_string_input(&input, "sort").unwrap_or_else(|| "relevance".to_string());

        let url = format!(
            "{}/search/advanced?order=desc&sort={}&q={}&site={}&pagesize={}",
            STACKEXCHANGE_API_BASE,
            urlencoding::encode(&sort),
            urlencoding::encode(&query),
            urlencoding::encode(&self.site),
            max_results
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                Error::tool_error(format!("Failed to call Stack Exchange API: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Stack Exchange API returned error: {}",
                response.status()
            )));
        }

        let se_response: StackExchangeResponse<Question> = response.json().await.map_err(|e| {
            Error::tool_error(format!("Failed to parse Stack Exchange response: {e}"))
        })?;

        if se_response.items.is_empty() {
            return Ok("No results found.".to_string());
        }

        let mut results = Vec::new();
        results.push(format!(
            "Found {} questions on {}:",
            se_response.items.len(),
            self.site
        ));
        results.push(String::new());

        for (idx, question) in se_response.items.iter().enumerate() {
            let answered_status = if question.is_answered {
                "✓ Answered"
            } else {
                "○ No accepted answer"
            };

            results.push(format!("{}. {}", idx + 1, question.title));
            results.push(format!(
                "   Score: {} | Answers: {} | Views: {} | {}",
                question.score, question.answer_count, question.view_count, answered_status
            ));
            results.push(format!("   Tags: {}", question.tags.join(", ")));
            results.push(format!("   Link: {}", question.link));
            results.push(String::new());
        }

        if let Some(quota) = se_response.quota_remaining {
            results.push(format!("API quota remaining: {quota}"));
        }

        Ok(results.join("\n"))
    }
}

/// Tool for getting detailed information about a specific Stack Exchange question.
///
/// This tool retrieves full details about a question including its body, answers,
/// and comments.
///
/// # Input Parameters
///
/// - `question_id` (required): The Stack Exchange question ID
/// - `include_answers` (optional): Whether to include answers (default: true)
///
/// # Example
///
/// ```ignore
/// use dashflow_stackexchange::StackExchangeQuestionTool;
/// use dashflow::core::tools::{Tool, ToolInput};
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());
///
///     let mut input = HashMap::new();
///     input.insert("question_id".to_string(), "12345678".to_string());
///
///     let result = tool._call(ToolInput::Map(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StackExchangeQuestionTool {
    client: Client,
    site: String,
}

impl StackExchangeQuestionTool {
    /// Create a new `StackExchangeQuestionTool`.
    ///
    /// # Arguments
    ///
    /// * `site` - The Stack Exchange site (e.g., "stackoverflow", "serverfault")
    #[must_use]
    pub fn new(site: String) -> Self {
        Self {
            client: Client::new(),
            site,
        }
    }
}

#[async_trait]
impl Tool for StackExchangeQuestionTool {
    fn name(&self) -> &'static str {
        "stack_exchange_question"
    }

    fn description(&self) -> &'static str {
        "Get detailed information about a specific Stack Exchange question by ID. \
         Input should be a map with 'question_id' (required) and 'include_answers' (optional, default true). \
         Returns question details including title, body, score, answers, and link."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let question_id = get_string_input(&input, "question_id")?;
        let include_answers = get_optional_string_input(&input, "include_answers")
            .and_then(|s| s.parse::<bool>().ok())
            .unwrap_or(true);

        let filter = if include_answers {
            "withbody"
        } else {
            "default"
        };

        let url = format!(
            "{}/questions/{}?order=desc&sort=activity&site={}&filter={}",
            STACKEXCHANGE_API_BASE,
            question_id,
            urlencoding::encode(&self.site),
            filter
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                Error::tool_error(format!("Failed to call Stack Exchange API: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Stack Exchange API returned error: {}",
                response.status()
            )));
        }

        let se_response: StackExchangeResponse<Question> = response.json().await.map_err(|e| {
            Error::tool_error(format!("Failed to parse Stack Exchange response: {e}"))
        })?;

        if se_response.items.is_empty() {
            return Ok(format!(
                "Question {} not found on {}.",
                question_id, self.site
            ));
        }

        let question = &se_response.items[0];
        let mut results = Vec::new();

        results.push(format!("Question ID: {}", question.question_id));
        results.push(format!("Title: {}", question.title));
        results.push(format!(
            "Score: {} | Answers: {} | Views: {}",
            question.score, question.answer_count, question.view_count
        ));
        results.push(format!("Tags: {}", question.tags.join(", ")));
        results.push(format!("Asked by: {}", question.owner.display_name));

        if let Some(reputation) = question.owner.reputation {
            results.push(format!("   Reputation: {reputation}"));
        }

        results.push(String::new());

        if let Some(body) = &question.body {
            results.push("Body:".to_string());
            results.push(body.clone());
            results.push(String::new());
        }

        let answered_status = if question.is_answered { "Yes" } else { "No" };
        results.push(format!("Has accepted answer: {answered_status}"));

        if let Some(accepted_id) = question.accepted_answer_id {
            results.push(format!("Accepted answer ID: {accepted_id}"));
        }

        results.push(format!("Link: {}", question.link));

        Ok(results.join("\n"))
    }
}

/// Tool for getting information about a Stack Exchange user.
///
/// This tool retrieves user profile information including reputation, badges,
/// and account details.
///
/// # Input Parameters
///
/// - `user_id` (required): The Stack Exchange user ID
///
/// # Example
///
/// ```ignore
/// use dashflow_stackexchange::StackExchangeUserTool;
/// use dashflow::core::tools::{Tool, ToolInput};
/// use std::collections::HashMap;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let tool = StackExchangeUserTool::new("stackoverflow".to_string());
///
///     let mut input = HashMap::new();
///     input.insert("user_id".to_string(), "123456".to_string());
///
///     let result = tool._call(ToolInput::Map(input)).await?;
///     println!("{}", result);
///
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct StackExchangeUserTool {
    client: Client,
    site: String,
}

impl StackExchangeUserTool {
    /// Create a new `StackExchangeUserTool`.
    ///
    /// # Arguments
    ///
    /// * `site` - The Stack Exchange site (e.g., "stackoverflow", "serverfault")
    #[must_use]
    pub fn new(site: String) -> Self {
        Self {
            client: Client::new(),
            site,
        }
    }
}

#[async_trait]
impl Tool for StackExchangeUserTool {
    fn name(&self) -> &'static str {
        "stack_exchange_user"
    }

    fn description(&self) -> &'static str {
        "Get information about a Stack Exchange user by ID. \
         Input should be a map with 'user_id' (required). \
         Returns user profile including reputation, badges, location, and profile link."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let user_id = get_string_input(&input, "user_id")?;

        let url = format!(
            "{}/users/{}?order=desc&sort=reputation&site={}",
            STACKEXCHANGE_API_BASE,
            user_id,
            urlencoding::encode(&self.site)
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                Error::tool_error(format!("Failed to call Stack Exchange API: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(Error::tool_error(format!(
                "Stack Exchange API returned error: {}",
                response.status()
            )));
        }

        let se_response: StackExchangeResponse<User> = response.json().await.map_err(|e| {
            Error::tool_error(format!("Failed to parse Stack Exchange response: {e}"))
        })?;

        if se_response.items.is_empty() {
            return Ok(format!("User {} not found on {}.", user_id, self.site));
        }

        let user = &se_response.items[0];
        let mut results = Vec::new();

        results.push(format!("User ID: {}", user.user_id));
        results.push(format!("Display Name: {}", user.display_name));
        results.push(format!("Reputation: {}", user.reputation));

        if let Some(location) = &user.location {
            results.push(format!("Location: {location}"));
        }

        if let Some(badges) = &user.badge_counts {
            results.push(format!(
                "Badges: {} gold, {} silver, {} bronze",
                badges.gold, badges.silver, badges.bronze
            ));
        }

        results.push(format!("Profile: {}", user.link));

        Ok(results.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ========================================================================
    // HELPER FUNCTION TESTS
    // ========================================================================

    #[test]
    fn test_get_string_input_structured_with_key() {
        let input = ToolInput::Structured(json!({"query": "rust async"}));
        let result = get_string_input(&input, "query").unwrap();
        assert_eq!(result, "rust async");
    }

    #[test]
    fn test_get_string_input_structured_missing_key() {
        let input = ToolInput::Structured(json!({"other": "value"}));
        let result = get_string_input(&input, "query");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("query"));
    }

    #[test]
    fn test_get_string_input_structured_null_value() {
        let input = ToolInput::Structured(json!({"query": null}));
        let result = get_string_input(&input, "query");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_string_input_structured_numeric_value() {
        let input = ToolInput::Structured(json!({"query": 123}));
        let result = get_string_input(&input, "query");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_string_input_string_for_query() {
        let input = ToolInput::String("rust borrowing".to_string());
        let result = get_string_input(&input, "query").unwrap();
        assert_eq!(result, "rust borrowing");
    }

    #[test]
    fn test_get_string_input_string_for_non_query_key() {
        let input = ToolInput::String("some value".to_string());
        let result = get_string_input(&input, "other_key");
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("other_key"));
    }

    #[test]
    fn test_get_string_input_structured_empty_string() {
        let input = ToolInput::Structured(json!({"query": ""}));
        let result = get_string_input(&input, "query").unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_get_string_input_structured_with_special_chars() {
        let input = ToolInput::Structured(json!({"query": "rust<script>alert()</script>"}));
        let result = get_string_input(&input, "query").unwrap();
        assert_eq!(result, "rust<script>alert()</script>");
    }

    #[test]
    fn test_get_string_input_structured_with_unicode() {
        let input = ToolInput::Structured(json!({"query": "rust 日本語"}));
        let result = get_string_input(&input, "query").unwrap();
        assert_eq!(result, "rust 日本語");
    }

    #[test]
    fn test_get_optional_string_input_present() {
        let input = ToolInput::Structured(json!({"sort": "votes"}));
        let result = get_optional_string_input(&input, "sort");
        assert_eq!(result, Some("votes".to_string()));
    }

    #[test]
    fn test_get_optional_string_input_missing() {
        let input = ToolInput::Structured(json!({"other": "value"}));
        let result = get_optional_string_input(&input, "sort");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_optional_string_input_null_value() {
        let input = ToolInput::Structured(json!({"sort": null}));
        let result = get_optional_string_input(&input, "sort");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_optional_string_input_numeric_value() {
        let input = ToolInput::Structured(json!({"max_results": 5}));
        let result = get_optional_string_input(&input, "max_results");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_optional_string_input_from_string_input() {
        let input = ToolInput::String("some query".to_string());
        let result = get_optional_string_input(&input, "any_key");
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_optional_string_input_empty_string() {
        let input = ToolInput::Structured(json!({"sort": ""}));
        let result = get_optional_string_input(&input, "sort");
        assert_eq!(result, Some(String::new()));
    }

    // ========================================================================
    // DATA STRUCTURE SERIALIZATION/DESERIALIZATION TESTS
    // ========================================================================

    #[test]
    fn test_question_deserialize_full() {
        let json_data = json!({
            "question_id": 12345,
            "title": "How to async in Rust?",
            "body": "<p>Question body</p>",
            "link": "https://stackoverflow.com/q/12345",
            "score": 42,
            "view_count": 1000,
            "answer_count": 3,
            "is_answered": true,
            "tags": ["rust", "async", "tokio"],
            "creation_date": 1609459200,
            "accepted_answer_id": 67890,
            "owner": {
                "user_id": 111,
                "display_name": "RustDev",
                "reputation": 5000,
                "link": "https://stackoverflow.com/users/111"
            }
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert_eq!(question.question_id, 12345);
        assert_eq!(question.title, "How to async in Rust?");
        assert_eq!(question.body, Some("<p>Question body</p>".to_string()));
        assert_eq!(question.link, "https://stackoverflow.com/q/12345");
        assert_eq!(question.score, 42);
        assert_eq!(question.view_count, 1000);
        assert_eq!(question.answer_count, 3);
        assert!(question.is_answered);
        assert_eq!(question.tags, vec!["rust", "async", "tokio"]);
        assert_eq!(question.creation_date, 1609459200);
        assert_eq!(question.accepted_answer_id, Some(67890));
        assert_eq!(question.owner.display_name, "RustDev");
    }

    #[test]
    fn test_question_deserialize_minimal() {
        let json_data = json!({
            "question_id": 1,
            "title": "Test",
            "link": "https://example.com",
            "score": 0,
            "view_count": 0,
            "answer_count": 0,
            "is_answered": false,
            "creation_date": 0,
            "owner": {
                "display_name": "Anonymous"
            }
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert_eq!(question.question_id, 1);
        assert_eq!(question.body, None);
        assert!(question.tags.is_empty());
        assert_eq!(question.accepted_answer_id, None);
    }

    #[test]
    fn test_question_deserialize_negative_score() {
        let json_data = json!({
            "question_id": 1,
            "title": "Bad Question",
            "link": "https://example.com",
            "score": -5,
            "view_count": 100,
            "answer_count": 0,
            "is_answered": false,
            "creation_date": 0,
            "owner": {
                "display_name": "User"
            }
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert_eq!(question.score, -5);
    }

    #[test]
    fn test_owner_deserialize_full() {
        let json_data = json!({
            "user_id": 123,
            "display_name": "TestUser",
            "reputation": 10000,
            "link": "https://stackoverflow.com/users/123"
        });

        let owner: Owner = serde_json::from_value(json_data).unwrap();
        assert_eq!(owner.user_id, Some(123));
        assert_eq!(owner.display_name, "TestUser");
        assert_eq!(owner.reputation, Some(10000));
        assert_eq!(owner.link, Some("https://stackoverflow.com/users/123".to_string()));
    }

    #[test]
    fn test_owner_deserialize_minimal() {
        let json_data = json!({
            "display_name": "AnonymousUser"
        });

        let owner: Owner = serde_json::from_value(json_data).unwrap();
        assert_eq!(owner.user_id, None);
        assert_eq!(owner.display_name, "AnonymousUser");
        assert_eq!(owner.reputation, None);
        assert_eq!(owner.link, None);
    }

    #[test]
    fn test_stackexchange_response_deserialize() {
        let json_data = json!({
            "items": [
                {
                    "question_id": 1,
                    "title": "Q1",
                    "link": "https://example.com/1",
                    "score": 10,
                    "view_count": 100,
                    "answer_count": 2,
                    "is_answered": true,
                    "creation_date": 0,
                    "owner": {"display_name": "User1"}
                }
            ],
            "has_more": true,
            "quota_remaining": 299
        });

        let response: StackExchangeResponse<Question> = serde_json::from_value(json_data).unwrap();
        assert_eq!(response.items.len(), 1);
        assert!(response.has_more);
        assert_eq!(response.quota_remaining, Some(299));
    }

    #[test]
    fn test_stackexchange_response_empty_items() {
        let json_data = json!({
            "items": [],
            "has_more": false
        });

        let response: StackExchangeResponse<Question> = serde_json::from_value(json_data).unwrap();
        assert!(response.items.is_empty());
        assert!(!response.has_more);
        assert_eq!(response.quota_remaining, None);
    }

    #[test]
    fn test_user_deserialize_full() {
        let json_data = json!({
            "user_id": 12345,
            "display_name": "Jon Skeet",
            "reputation": 1400000,
            "link": "https://stackoverflow.com/users/12345",
            "location": "Reading, UK",
            "badge_counts": {
                "bronze": 9000,
                "silver": 8000,
                "gold": 800
            },
            "creation_date": 1222387200
        });

        let user: User = serde_json::from_value(json_data).unwrap();
        assert_eq!(user.user_id, 12345);
        assert_eq!(user.display_name, "Jon Skeet");
        assert_eq!(user.reputation, 1400000);
        assert_eq!(user.location, Some("Reading, UK".to_string()));
        let badges = user.badge_counts.unwrap();
        assert_eq!(badges.gold, 800);
        assert_eq!(badges.silver, 8000);
        assert_eq!(badges.bronze, 9000);
    }

    #[test]
    fn test_user_deserialize_minimal() {
        let json_data = json!({
            "user_id": 1,
            "display_name": "NewUser",
            "reputation": 1,
            "link": "https://stackoverflow.com/users/1",
            "creation_date": 0
        });

        let user: User = serde_json::from_value(json_data).unwrap();
        assert_eq!(user.user_id, 1);
        assert_eq!(user.reputation, 1);
        assert_eq!(user.location, None);
        assert!(user.badge_counts.is_none());
    }

    #[test]
    fn test_badge_counts_deserialize() {
        let json_data = json!({
            "bronze": 100,
            "silver": 50,
            "gold": 10
        });

        let badges: BadgeCounts = serde_json::from_value(json_data).unwrap();
        assert_eq!(badges.bronze, 100);
        assert_eq!(badges.silver, 50);
        assert_eq!(badges.gold, 10);
    }

    #[test]
    fn test_badge_counts_all_zero() {
        let json_data = json!({
            "bronze": 0,
            "silver": 0,
            "gold": 0
        });

        let badges: BadgeCounts = serde_json::from_value(json_data).unwrap();
        assert_eq!(badges.bronze, 0);
        assert_eq!(badges.silver, 0);
        assert_eq!(badges.gold, 0);
    }

    // ========================================================================
    // TOOL CONSTRUCTOR TESTS
    // ========================================================================

    #[test]
    fn test_search_tool_new() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        assert_eq!(tool.site, "stackoverflow");
    }

    #[test]
    fn test_search_tool_new_different_site() {
        let tool = StackExchangeSearchTool::new("serverfault".to_string());
        assert_eq!(tool.site, "serverfault");
    }

    #[test]
    fn test_search_tool_new_superuser() {
        let tool = StackExchangeSearchTool::new("superuser".to_string());
        assert_eq!(tool.site, "superuser");
    }

    #[test]
    fn test_question_tool_new() {
        let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());
        assert_eq!(tool.site, "stackoverflow");
    }

    #[test]
    fn test_question_tool_new_askubuntu() {
        let tool = StackExchangeQuestionTool::new("askubuntu".to_string());
        assert_eq!(tool.site, "askubuntu");
    }

    #[test]
    fn test_user_tool_new() {
        let tool = StackExchangeUserTool::new("stackoverflow".to_string());
        assert_eq!(tool.site, "stackoverflow");
    }

    #[test]
    fn test_user_tool_new_mathoverflow() {
        let tool = StackExchangeUserTool::new("mathoverflow".to_string());
        assert_eq!(tool.site, "mathoverflow");
    }

    // ========================================================================
    // TOOL TRAIT IMPLEMENTATION TESTS
    // ========================================================================

    #[test]
    fn test_search_tool_name() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        assert_eq!(tool.name(), "stack_exchange_search");
    }

    #[test]
    fn test_search_tool_description() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Search"));
        assert!(desc.contains("Stack Exchange"));
    }

    #[test]
    fn test_question_tool_name() {
        let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());
        assert_eq!(tool.name(), "stack_exchange_question");
    }

    #[test]
    fn test_question_tool_description() {
        let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("question"));
        assert!(desc.contains("question_id"));
    }

    #[test]
    fn test_user_tool_name() {
        let tool = StackExchangeUserTool::new("stackoverflow".to_string());
        assert_eq!(tool.name(), "stack_exchange_user");
    }

    #[test]
    fn test_user_tool_description() {
        let tool = StackExchangeUserTool::new("stackoverflow".to_string());
        let desc = tool.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("user"));
        assert!(desc.contains("user_id"));
    }

    // ========================================================================
    // CLONE AND DEBUG TRAIT TESTS
    // ========================================================================

    #[test]
    fn test_search_tool_clone() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        let cloned = tool.clone();
        assert_eq!(tool.site, cloned.site);
    }

    #[test]
    fn test_search_tool_debug() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("StackExchangeSearchTool"));
        assert!(debug_str.contains("stackoverflow"));
    }

    #[test]
    fn test_question_tool_clone() {
        let tool = StackExchangeQuestionTool::new("askubuntu".to_string());
        let cloned = tool.clone();
        assert_eq!(tool.site, cloned.site);
    }

    #[test]
    fn test_question_tool_debug() {
        let tool = StackExchangeQuestionTool::new("serverfault".to_string());
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("StackExchangeQuestionTool"));
        assert!(debug_str.contains("serverfault"));
    }

    #[test]
    fn test_user_tool_clone() {
        let tool = StackExchangeUserTool::new("superuser".to_string());
        let cloned = tool.clone();
        assert_eq!(tool.site, cloned.site);
    }

    #[test]
    fn test_user_tool_debug() {
        let tool = StackExchangeUserTool::new("unix".to_string());
        let debug_str = format!("{:?}", tool);
        assert!(debug_str.contains("StackExchangeUserTool"));
        assert!(debug_str.contains("unix"));
    }

    // ========================================================================
    // SERIALIZATION TESTS (Question, Owner, User, BadgeCounts)
    // ========================================================================

    #[test]
    fn test_question_serialize() {
        let question = Question {
            question_id: 1,
            title: "Test".to_string(),
            body: Some("Body".to_string()),
            link: "https://example.com".to_string(),
            score: 5,
            view_count: 100,
            answer_count: 2,
            is_answered: true,
            tags: vec!["rust".to_string()],
            creation_date: 1609459200,
            accepted_answer_id: Some(10),
            owner: Owner {
                user_id: Some(1),
                display_name: "Test".to_string(),
                reputation: Some(100),
                link: Some("https://example.com/user".to_string()),
            },
        };

        let json_str = serde_json::to_string(&question).unwrap();
        assert!(json_str.contains("\"question_id\":1"));
        assert!(json_str.contains("\"title\":\"Test\""));
    }

    #[test]
    fn test_owner_serialize() {
        let owner = Owner {
            user_id: Some(123),
            display_name: "TestUser".to_string(),
            reputation: Some(5000),
            link: Some("https://example.com".to_string()),
        };

        let json_str = serde_json::to_string(&owner).unwrap();
        assert!(json_str.contains("\"user_id\":123"));
        assert!(json_str.contains("\"display_name\":\"TestUser\""));
    }

    #[test]
    fn test_user_serialize() {
        let user = User {
            user_id: 456,
            display_name: "Serializer".to_string(),
            reputation: 9999,
            link: "https://example.com/user".to_string(),
            location: Some("Earth".to_string()),
            badge_counts: Some(BadgeCounts {
                bronze: 10,
                silver: 5,
                gold: 1,
            }),
            creation_date: 1609459200,
        };

        let json_str = serde_json::to_string(&user).unwrap();
        assert!(json_str.contains("\"user_id\":456"));
        assert!(json_str.contains("\"reputation\":9999"));
    }

    #[test]
    fn test_badge_counts_serialize() {
        let badges = BadgeCounts {
            bronze: 25,
            silver: 10,
            gold: 2,
        };

        let json_str = serde_json::to_string(&badges).unwrap();
        assert!(json_str.contains("\"bronze\":25"));
        assert!(json_str.contains("\"silver\":10"));
        assert!(json_str.contains("\"gold\":2"));
    }

    // ========================================================================
    // API CONSTANT TESTS
    // ========================================================================

    #[test]
    fn test_api_base_url_format() {
        assert!(STACKEXCHANGE_API_BASE.starts_with("https://"));
        assert!(STACKEXCHANGE_API_BASE.contains("stackexchange.com"));
        assert!(STACKEXCHANGE_API_BASE.contains("2.3"));
    }

    #[test]
    fn test_api_base_url_no_trailing_slash() {
        assert!(!STACKEXCHANGE_API_BASE.ends_with('/'));
    }

    // ========================================================================
    // EDGE CASE TESTS
    // ========================================================================

    #[test]
    fn test_question_with_empty_tags() {
        let json_data = json!({
            "question_id": 1,
            "title": "No tags",
            "link": "https://example.com",
            "score": 0,
            "view_count": 0,
            "answer_count": 0,
            "is_answered": false,
            "tags": [],
            "creation_date": 0,
            "owner": {"display_name": "User"}
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert!(question.tags.is_empty());
    }

    #[test]
    fn test_question_with_many_tags() {
        let json_data = json!({
            "question_id": 1,
            "title": "Many tags",
            "link": "https://example.com",
            "score": 0,
            "view_count": 0,
            "answer_count": 0,
            "is_answered": false,
            "tags": ["rust", "async", "tokio", "futures", "concurrency"],
            "creation_date": 0,
            "owner": {"display_name": "User"}
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert_eq!(question.tags.len(), 5);
    }

    #[test]
    fn test_user_response_with_multiple_users() {
        let json_data = json!({
            "items": [
                {
                    "user_id": 1,
                    "display_name": "User1",
                    "reputation": 100,
                    "link": "https://example.com/1",
                    "creation_date": 0
                },
                {
                    "user_id": 2,
                    "display_name": "User2",
                    "reputation": 200,
                    "link": "https://example.com/2",
                    "creation_date": 0
                }
            ],
            "has_more": false
        });

        let response: StackExchangeResponse<User> = serde_json::from_value(json_data).unwrap();
        assert_eq!(response.items.len(), 2);
        assert_eq!(response.items[0].display_name, "User1");
        assert_eq!(response.items[1].display_name, "User2");
    }

    #[test]
    fn test_question_high_values() {
        let json_data = json!({
            "question_id": u64::MAX,
            "title": "Popular Question",
            "link": "https://example.com",
            "score": i32::MAX,
            "view_count": u64::MAX,
            "answer_count": u64::MAX,
            "is_answered": true,
            "creation_date": u64::MAX,
            "owner": {"display_name": "Famous"}
        });

        let question: Question = serde_json::from_value(json_data).unwrap();
        assert_eq!(question.question_id, u64::MAX);
        assert_eq!(question.score, i32::MAX);
        assert_eq!(question.view_count, u64::MAX);
    }

    #[test]
    fn test_user_high_reputation() {
        let json_data = json!({
            "user_id": 22656,
            "display_name": "Top User",
            "reputation": 1500000,
            "link": "https://stackoverflow.com/users/22656",
            "creation_date": 1222300800
        });

        let user: User = serde_json::from_value(json_data).unwrap();
        assert_eq!(user.reputation, 1500000);
    }

    // ========================================================================
    // INTEGRATION TESTS (IGNORED - require network)
    // ========================================================================

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_search_integration() {
        let tool = StackExchangeSearchTool::new("stackoverflow".to_string());
        let input = json!({
            "query": "rust",
            "max_results": "2"
        });

        let output = tool
            ._call(ToolInput::Structured(input))
            .await
            .expect("StackExchange search failed");
        assert!(output.contains("stackoverflow"));
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_question_integration() {
        let tool = StackExchangeQuestionTool::new("stackoverflow".to_string());
        let input = json!({
            "question_id": "1"
        });

        tool._call(ToolInput::Structured(input))
            .await
            .expect("StackExchange question fetch failed");
    }

    #[tokio::test]
    #[ignore = "requires network access"]
    async fn test_user_integration() {
        let tool = StackExchangeUserTool::new("stackoverflow".to_string());
        let input = json!({
            "user_id": "1"
        });

        tool._call(ToolInput::Structured(input))
            .await
            .expect("StackExchange user fetch failed");
    }
}
