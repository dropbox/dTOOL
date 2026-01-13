//! SQL Database Chain
//!
//! This module provides functions for generating SQL queries based on natural language questions.
//!
//! # Example
//!
//! ```rust,no_run
//! use dashflow_chains::sql_database_chain::{generate_sql_query, SQLInput, SQLDatabaseInfo};
//! use dashflow::core::language_models::ChatModel;
//! # use dashflow::core::Error;
//!
//! # async fn example<M: ChatModel>(llm: M) -> Result<(), Error> {
//! // Create database info with dialect and table information function
//! let db_info = SQLDatabaseInfo::new(
//!     "postgresql".to_string(),
//!     |tables| Ok("Table schema info here".to_string())
//! );
//!
//! // Generate SQL query from natural language question
//! let input = SQLInput {
//!     question: "How many users are there?".to_string(),
//!     table_names_to_use: None,
//! };
//!
//! let sql_query = generate_sql_query(&llm, &db_info, input, None, 5).await?;
//! println!("Generated SQL: {}", sql_query);
//! # Ok(())
//! # }
//! ```

use dashflow::core::language_models::ChatModel;
use dashflow::core::prompts::ChatPromptTemplate;
use dashflow::core::Error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::sql_database_prompts;

/// Input for a SQL query chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SQLInput {
    /// The natural language question to convert to SQL
    pub question: String,
    /// Optional list of specific tables to use (for security/scoping)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_names_to_use: Option<Vec<String>>,
}

/// Helper struct to hold SQL database information
#[derive(Clone)]
pub struct SQLDatabaseInfo {
    dialect: String,
    #[allow(clippy::type_complexity)] // Callback for dynamic table introspection requires full trait bounds
    get_table_info_fn: Arc<dyn Fn(Option<Vec<String>>) -> Result<String, Error> + Send + Sync>,
}

impl SQLDatabaseInfo {
    /// Create a new `SQLDatabaseInfo`
    pub fn new<F>(dialect: String, get_table_info_fn: F) -> Self
    where
        F: Fn(Option<Vec<String>>) -> Result<String, Error> + Send + Sync + 'static,
    {
        Self {
            dialect,
            get_table_info_fn: Arc::new(get_table_info_fn),
        }
    }

    /// Get table information for the specified tables
    pub fn get_table_info(&self, table_names: Option<Vec<String>>) -> Result<String, Error> {
        (self.get_table_info_fn)(table_names)
    }

    /// Get the SQL dialect
    #[must_use]
    pub fn dialect(&self) -> &str {
        &self.dialect
    }
}

