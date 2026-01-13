//! External data source and API integrations.
//!
//! This module provides loaders for external data sources:
//! - Databases (`PostgreSQL`, `MySQL`, `MongoDB`, `BigQuery`, etc.)
//! - Cloud storage (S3, Azure, GCS, Dropbox, Google Drive, etc.)
//! - `SaaS` platforms (Notion, Confluence, Jira, Salesforce, etc.)
//! - Communication platforms (Slack, Discord, Telegram, etc.)
//! - Social media (Twitter, Reddit, Mastodon)
//! - Content platforms (Wikipedia, `ArXiv`, News, `YouTube`)
//! - Developer platforms (GitHub, `GitBook`, Git)

pub mod cloud;
pub mod communication;
pub mod content;
pub mod databases;
pub mod developer;
pub mod saas;
pub mod social;

// Re-export all loaders for convenience
// cloud::* not re-exported (no implemented loaders in cloud module as of N=301)
pub use communication::*;
pub use content::*;
// databases::* not re-exported (no implemented loaders in databases module as of N=301)
pub use developer::*;
// saas::* not re-exported (no implemented loaders in saas module as of N=301)
pub use social::*;
