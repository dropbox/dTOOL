//! Chat prompt template implementation
//!
//! This module provides `ChatPromptTemplate` and `MessagesPlaceholder`
//! for building structured chat prompts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::core::deserialization::{
    extract_serialized_fields, get_optional_string_array, get_optional_string_map,
    get_string_array, validate_id, Deserializable,
};
use crate::core::error::{Error, Result};
use crate::core::messages::Message;
use crate::core::prompt_values::{ChatPromptValue, PromptValue};
use crate::core::prompts::base::{extract_fstring_variables, format_fstring, BasePromptTemplate};
use crate::core::serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION};

/// A message template part
///
/// This can be either a fixed message template or a placeholder for
/// a list of messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageTemplate {
    /// A system message template
    #[serde(rename = "system")]
    System {
        /// The f-string template content with {variable} placeholders.
        content: String,
    },

    /// A human message template
    #[serde(rename = "human")]
    Human {
        /// The f-string template content with {variable} placeholders.
        content: String,
    },

    /// An AI message template
    #[serde(rename = "ai")]
    AI {
        /// The f-string template content with {variable} placeholders.
        content: String,
    },

    /// A placeholder for a list of messages
    #[serde(rename = "placeholder")]
    Placeholder {
        /// Variable name containing the messages
        variable_name: String,
        /// Whether this placeholder is optional
        #[serde(default)]
        optional: bool,
    },
}

impl MessageTemplate {
    /// Extract variables from this message template
    fn extract_variables(&self) -> Vec<String> {
        match self {
            Self::System { content } | Self::Human { content } | Self::AI { content } => {
                extract_fstring_variables(content)
            }
            Self::Placeholder { .. } => Vec::new(),
        }
    }

    /// Format this message template with variables
    fn format(&self, variables: &HashMap<String, String>) -> Result<Vec<Message>> {
        match self {
            Self::System { content } => {
                let formatted = format_fstring(content, variables)?;
                Ok(vec![Message::system(formatted)])
            }
            Self::Human { content } => {
                let formatted = format_fstring(content, variables)?;
                Ok(vec![Message::human(formatted)])
            }
            Self::AI { content } => {
                let formatted = format_fstring(content, variables)?;
                Ok(vec![Message::ai(formatted)])
            }
            Self::Placeholder {
                variable_name,
                optional: _,
            } => {
                // Placeholders are handled separately in ChatPromptTemplate
                // This should not be called directly
                Err(Error::InvalidInput(format!(
                    "Cannot format placeholder '{variable_name}' directly. Use ChatPromptTemplate.format_messages()"
                )))
            }
        }
    }
}

/// A prompt template that produces a list of messages
///
/// `ChatPromptTemplate` combines multiple message templates and placeholders
/// to create a complete chat prompt.
///
/// # Example
///
/// ```
/// use std::collections::HashMap;
/// use dashflow::core::prompts::{ChatPromptTemplate, MessagesPlaceholder};
/// use dashflow::core::messages::Message;
///
/// let template = ChatPromptTemplate::from_messages(vec![
///     ("system", "You are a helpful assistant"),
///     ("placeholder", "history"),
///     ("human", "{question}"),
/// ]).unwrap();
///
/// let mut vars = HashMap::new();
/// vars.insert("question".to_string(), "What is 2+2?".to_string());
/// vars.insert("history".to_string(), serde_json::to_string(&vec![
///     Message::human("Hello"),
///     Message::ai("Hi there!"),
/// ]).unwrap());
///
/// let messages = template.format_messages(&vars).unwrap();
/// assert_eq!(messages.len(), 4); // system + 2 history + human
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPromptTemplate {
    /// The message templates
    pub messages: Vec<MessageTemplate>,

    /// Input variables extracted from templates
    pub input_variables: Vec<String>,

    /// Optional variables (placeholders marked as optional)
    #[serde(default)]
    pub optional_variables: Vec<String>,

    /// Partial variables (pre-filled values)
    #[serde(default)]
    pub partial_variables: HashMap<String, String>,
}