/// Generate a SQL query from a natural language question.
///
/// # Security Note
///
/// This function generates SQL queries for the given database. To mitigate risk of leaking
/// sensitive data:
/// - Limit database permissions to read-only
/// - Scope access to only necessary tables
/// - Use `table_names_to_use` in `SQLInput` to restrict which tables can be queried
/// - Control who can submit requests
///
/// # Arguments
///
/// * `llm` - The language model to use for query generation
/// * `db_info` - Database information including dialect and schema
/// * `input` - The question and optional table restrictions
/// * `prompt` - Optional custom prompt template. If None, uses dialect-specific default
/// * `k` - Maximum number of results to return per query (used in LIMIT clause)
///
/// # Returns
///
/// A SQL query string generated from the natural language question
///
/// # Example
///
/// ```rust,no_run
/// use dashflow_chains::sql_database_chain::{generate_sql_query, SQLInput, SQLDatabaseInfo};
/// # use dashflow::core::Error;
/// # use dashflow::core::language_models::ChatModel;
///
/// # async fn example<M: ChatModel>(llm: M) -> Result<(), Error> {
/// let db_info = SQLDatabaseInfo::new(
///     "postgresql".to_string(),
///     |_| Ok("users table: id INTEGER, name TEXT, email TEXT".to_string())
/// );
///
/// let input = SQLInput {
///     question: "How many employees are in the sales department?".to_string(),
///     table_names_to_use: Some(vec!["employees".to_string(), "departments".to_string()]),
/// };
///
/// let sql = generate_sql_query(&llm, &db_info, input, None, 5).await?;
/// println!("SQL: {}", sql);
/// # Ok(())
/// # }
/// ```
pub async fn generate_sql_query<M>(
    llm: &M,
    db_info: &SQLDatabaseInfo,
    input: SQLInput,
    prompt: Option<ChatPromptTemplate>,
    k: usize,
) -> Result<String, Error>
where
    M: ChatModel,
{
    // Get or create the prompt template
    let prompt_template = if let Some(p) = prompt {
        p
    } else {
        sql_database_prompts::create_sql_prompt(db_info.dialect())
    };

    // Get table information
    let table_info = db_info.get_table_info(input.table_names_to_use.clone())?;

    // Prepare prompt variables
    let mut variables = HashMap::new();
    variables.insert(
        "input".to_string(),
        format!("{}\nSQLQuery: ", input.question),
    );
    variables.insert("table_info".to_string(), table_info);
    variables.insert("top_k".to_string(), k.to_string());
    variables.insert("dialect".to_string(), db_info.dialect().to_string());

    // Format the prompt
    let messages = prompt_template.format_messages(&variables)?;

    // Call the LLM
    let result = llm.generate(&messages, None, None, None, None).await?;

    // Extract the SQL query from the response
    let response_text = result
        .generations
        .first()
        .ok_or_else(|| Error::other("No response generated from LLM"))?
        .message
        .as_text();

    // Strip the response and extract just the SQL query
    let sql_query = extract_sql_query(&response_text);

    Ok(sql_query)
}

/// Extract SQL query from LLM response
///
/// The LLM response may contain additional text like "`SQLQuery`: SELECT..."
/// or multi-line responses. This function extracts just the SQL query.
fn extract_sql_query(response: &str) -> String {
    let response = response.trim();

    // Check if response contains "SQLQuery:" marker
    if let Some(sql_start) = response.find("SQLQuery:") {
        let after_marker = &response[sql_start + 9..];

        // Find the end of the SQL query (marked by SQLResult: or Answer: or end of string)
        let sql_end = after_marker
            .find("SQLResult:")
            .or_else(|| after_marker.find("Answer:"))
            .unwrap_or(after_marker.len());

        after_marker[..sql_end].trim().to_string()
    } else {
        // If no marker, assume the entire response is the SQL query
        // Stop at first occurrence of SQLResult: or Answer:
        let sql_end = response
            .find("SQLResult:")
            .or_else(|| response.find("Answer:"))
            .unwrap_or(response.len());

        response[..sql_end].trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_sql_query_with_marker() {
        let response = "SQLQuery: SELECT * FROM users LIMIT 5\nSQLResult: ...";
        let sql = extract_sql_query(response);
        assert_eq!(sql, "SELECT * FROM users LIMIT 5");
    }

    #[test]
    fn test_extract_sql_query_without_marker() {
        let response = "SELECT * FROM users LIMIT 5";
        let sql = extract_sql_query(response);
        assert_eq!(sql, "SELECT * FROM users LIMIT 5");
    }

    #[test]
    fn test_extract_sql_query_with_answer() {
        let response = "SQLQuery: SELECT COUNT(*) FROM users\nAnswer: There are 100 users";
        let sql = extract_sql_query(response);
        assert_eq!(sql, "SELECT COUNT(*) FROM users");
    }

    #[test]
    fn test_sql_input_serialization() {
        let input = SQLInput {
            question: "How many users?".to_string(),
            table_names_to_use: Some(vec!["users".to_string()]),
        };

        let json = serde_json::to_string(&input).unwrap();
        assert!(json.contains("How many users?"));
        assert!(json.contains("users"));
    }
}
