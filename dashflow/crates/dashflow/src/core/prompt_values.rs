//! Prompt values for language model prompts
//!
//! Prompt values are used to represent different pieces of prompts.
//! They can be used to represent text or chat message pieces.

use serde::{Deserialize, Serialize};

use crate::core::messages::Message;

/// Base trait for inputs to any language model
///
/// `PromptValue` can be converted to both LLM (pure text-generation) inputs
/// and chat model inputs.
pub trait PromptValue: Send + Sync {
    /// Return prompt value as string
    fn to_string(&self) -> String;

    /// Return prompt as a list of messages
    fn to_messages(&self) -> Vec<Message>;
}

/// String prompt value
///
/// Represents a simple text prompt that can be used with LLMs or
/// converted to a `HumanMessage` for chat models.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StringPromptValue {
    /// Prompt text
    pub text: String,
}

impl StringPromptValue {
    /// Create a new string prompt value
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

impl PromptValue for StringPromptValue {
    fn to_string(&self) -> String {
        self.text.clone()
    }

    fn to_messages(&self) -> Vec<Message> {
        vec![Message::human(self.text.clone())]
    }
}

/// Chat prompt value
///
/// A type of prompt value that is built from messages.
/// This is used when you want to pass a sequence of messages
/// to a chat model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatPromptValue {
    /// List of messages
    pub messages: Vec<Message>,
}

impl ChatPromptValue {
    /// Create a new chat prompt value from messages
    #[must_use]
    pub fn new(messages: Vec<Message>) -> Self {
        Self { messages }
    }
}

