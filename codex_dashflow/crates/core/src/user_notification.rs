//! User notifications for agent events
//!
//! This module provides a notification system that allows users to configure
//! external programs to be notified when agent events occur (e.g., turn completion).

use serde::Serialize;
use std::process::Command;
use tracing::{error, warn};

/// Handles sending notifications to an external program.
///
/// Users can configure a command that will receive notifications as JSON arguments.
#[derive(Debug, Default)]
pub struct UserNotifier {
    /// Command and arguments to execute for notifications
    notify_command: Option<Vec<String>>,
}

impl UserNotifier {
    /// Create a new user notifier.
    ///
    /// # Arguments
    ///
    /// * `notify` - Optional command specification. If provided, the first element
    ///   is the program and subsequent elements are its arguments.
    pub fn new(notify: Option<Vec<String>>) -> Self {
        Self {
            notify_command: notify,
        }
    }

    /// Send a notification to the configured external program.
    ///
    /// The notification is serialized to JSON and passed as the final argument
    /// to the configured command. The command is spawned in a fire-and-forget
    /// manner - we don't wait for completion.
    pub fn notify(&self, notification: &UserNotification) {
        if let Some(notify_command) = &self.notify_command {
            if !notify_command.is_empty() {
                self.invoke_notify(notify_command, notification);
            }
        }
    }

    /// Check if a notifier is configured
    pub fn is_configured(&self) -> bool {
        self.notify_command
            .as_ref()
            .is_some_and(|cmd| !cmd.is_empty())
    }

    fn invoke_notify(&self, notify_command: &[String], notification: &UserNotification) {
        let Ok(json) = serde_json::to_string(&notification) else {
            error!("Failed to serialize notification payload");
            return;
        };

        let mut command = Command::new(&notify_command[0]);
        if notify_command.len() > 1 {
            command.args(&notify_command[1..]);
        }
        command.arg(&json);

        // Fire-and-forget – we do not wait for completion.
        if let Err(e) = command.spawn() {
            warn!("Failed to spawn notifier '{}': {e}", notify_command[0]);
        }
    }
}

/// User notification events.
///
/// Each notification is serialized as JSON and passed as an argument to the
/// configured notification program.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum UserNotification {
    /// Notification sent when an agent turn completes.
    #[serde(rename_all = "kebab-case")]
    AgentTurnComplete {
        /// Thread/session identifier
        thread_id: String,
        /// Turn identifier within the session
        turn_id: String,
        /// Current working directory
        cwd: String,
        /// Messages that the user sent to the agent to initiate the turn
        input_messages: Vec<String>,
        /// The last message sent by the assistant in the turn
        last_assistant_message: Option<String>,
    },

    /// Notification sent when an error occurs.
    #[serde(rename_all = "kebab-case")]
    AgentError {
        /// Thread/session identifier
        thread_id: String,
        /// Error message
        error: String,
    },

    /// Notification sent when agent requests user input.
    #[serde(rename_all = "kebab-case")]
    AgentAwaitingInput {
        /// Thread/session identifier
        thread_id: String,
        /// Prompt for the user
        prompt: Option<String>,
    },
}

impl UserNotification {
    /// Create a turn complete notification.
    pub fn turn_complete(
        thread_id: impl Into<String>,
        turn_id: impl Into<String>,
        cwd: impl Into<String>,
        input_messages: Vec<String>,
        last_assistant_message: Option<String>,
    ) -> Self {
        Self::AgentTurnComplete {
            thread_id: thread_id.into(),
            turn_id: turn_id.into(),
            cwd: cwd.into(),
            input_messages,
            last_assistant_message,
        }
    }

    /// Create an error notification.
    pub fn error(thread_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self::AgentError {
            thread_id: thread_id.into(),
            error: error.into(),
        }
    }

