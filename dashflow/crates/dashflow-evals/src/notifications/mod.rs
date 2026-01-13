//! Notification Integrations
//!
//! Send evaluation results to various notification channels.

pub mod slack;

pub use slack::{SlackConfig, SlackMessage, SlackNotifier};