impl ChatPromptTemplate {
    /// Create a chat prompt template from a list of message tuples
    ///
    /// Each tuple is (role, content) where role is one of:
    /// - "system", "human", "ai" for fixed messages
    /// - "placeholder" for a `MessagesPlaceholder` (content is variable name)
    pub fn from_messages(messages: Vec<(&str, &str)>) -> Result<Self> {
        let mut templates = Vec::new();
        let mut input_variables = Vec::new();
        let mut optional_variables = Vec::new();

        for (role, content) in messages {
            let template = match role {
                "system" => MessageTemplate::System {
                    content: content.to_string(),
                },
                "human" => MessageTemplate::Human {
                    content: content.to_string(),
                },
                "ai" => MessageTemplate::AI {
                    content: content.to_string(),
                },
                "placeholder" => {
                    // Content is the variable name
                    let variable_name = content.to_string();
                    optional_variables.push(variable_name.clone());
                    MessageTemplate::Placeholder {
                        variable_name,
                        optional: true,
                    }
                }
                _ => {
                    return Err(Error::InvalidInput(format!(
                    "Unknown message role: {role}. Expected one of: system, human, ai, placeholder"
                )))
                }
            };

            // Extract variables from content
            let vars = template.extract_variables();
            for var in vars {
                if !input_variables.contains(&var) {
                    input_variables.push(var);
                }
            }

            templates.push(template);
        }

        Ok(Self {
            messages: templates,
            input_variables,
            optional_variables,
            partial_variables: HashMap::new(),
        })
    }

    /// Set partial variables
    #[must_use]
    pub fn with_partial_variables(mut self, partial_variables: HashMap<String, String>) -> Self {
        self.partial_variables = partial_variables;
        self
    }

    /// Format the template into a list of messages
    pub fn format_messages(&self, inputs: &HashMap<String, String>) -> Result<Vec<Message>> {
        // Merge with partial variables
        let merged = self.merge_inputs(inputs);

        let mut result = Vec::new();

        for template in &self.messages {
            if let MessageTemplate::Placeholder {
                variable_name,
                optional,
            } = template
            {
                // Try to get messages from variables
                if let Some(messages_json) = merged.get(variable_name) {
                    // Try to deserialize as Vec<Message>
                    match serde_json::from_str::<Vec<Message>>(messages_json) {
                        Ok(messages) => {
                            result.extend(messages);
                        }
                        Err(e) => {
                            return Err(Error::InvalidInput(format!(
                                "Failed to deserialize messages for placeholder '{variable_name}': {e}"
                            )));
                        }
                    }
                } else if !optional {
                    return Err(Error::InvalidInput(format!(
                        "Required placeholder variable '{variable_name}' not provided"
                    )));
                }
            } else {
                // Format regular message templates
                let messages = template.format(&merged)?;
                result.extend(messages);
            }
        }

        Ok(result)
    }
}

impl BasePromptTemplate for ChatPromptTemplate {
    fn input_variables(&self) -> &[String] {
        &self.input_variables
    }

    fn optional_variables(&self) -> &[String] {
        &self.optional_variables
    }

    fn partial_variables(&self) -> &HashMap<String, String> {
        &self.partial_variables
    }

    fn format_prompt(&self, inputs: &HashMap<String, String>) -> Result<Box<dyn PromptValue>> {
        let messages = self.format_messages(inputs)?;
        Ok(Box::new(ChatPromptValue::new(messages)))
    }
}

/// A placeholder for a list of messages
///
/// This is a convenience wrapper that creates a `MessageTemplate::Placeholder`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesPlaceholder {
    /// Variable name containing the messages
    pub variable_name: String,

    /// Whether this placeholder is optional
    #[serde(default)]
    pub optional: bool,
}