    /// Create an awaiting input notification.
    pub fn awaiting_input(thread_id: impl Into<String>, prompt: Option<String>) -> Self {
        Self::AgentAwaitingInput {
            thread_id: thread_id.into(),
            prompt,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_notifier_new() {
        let notifier = UserNotifier::new(None);
        assert!(!notifier.is_configured());

        let notifier = UserNotifier::new(Some(vec![]));
        assert!(!notifier.is_configured());

        let notifier = UserNotifier::new(Some(vec!["notify-send".to_string()]));
        assert!(notifier.is_configured());
    }

    #[test]
    fn test_user_notification_turn_complete() {
        let notification = UserNotification::turn_complete(
            "b5f6c1c2-1111-2222-3333-444455556666",
            "12345",
            "/Users/example/project",
            vec!["Rename `foo` to `bar` and update the callsites.".to_string()],
            Some("Rename complete and verified `cargo build` succeeds.".to_string()),
        );

        let serialized = serde_json::to_string(&notification).unwrap();
        assert_eq!(
            serialized,
            r#"{"type":"agent-turn-complete","thread-id":"b5f6c1c2-1111-2222-3333-444455556666","turn-id":"12345","cwd":"/Users/example/project","input-messages":["Rename `foo` to `bar` and update the callsites."],"last-assistant-message":"Rename complete and verified `cargo build` succeeds."}"#
        );
    }

    #[test]
    fn test_user_notification_turn_complete_no_last_message() {
        let notification = UserNotification::turn_complete(
            "thread-1",
            "turn-1",
            "/tmp",
            vec!["Hello".to_string()],
            None,
        );

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""last-assistant-message":null"#));
    }

    #[test]
    fn test_user_notification_error() {
        let notification = UserNotification::error("thread-1", "Something went wrong");

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""type":"agent-error""#));
        assert!(serialized.contains(r#""error":"Something went wrong""#));
    }

    #[test]
    fn test_user_notification_awaiting_input() {
        let notification =
            UserNotification::awaiting_input("thread-1", Some("Enter your name:".to_string()));

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""type":"agent-awaiting-input""#));
        assert!(serialized.contains(r#""prompt":"Enter your name:""#));
    }

    #[test]
    fn test_user_notification_awaiting_input_no_prompt() {
        let notification = UserNotification::awaiting_input("thread-1", None);

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""prompt":null"#));
    }

    #[test]
    fn test_user_notification_equality() {
        let n1 = UserNotification::turn_complete("t1", "turn1", "/", vec![], None);
        let n2 = UserNotification::turn_complete("t1", "turn1", "/", vec![], None);
        assert_eq!(n1, n2);

        let n3 = UserNotification::turn_complete("t2", "turn1", "/", vec![], None);
        assert_ne!(n1, n3);
    }

    #[test]
    fn test_notifier_default() {
        let notifier = UserNotifier::default();
        assert!(!notifier.is_configured());
    }

    #[test]
    fn test_notification_multiple_input_messages() {
        let notification = UserNotification::turn_complete(
            "thread-1",
            "turn-1",
            "/home/user",
            vec![
                "First message".to_string(),
                "Second message".to_string(),
                "Third message".to_string(),
            ],
            Some("Done!".to_string()),
        );

        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized
            .contains(r#""input-messages":["First message","Second message","Third message"]"#));
    }

    #[test]
    fn test_user_notifier_debug() {
        let notifier = UserNotifier::new(Some(vec!["echo".to_string(), "arg1".to_string()]));
        let debug_str = format!("{:?}", notifier);
        assert!(debug_str.contains("UserNotifier"));
        assert!(debug_str.contains("notify_command"));
    }

    #[test]
    fn test_user_notification_clone() {
        let notification = UserNotification::turn_complete(
            "thread-1",
            "turn-1",
            "/tmp",
            vec!["msg".to_string()],
            Some("response".to_string()),
        );
        let cloned = notification.clone();
        assert_eq!(notification, cloned);
    }

    #[test]
    fn test_user_notification_error_clone() {
        let notification = UserNotification::error("t1", "error message");
        let cloned = notification.clone();
        assert_eq!(notification, cloned);
    }

    #[test]
    fn test_user_notification_awaiting_input_clone() {
        let notification = UserNotification::awaiting_input("t1", Some("prompt".to_string()));
        let cloned = notification.clone();
        assert_eq!(notification, cloned);
    }

    #[test]
    fn test_notification_empty_thread_id() {
        let notification = UserNotification::turn_complete("", "turn", "/", vec![], None);
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""thread-id":"""#));
    }

    #[test]
    fn test_notification_empty_turn_id() {
        let notification = UserNotification::turn_complete("thread", "", "/", vec![], None);
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""turn-id":"""#));
    }

    #[test]
    fn test_notification_empty_cwd() {
        let notification = UserNotification::turn_complete("thread", "turn", "", vec![], None);
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""cwd":"""#));
    }

    #[test]
    fn test_notification_empty_input_messages() {
        let notification = UserNotification::turn_complete("t", "turn", "/", vec![], None);
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""input-messages":[]"#));
    }

    #[test]
    fn test_notification_error_empty_error() {
        let notification = UserNotification::error("thread", "");
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains(r#""error":"""#));
    }

    #[test]
    fn test_notification_special_characters() {
        let notification = UserNotification::turn_complete(
            "thread-with-dashes",
            "turn_with_underscores",
            "/path/with spaces/and\"quotes",
            vec!["message with\nnewline".to_string()],
            Some("response with\ttab".to_string()),
        );
        let serialized = serde_json::to_string(&notification).unwrap();
        // JSON should properly escape special characters
        assert!(serialized.contains("\\n")); // newline escaped
        assert!(serialized.contains("\\t")); // tab escaped
        assert!(serialized.contains("\\\"quotes")); // quotes escaped
    }

    #[test]
    fn test_notification_unicode() {
        let notification = UserNotification::turn_complete(
            "线程",
            "回合",
            "/путь/路径",
            vec!["消息 🎉".to_string()],
            Some("响应 ✓".to_string()),
        );
        let serialized = serde_json::to_string(&notification).unwrap();
        assert!(serialized.contains("线程"));
        assert!(serialized.contains("路径"));
        assert!(serialized.contains("🎉"));
        assert!(serialized.contains("✓"));
    }

    #[test]
    fn test_user_notifier_new_with_multiple_args() {
        let notifier = UserNotifier::new(Some(vec![
            "notify-send".to_string(),
            "-u".to_string(),
            "critical".to_string(),
            "-t".to_string(),
            "5000".to_string(),
        ]));
        assert!(notifier.is_configured());
    }

    #[test]
    fn test_notification_types_distinct() {
        let turn_complete = UserNotification::turn_complete("t", "turn", "/", vec![], None);
        let error = UserNotification::error("t", "err");
        let awaiting = UserNotification::awaiting_input("t", None);

        // Verify they serialize with different type tags
        let tc_json = serde_json::to_string(&turn_complete).unwrap();
        let err_json = serde_json::to_string(&error).unwrap();
        let await_json = serde_json::to_string(&awaiting).unwrap();

        assert!(tc_json.contains("agent-turn-complete"));
        assert!(!tc_json.contains("agent-error"));
        assert!(!tc_json.contains("agent-awaiting-input"));

        assert!(err_json.contains("agent-error"));
        assert!(!err_json.contains("agent-turn-complete"));

        assert!(await_json.contains("agent-awaiting-input"));
        assert!(!await_json.contains("agent-turn-complete"));
    }

    #[test]
    fn test_notification_inequality_different_types() {
        let turn = UserNotification::turn_complete("t", "turn", "/", vec![], None);
        let error = UserNotification::error("t", "err");

        assert_ne!(turn, error);
    }

    #[test]
    fn test_notification_awaiting_equality() {
        let n1 = UserNotification::awaiting_input("t1", Some("p".to_string()));
        let n2 = UserNotification::awaiting_input("t1", Some("p".to_string()));
        let n3 = UserNotification::awaiting_input("t1", Some("different".to_string()));

        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }

    #[test]
    fn test_notification_error_equality() {
        let n1 = UserNotification::error("t", "error");
        let n2 = UserNotification::error("t", "error");
        let n3 = UserNotification::error("t", "different");

        assert_eq!(n1, n2);
        assert_ne!(n1, n3);
    }

    #[test]
    fn test_notifier_with_single_element() {
        let notifier = UserNotifier::new(Some(vec!["echo".to_string()]));
        assert!(notifier.is_configured());

        // Test notify doesn't panic (we can't easily test the actual command execution)
        let notification = UserNotification::error("t", "test");
        notifier.notify(&notification);
    }

    #[test]
    fn test_notification_long_strings() {
        let long_string = "a".repeat(10000);
        let notification = UserNotification::turn_complete(
            &long_string,
            &long_string,
            &long_string,
            vec![long_string.clone()],
            Some(long_string.clone()),
        );
        let serialized = serde_json::to_string(&notification).unwrap();
        // Should still serialize correctly
        assert!(serialized.len() > 50000);
    }
}