impl PromptValue for ChatPromptValue {
    fn to_string(&self) -> String {
        // Convert messages to a buffer string representation
        // Format: "System: ...\nHuman: ...\nAI: ..."
        self.messages
            .iter()
            .map(|msg| match msg {
                Message::System { content, .. } => format!("System: {}", content.as_text()),
                Message::Human { content, .. } => format!("Human: {}", content.as_text()),
                Message::AI { content, .. } => format!("AI: {}", content.as_text()),
                Message::Tool { content, .. } => format!("Tool: {}", content.as_text()),
                Message::Function { content, name, .. } => {
                    format!("Function({}): {}", name, content.as_text())
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn to_messages(&self) -> Vec<Message> {
        self.messages.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::PromptValue;
    use crate::test_prelude::*;

    #[test]
    fn test_string_prompt_value() {
        let prompt = StringPromptValue::new("Hello world");
        assert_eq!(prompt.to_string(), "Hello world");

        let messages = prompt.to_messages();
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Hello world"),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_chat_prompt_value() {
        let messages = vec![
            Message::system("You are a helpful assistant"),
            Message::human("What is 2+2?"),
            Message::ai("2+2 equals 4"),
        ];

        let prompt = ChatPromptValue::new(messages.clone());
        let output = prompt.to_string();

        assert!(output.contains("System: You are a helpful assistant"));
        assert!(output.contains("Human: What is 2+2?"));
        assert!(output.contains("AI: 2+2 equals 4"));

        let returned_messages = prompt.to_messages();
        assert_eq!(returned_messages.len(), 3);
    }

    #[test]
    fn test_chat_prompt_value_to_messages() {
        let messages = vec![Message::human("Hello")];
        let prompt = ChatPromptValue::new(messages);
        let result = prompt.to_messages();

        assert_eq!(result.len(), 1);
        match &result[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "Hello"),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_string_prompt_value_empty() {
        let prompt = StringPromptValue::new("");
        assert_eq!(prompt.to_string(), "");

        let messages = prompt.to_messages();
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), ""),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_string_prompt_value_multiline() {
        let text = "Line 1\nLine 2\nLine 3";
        let prompt = StringPromptValue::new(text);
        assert_eq!(prompt.to_string(), text);

        let messages = prompt.to_messages();
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), text),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_string_prompt_value_serialization() {
        let prompt = StringPromptValue::new("Test prompt");
        let json = serde_json::to_string(&prompt).unwrap();
        assert!(json.contains("Test prompt"));

        let deserialized: StringPromptValue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, prompt);
        assert_eq!(deserialized.text, "Test prompt");
    }

    #[test]
    fn test_string_prompt_value_clone() {
        let prompt = StringPromptValue::new("Original");
        let cloned = prompt.clone();

        assert_eq!(prompt.text, cloned.text);
        assert_eq!(prompt.to_string(), cloned.to_string());
    }

    #[test]
    fn test_string_prompt_value_equality() {
        let prompt1 = StringPromptValue::new("Same text");
        let prompt2 = StringPromptValue::new("Same text");
        let prompt3 = StringPromptValue::new("Different text");

        assert_eq!(prompt1, prompt2);
        assert_ne!(prompt1, prompt3);
    }

    #[test]
    fn test_chat_prompt_value_empty() {
        let prompt = ChatPromptValue::new(vec![]);
        assert_eq!(prompt.to_string(), "");

        let messages = prompt.to_messages();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_chat_prompt_value_single_system() {
        let messages = vec![Message::system("You are helpful")];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert_eq!(output, "System: You are helpful");

        let returned = prompt.to_messages();
        assert_eq!(returned.len(), 1);
    }

    #[test]
    fn test_chat_prompt_value_single_human() {
        let messages = vec![Message::human("Hello")];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert_eq!(output, "Human: Hello");

        let returned = prompt.to_messages();
        assert_eq!(returned.len(), 1);
    }

    #[test]
    fn test_chat_prompt_value_single_ai() {
        let messages = vec![Message::ai("Hi there!")];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert_eq!(output, "AI: Hi there!");

        let returned = prompt.to_messages();
        assert_eq!(returned.len(), 1);
    }

    #[test]
    fn test_chat_prompt_value_with_tool_message() {
        let messages = vec![
            Message::human("Use calculator"),
            Message::tool("42", "calc_123"),
        ];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert!(output.contains("Human: Use calculator"));
        assert!(output.contains("Tool: 42"));

        let returned = prompt.to_messages();
        assert_eq!(returned.len(), 2);
    }

    #[test]
    fn test_chat_prompt_value_all_message_types() {
        let messages = vec![
            Message::system("System message"),
            Message::human("Human message"),
            Message::ai("AI message"),
            Message::tool("Tool output", "tool_id"),
        ];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert!(output.contains("System: System message"));
        assert!(output.contains("Human: Human message"));
        assert!(output.contains("AI: AI message"));
        assert!(output.contains("Tool: Tool output"));

        let returned = prompt.to_messages();
        assert_eq!(returned.len(), 4);
    }

    #[test]
    fn test_chat_prompt_value_multiline_messages() {
        let messages = vec![
            Message::system("Line 1\nLine 2\nLine 3"),
            Message::human("Question\nwith\nmultiple\nlines"),
        ];
        let prompt = ChatPromptValue::new(messages);

        let output = prompt.to_string();
        assert!(output.contains("System: Line 1\nLine 2\nLine 3"));
        assert!(output.contains("Human: Question\nwith\nmultiple\nlines"));
    }

    #[test]
    fn test_chat_prompt_value_serialization() {
        let messages = vec![
            Message::system("System"),
            Message::human("Human"),
            Message::ai("AI"),
        ];
        let prompt = ChatPromptValue::new(messages);

        let json = serde_json::to_string(&prompt).unwrap();
        assert!(json.contains("System"));
        assert!(json.contains("Human"));
        assert!(json.contains("AI"));

        let deserialized: ChatPromptValue = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, prompt);
        assert_eq!(deserialized.messages.len(), 3);
    }

    #[test]
    fn test_chat_prompt_value_clone() {
        let messages = vec![Message::human("Original message")];
        let prompt = ChatPromptValue::new(messages);
        let cloned = prompt.clone();

        assert_eq!(prompt.messages.len(), cloned.messages.len());
        assert_eq!(prompt.to_string(), cloned.to_string());
    }

    #[test]
    fn test_chat_prompt_value_equality() {
        let messages1 = vec![Message::human("Same")];
        let messages2 = vec![Message::human("Same")];
        let messages3 = vec![Message::human("Different")];

        let prompt1 = ChatPromptValue::new(messages1);
        let prompt2 = ChatPromptValue::new(messages2);
        let prompt3 = ChatPromptValue::new(messages3);

        assert_eq!(prompt1, prompt2);
        assert_ne!(prompt1, prompt3);
    }

    #[test]
    fn test_chat_prompt_value_order_matters() {
        let messages1 = vec![Message::human("First"), Message::ai("Second")];
        let messages2 = vec![Message::ai("Second"), Message::human("First")];

        let prompt1 = ChatPromptValue::new(messages1);
        let prompt2 = ChatPromptValue::new(messages2);

        // Order should matter for equality
        assert_ne!(prompt1, prompt2);

        // Output should reflect order
        let output1 = prompt1.to_string();
        let output2 = prompt2.to_string();
        assert_ne!(output1, output2);
        assert!(output1.starts_with("Human:"));
        assert!(output2.starts_with("AI:"));
    }

    #[test]
    fn test_string_prompt_value_from_string() {
        let text = String::from("Test");
        let prompt = StringPromptValue::new(text.clone());
        assert_eq!(prompt.text, text);
    }

    #[test]
    fn test_string_prompt_value_from_str() {
        let prompt = StringPromptValue::new("Test");
        assert_eq!(prompt.text, "Test");
    }

    #[test]
    fn test_chat_prompt_value_string_format() {
        let messages = vec![
            Message::system("System"),
            Message::human("Human"),
            Message::ai("AI"),
        ];
        let prompt = ChatPromptValue::new(messages);
        let output = prompt.to_string();

        // Check format: each message on new line with prefix
        let lines: Vec<&str> = output.split('\n').collect();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "System: System");
        assert_eq!(lines[1], "Human: Human");
        assert_eq!(lines[2], "AI: AI");
    }

    #[test]
    fn test_string_prompt_value_special_characters() {
        let text = "Text with \"quotes\" and <tags> and &ampersands&";
        let prompt = StringPromptValue::new(text);
        assert_eq!(prompt.to_string(), text);

        let messages = prompt.to_messages();
        match &messages[0] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), text),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_chat_prompt_value_special_characters() {
        let messages = vec![
            Message::human("Text with \"quotes\""),
            Message::ai("Text with <tags>"),
        ];
        let prompt = ChatPromptValue::new(messages);
        let output = prompt.to_string();

        assert!(output.contains("\"quotes\""));
        assert!(output.contains("<tags>"));
    }
}