impl MessagesPlaceholder {
    /// Create a new messages placeholder
    pub fn new(variable_name: impl Into<String>) -> Self {
        Self {
            variable_name: variable_name.into(),
            optional: false,
        }
    }

    /// Create a new optional messages placeholder
    pub fn new_optional(variable_name: impl Into<String>) -> Self {
        Self {
            variable_name: variable_name.into(),
            optional: true,
        }
    }
}

// Serialization implementation
impl Serializable for ChatPromptTemplate {
    fn lc_id(&self) -> Vec<String> {
        vec![
            "dashflow_core".to_string(),
            "prompts".to_string(),
            "ChatPromptTemplate".to_string(),
        ]
    }

    fn is_lc_serializable(&self) -> bool {
        true
    }

    fn to_json(&self) -> SerializedObject {
        let mut kwargs = serde_json::Map::new();

        // Serialize the message templates
        kwargs.insert("messages".to_string(), serde_json::json!(self.messages));

        // Serialize input variables
        kwargs.insert(
            "input_variables".to_string(),
            serde_json::json!(self.input_variables),
        );

        // Optional fields (only include if non-empty)
        if !self.optional_variables.is_empty() {
            kwargs.insert(
                "optional_variables".to_string(),
                serde_json::json!(self.optional_variables),
            );
        }

        if !self.partial_variables.is_empty() {
            kwargs.insert(
                "partial_variables".to_string(),
                serde_json::json!(self.partial_variables),
            );
        }

        SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: self.lc_id(),
            kwargs: kwargs.into(),
        }
    }

    fn lc_secrets(&self) -> HashMap<String, String> {
        // ChatPromptTemplate has no secrets
        HashMap::new()
    }
}

impl Deserializable for ChatPromptTemplate {
    fn from_json(value: &serde_json::Value) -> Result<Self> {
        // Extract standard fields from serialized object
        let (_lc, id, kwargs) = extract_serialized_fields(value)?;

        // Validate this is a ChatPromptTemplate
        let expected_id = vec![
            "dashflow_core".to_string(),
            "prompts".to_string(),
            "ChatPromptTemplate".to_string(),
        ];
        validate_id(&id, &expected_id)?;

        // Extract required fields
        let messages_value = kwargs
            .get("messages")
            .ok_or_else(|| Error::InvalidInput("Missing 'messages' field".to_string()))?;

        // Deserialize message templates using serde
        let messages: Vec<MessageTemplate> = serde_json::from_value(messages_value.clone())
            .map_err(|e| Error::InvalidInput(format!("Failed to deserialize messages: {e}")))?;

        let input_variables = get_string_array(kwargs, "input_variables")?;

        // Extract optional fields with defaults
        let optional_variables =
            get_optional_string_array(kwargs, "optional_variables")?.unwrap_or_default();

        let partial_variables =
            get_optional_string_map(kwargs, "partial_variables")?.unwrap_or_default();

        Ok(ChatPromptTemplate {
            messages,
            input_variables,
            optional_variables,
            partial_variables,
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::core::deserialization::Deserializable;
    use crate::core::prompts::base::BasePromptTemplate;
    use crate::core::serialization::Serializable;
    use crate::test_prelude::*;

    #[test]
    fn test_message_template_system() {
        let template = MessageTemplate::System {
            content: "You are a {role}".to_string(),
        };

        let mut vars = HashMap::new();
        vars.insert("role".to_string(), "helpful assistant".to_string());

        let messages = template.format(&vars).unwrap();
        assert_eq!(messages.len(), 1);
        match &messages[0] {
            Message::System { content, .. } => {
                assert_eq!(content.as_text(), "You are a helpful assistant")
            }
            _ => panic!("Expected SystemMessage"),
        }
    }

    #[test]
    fn test_chat_prompt_template_simple() {
        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("human", "{question}"),
        ])
        .unwrap();

        assert_eq!(template.input_variables, vec!["question"]);

        let mut vars = HashMap::new();
        vars.insert("question".to_string(), "What is 2+2?".to_string());

        let messages = template.format_messages(&vars).unwrap();
        assert_eq!(messages.len(), 2);

        match &messages[0] {
            Message::System { content, .. } => {
                assert_eq!(content.as_text(), "You are a helpful assistant")
            }
            _ => panic!("Expected SystemMessage"),
        }

        match &messages[1] {
            Message::Human { content, .. } => assert_eq!(content.as_text(), "What is 2+2?"),
            _ => panic!("Expected HumanMessage"),
        }
    }

    #[test]
    fn test_chat_prompt_template_with_placeholder() {
        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("placeholder", "history"),
            ("human", "{question}"),
        ])
        .unwrap();

        // history should be in optional_variables
        assert!(template.optional_variables.contains(&"history".to_string()));

        let mut vars = HashMap::new();
        vars.insert("question".to_string(), "What is 2+2?".to_string());

        // Without history, should work (placeholder is optional)
        let messages = template.format_messages(&vars).unwrap();
        assert_eq!(messages.len(), 2); // system + human

        // With history
        let history = vec![Message::human("Hello"), Message::ai("Hi there!")];
        vars.insert(
            "history".to_string(),
            serde_json::to_string(&history).unwrap(),
        );

        let messages = template.format_messages(&vars).unwrap();
        assert_eq!(messages.len(), 4); // system + 2 history + human
    }

    #[test]
    fn test_chat_prompt_template_format_prompt() {
        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("human", "{question}"),
        ])
        .unwrap();

        let mut vars = HashMap::new();
        vars.insert("question".to_string(), "What is 2+2?".to_string());

        let prompt = template.format_prompt(&vars).unwrap();
        let messages = prompt.to_messages();
        assert_eq!(messages.len(), 2);

        // Check to_string works
        let string_repr = prompt.to_string();
        assert!(string_repr.contains("System:"));
        assert!(string_repr.contains("Human:"));
    }

    #[test]
    fn test_chat_prompt_template_with_partial_variables() {
        let mut partial = HashMap::new();
        partial.insert("assistant_name".to_string(), "Claude".to_string());

        let template = ChatPromptTemplate::from_messages(vec![(
            "system",
            "You are {assistant_name}, a helpful assistant",
        )])
        .unwrap()
        .with_partial_variables(partial);

        let vars = HashMap::new();
        let messages = template.format_messages(&vars).unwrap();

        match &messages[0] {
            Message::System { content, .. } => {
                assert_eq!(content.as_text(), "You are Claude, a helpful assistant")
            }
            _ => panic!("Expected SystemMessage"),
        }
    }

    #[test]
    fn test_messages_placeholder() {
        let placeholder = MessagesPlaceholder::new("history");
        assert_eq!(placeholder.variable_name, "history");
        assert!(!placeholder.optional);

        let placeholder_opt = MessagesPlaceholder::new_optional("history");
        assert_eq!(placeholder_opt.variable_name, "history");
        assert!(placeholder_opt.optional);
    }

    #[test]
    fn test_extract_variables_from_multiple_messages() {
        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are {role}"),
            ("human", "My name is {name}"),
            ("ai", "I will help you with {task}"),
        ])
        .unwrap();

        // Should extract all unique variables
        let mut expected_vars = vec!["role", "name", "task"];
        let mut actual_vars = template.input_variables.clone();
        expected_vars.sort();
        actual_vars.sort();
        assert_eq!(actual_vars, expected_vars);
    }

    #[test]
    fn test_chat_prompt_serialization_simple() {
        use crate::core::serialization::Serializable;

        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("human", "{question}"),
        ])
        .unwrap();

        let json_value = template.to_json_value().unwrap();

        // Check structure
        assert_eq!(json_value["lc"], 1);
        assert_eq!(json_value["type"], "constructor");
        assert_eq!(
            json_value["id"],
            serde_json::json!(["dashflow_core", "prompts", "ChatPromptTemplate"])
        );

        // Check kwargs
        let kwargs = &json_value["kwargs"];
        assert_eq!(kwargs["input_variables"], serde_json::json!(["question"]));
        assert_eq!(kwargs["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_chat_prompt_serialization_with_placeholder() {
        use crate::core::serialization::Serializable;

        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("placeholder", "history"),
            ("human", "{question}"),
        ])
        .unwrap();

        let json_value = template.to_json_value().unwrap();
        let kwargs = &json_value["kwargs"];

        // Check that placeholder is serialized correctly
        let messages = kwargs["messages"].as_array().unwrap();
        assert_eq!(messages.len(), 3);

        // The middle message should be a placeholder
        assert_eq!(messages[1]["type"], "placeholder");
        assert_eq!(messages[1]["variable_name"], "history");
    }

    #[test]
    fn test_chat_prompt_serialization_with_partial_variables() {
        use crate::core::serialization::Serializable;

        let mut partial = HashMap::new();
        partial.insert("assistant_name".to_string(), "Claude".to_string());

        let template = ChatPromptTemplate::from_messages(vec![(
            "system",
            "You are {assistant_name}, a helpful assistant",
        )])
        .unwrap()
        .with_partial_variables(partial.clone());

        let json_value = template.to_json_value().unwrap();
        let kwargs = &json_value["kwargs"];

        assert_eq!(kwargs["partial_variables"], serde_json::json!(partial));
    }

    #[test]
    fn test_chat_prompt_serialization_pretty_json() {
        use crate::core::serialization::Serializable;

        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a helpful assistant"),
            ("human", "{question}"),
        ])
        .unwrap();

        let json_string = template.to_json_string(true).unwrap();

        // Should be pretty-printed (contains newlines)
        assert!(json_string.contains('\n'));
        assert!(json_string.contains("dashflow_core"));
        assert!(json_string.contains("ChatPromptTemplate"));
    }

    #[test]
    fn test_chat_prompt_serialization_roundtrip() {
        use crate::core::serialization::Serializable;

        let template = ChatPromptTemplate::from_messages(vec![
            ("system", "You are {role}"),
            ("human", "{question}"),
        ])
        .unwrap();

        // Serialize to JSON value
        let json_value = template.to_json_value().unwrap();

        // Extract the kwargs to verify we can reconstruct
        let kwargs = json_value["kwargs"].as_object().unwrap();

        assert_eq!(
            kwargs["input_variables"],
            serde_json::json!(["role", "question"])
        );

        // Verify we can deserialize the messages back
        let messages: Vec<MessageTemplate> =
            serde_json::from_value(kwargs["messages"].clone()).unwrap();
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn test_chat_prompt_deserialization_basic() {
        let json = serde_json::json!({
            "lc": 1,
            "type": "constructor",
            "id": ["dashflow_core", "prompts", "ChatPromptTemplate"],
            "kwargs": {
                "messages": [
                    {"type": "system", "content": "You are a helpful assistant"},
                    {"type": "human", "content": "Hello {name}!"}
                ],
                "input_variables": ["name"]
            }
        });

        let template = ChatPromptTemplate::from_json(&json).unwrap();

        assert_eq!(template.messages.len(), 2);
        assert_eq!(template.input_variables, vec!["name"]);
        assert!(template.optional_variables.is_empty());
        assert!(template.partial_variables.is_empty());
    }

    #[test]
    fn test_chat_prompt_deserialization_with_placeholder() {
        let json = serde_json::json!({
            "lc": 1,
            "type": "constructor",
            "id": ["dashflow_core", "prompts", "ChatPromptTemplate"],
            "kwargs": {
                "messages": [
                    {"type": "system", "content": "You are a helpful assistant"},
                    {"type": "placeholder", "variable_name": "history", "optional": false},
                    {"type": "human", "content": "{question}"}
                ],
                "input_variables": ["question"]
            }
        });

        let template = ChatPromptTemplate::from_json(&json).unwrap();

        assert_eq!(template.messages.len(), 3);

        match &template.messages[1] {
            MessageTemplate::Placeholder {
                variable_name,
                optional,
            } => {
                assert_eq!(variable_name, "history");
                assert!(!optional);
            }
            _ => panic!("Expected placeholder"),
        }
    }

    #[test]
    fn test_chat_prompt_deserialization_full() {
        let mut partial_vars = HashMap::new();
        partial_vars.insert("assistant_name".to_string(), "Claude".to_string());

        let json = serde_json::json!({
            "lc": 1,
            "type": "constructor",
            "id": ["dashflow_core", "prompts", "ChatPromptTemplate"],
            "kwargs": {
                "messages": [
                    {"type": "system", "content": "You are {assistant_name}"},
                    {"type": "human", "content": "{question}"}
                ],
                "input_variables": ["question"],
                "optional_variables": ["context"],
                "partial_variables": {"assistant_name": "Claude"}
            }
        });

        let template = ChatPromptTemplate::from_json(&json).unwrap();

        assert_eq!(template.messages.len(), 2);
        assert_eq!(template.input_variables, vec!["question"]);
        assert_eq!(template.optional_variables, vec!["context"]);
        assert_eq!(
            template.partial_variables.get("assistant_name"),
            Some(&"Claude".to_string())
        );
    }

    #[test]
    fn test_chat_prompt_roundtrip_serialization_deserialization() {
        use crate::core::deserialization::from_json_str;

        let original = ChatPromptTemplate::from_messages(vec![
            ("system", "You are a {role}"),
            ("placeholder", "history"),
            ("human", "{question}"),
        ])
        .unwrap();

        // Serialize
        let json_str = original.to_json_string(false).unwrap();

        // Deserialize
        let reconstructed: ChatPromptTemplate = from_json_str(&json_str).unwrap();

        // Verify equivalence
        assert_eq!(original.messages.len(), reconstructed.messages.len());
        assert_eq!(original.input_variables, reconstructed.input_variables);
        assert_eq!(
            original.optional_variables,
            reconstructed.optional_variables
        );

        // Verify message templates match
        for (orig, recon) in original.messages.iter().zip(reconstructed.messages.iter()) {
            match (orig, recon) {
                (
                    MessageTemplate::System { content: c1 },
                    MessageTemplate::System { content: c2 },
                ) => assert_eq!(c1, c2),
                (
                    MessageTemplate::Human { content: c1 },
                    MessageTemplate::Human { content: c2 },
                ) => assert_eq!(c1, c2),
                (MessageTemplate::AI { content: c1 }, MessageTemplate::AI { content: c2 }) => {
                    assert_eq!(c1, c2)
                }
                (
                    MessageTemplate::Placeholder {
                        variable_name: v1,
                        optional: o1,
                    },
                    MessageTemplate::Placeholder {
                        variable_name: v2,
                        optional: o2,
                    },
                ) => {
                    assert_eq!(v1, v2);
                    assert_eq!(o1, o2);
                }
                _ => panic!("Message template types don't match"),
            }
        }
    }

    #[test]
    fn test_chat_prompt_deserialization_type_mismatch() {
        let json = serde_json::json!({
            "lc": 1,
            "type": "constructor",
            "id": ["dashflow_core", "wrong", "Type"],
            "kwargs": {}
        });

        let result = ChatPromptTemplate::from_json(&json);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Type mismatch"));
    }

    #[test]
    fn test_chat_prompt_deserialization_missing_required_field() {
        let json = serde_json::json!({
            "lc": 1,
            "type": "constructor",
            "id": ["dashflow_core", "prompts", "ChatPromptTemplate"],
            "kwargs": {
                "input_variables": ["name"]
                // Missing "messages" field
            }
        });

        let result = ChatPromptTemplate::from_json(&json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Missing 'messages' field"));
    }
}
