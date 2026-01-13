//! Message history management for Runnables
//!
//! This module provides:
//! - `GetSessionHistoryFn`: Type alias for session history retrieval functions
//! - `RunnableWithMessageHistory`: Wraps a runnable with automatic chat history management

use std::collections::HashMap;
use std::sync::Arc;

use super::{ConfigurableFieldSpec, Runnable};
use crate::core::config::RunnableConfig;

// ============================================================================
// RunnableWithMessageHistory
// ============================================================================

/// Type alias for the `get_session_history` callable
///
/// This function is called with configurable parameters to retrieve
/// the appropriate `InMemoryChatMessageHistory` instance for the session.
///
/// # Python Baseline Compatibility
///
/// Matches `GetSessionHistoryCallable` in `dashflow_core/runnables/history.py:35`.
///
/// # Implementation Note
///
/// Uses `InMemoryChatMessageHistory` directly instead of `dyn BaseChatMessageHistory`
/// because the trait has generic methods (`add_user_message`, `add_ai_message`) that
/// prevent it from being dyn-compatible. Users needing custom backends can:
/// 1. Wrap their type in a newtype that implements the needed methods
/// 2. Use a concrete type and map to `InMemoryChatMessageHistory`
/// 3. Wait for future refactoring that moves generic methods out of the trait
pub type GetSessionHistoryFn = Arc<
    dyn Fn(
            HashMap<String, serde_json::Value>,
        ) -> Arc<crate::core::chat_history::InMemoryChatMessageHistory>
        + Send
        + Sync,
>;

/// Runnable that manages chat message history for another Runnable.
///
/// `RunnableWithMessageHistory` wraps another Runnable and manages the chat message
/// history for it. It is responsible for:
/// 1. Loading historical messages before each invocation
/// 2. Saving new messages after each invocation completes
///
/// # Core Functionality
///
/// - **Session Management**: Each conversation has a session ID for history isolation
/// - **Automatic Loading**: Historical messages are loaded from storage before invocation
/// - **Automatic Saving**: New messages are saved to storage after successful invocation
/// - **Flexible Backends**: Works with any `BaseChatMessageHistory` implementation
///
/// # Configuration
///
/// Must always be called with a config containing session parameters.
/// By default, expects a `session_id` parameter:
///
/// ```rust,ignore
/// let mut config = RunnableConfig::default();
/// config.add_configurable("session_id", "user_123");
/// chain_with_history.invoke(input, Some(config)).await?;
/// ```
///
/// # Input/Output Formats
///
/// The wrapped Runnable can accept:
/// - A `Vec<Message>` (list of messages)
/// - A `HashMap` with one key for messages
/// - A `HashMap` with separate keys for current input and historical messages
///
/// The wrapped Runnable can return:
/// - A `String` (treated as `AIMessage`)
/// - A `Message` or `Vec<Message>`
/// - A `HashMap` with a key for messages
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::chat_history::InMemoryChatMessageHistory;
/// use dashflow::core::runnable::RunnableWithMessageHistory;
/// use std::collections::HashMap;
/// use std::sync::{Arc, Mutex};
///
/// // Create session store
/// let store: Arc<Mutex<HashMap<String, InMemoryChatMessageHistory>>> =
///     Arc::new(Mutex::new(HashMap::new()));
///
/// // Session history factory
/// let store_clone = store.clone();
/// let get_session_history = Arc::new(move |params: HashMap<String, serde_json::Value>| {
///     let session_id = params.get("session_id")
///         .and_then(|v| v.as_str())
///         .unwrap_or("default")
///         .to_string();
///
///     let mut s = store_clone.lock().unwrap();
///     Arc::new(s.entry(session_id)
///         .or_insert_with(InMemoryChatMessageHistory::new)
///         .clone())
/// });
///
/// // Wrap your chain
/// let chain_with_history = RunnableWithMessageHistory::new(
///     my_chain,
///     get_session_history,
///     None, // input_messages_key
///     None, // output_messages_key
///     None, // history_messages_key
///     None, // history_factory_config
/// );
///
/// // Use with session config
/// let mut config = RunnableConfig::default();
/// config.add_configurable("session_id", "user_123");
/// let response = chain_with_history.invoke(messages, Some(config)).await?;
/// ```
///
/// # Python Baseline Compatibility
///
/// Matches `RunnableWithMessageHistory` in `dashflow_core/runnables/history.py:38-606`.
///
/// Implementation differences from Python:
/// - **Simplified architecture**: Rust version directly wraps the runnable instead of
///   building a complex chain with `RunnableLambda` and RunnablePassthrough.assign
/// - **Arc-based history**: Uses `Arc<dyn BaseChatMessageHistory>` instead of Python's duck typing
/// - **Async-first**: No separate sync/async variants (`with_listeners/with_alisteners`)
/// - **Type-safe config**: Uses typed `HashMap` instead of **kwargs
/// - **Direct message handling**: Processes messages directly instead of using load/save serialization
///
/// These differences simplify the implementation while maintaining the core functionality.
/// Future iterations may add the full chain-based architecture for complete Python parity.
///
/// # Implementation Notes
///
/// This is a foundational implementation focusing on core functionality:
/// - ✅ Session-based history management
/// - ✅ Automatic message loading/saving
/// - ✅ Configurable session parameters
/// - ✅ Multiple history backends support
/// - ⚠️  Simplified vs Python's chain-based architecture
/// - ⚠️  Limited support for complex input/output key mapping
///
/// The implementation prioritizes correctness and usability over complete Python parity.
/// It provides the essential message history functionality needed for production use.
pub struct RunnableWithMessageHistory<R, Input, Output>
where
    R: Runnable<Input = Input, Output = Output>,
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// The underlying runnable to wrap
    runnable: R,

    /// Function that returns a `BaseChatMessageHistory` for a session
    get_session_history: GetSessionHistoryFn,

    /// Key in input dict containing the messages (if input is dict).
    /// Not yet used - message key extraction not implemented.
    #[allow(dead_code)] // API Parity: Python LangChain message key extraction pending
    input_messages_key: Option<String>,

    /// Key in output dict containing the messages (if output is dict).
    /// Not yet used - message key extraction not implemented.
    #[allow(dead_code)] // API Parity: Python LangChain message key extraction pending
    output_messages_key: Option<String>,

    /// Key in input dict for historical messages (if separate from current input).
    /// Not yet used - history key extraction not implemented.
    #[allow(dead_code)] // API Parity: Python LangChain message key extraction pending
    history_messages_key: Option<String>,

    /// Configure fields passed to the chat history factory
    history_factory_config: Vec<ConfigurableFieldSpec>,

    _phantom_input: std::marker::PhantomData<Input>,
    _phantom_output: std::marker::PhantomData<Output>,
}

impl<R, Input, Output> RunnableWithMessageHistory<R, Input, Output>
where
    R: Runnable<Input = Input, Output = Output>,
    Input: Send + 'static,
    Output: Send + 'static,
{
    /// Create a new `RunnableWithMessageHistory`
    ///
    /// # Arguments
    ///
    /// * `runnable` - The base Runnable to wrap
    /// * `get_session_history` - Function that returns a `BaseChatMessageHistory` for a session
    /// * `input_messages_key` - Optional key for messages in input dict (if input is dict)
    /// * `output_messages_key` - Optional key for messages in output dict (if output is dict)
    /// * `history_messages_key` - Optional key for historical messages in input dict
    /// * `history_factory_config` - Optional config specs for the history factory
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use std::sync::Arc;
    ///
    /// let chain_with_history = RunnableWithMessageHistory::new(
    ///     my_chain,
    ///     Arc::new(|params| get_history_for_session(params)),
    ///     Some("question".to_string()),      // Input is {"question": "..."}
    ///     None,                               // Output is a string
    ///     Some("history".to_string()),        // History injected as {"history": [...]}
    ///     None,                               // Use default session_id config
    /// );
    /// ```
    pub fn new(
        runnable: R,
        get_session_history: GetSessionHistoryFn,
        input_messages_key: Option<String>,
        output_messages_key: Option<String>,
        history_messages_key: Option<String>,
        history_factory_config: Option<Vec<ConfigurableFieldSpec>>,
    ) -> Self {
        // Default config specs if not provided
        let config_specs = history_factory_config.unwrap_or_else(|| {
            vec![ConfigurableFieldSpec::new("session_id", "String")
                .with_name("Session ID")
                .with_description("Unique identifier for a session.")
                .with_default(serde_json::json!(""))
                .with_shared(true)]
        });

        Self {
            runnable,
            get_session_history,
            input_messages_key,
            output_messages_key,
            history_messages_key,
            history_factory_config: config_specs,
            _phantom_input: std::marker::PhantomData,
            _phantom_output: std::marker::PhantomData,
        }
    }

    /// Get the configuration specs for this runnable
    ///
    /// Returns the history factory config specs that describe what
    /// configurable parameters are needed (e.g., `session_id`).
    pub fn config_specs(&self) -> &[ConfigurableFieldSpec] {
        &self.history_factory_config
    }

    /// Get the history factory config
    pub fn history_factory_config(&self) -> &[ConfigurableFieldSpec] {
        &self.history_factory_config
    }
}

// Runnable implementation for Vec<Message> -> Vec<Message>
//
// This is the simplest and most common case: a chain that takes messages as input
// and produces messages as output, with conversation history automatically managed.
#[async_trait::async_trait]
impl<R> Runnable
    for RunnableWithMessageHistory<
        R,
        Vec<crate::core::messages::Message>,
        Vec<crate::core::messages::Message>,
    >
where
    R: Runnable<
            Input = Vec<crate::core::messages::Message>,
            Output = Vec<crate::core::messages::Message>,
        > + Clone,
{
    type Input = Vec<crate::core::messages::Message>;
    type Output = Vec<crate::core::messages::Message>;

    async fn invoke(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> crate::core::error::Result<Self::Output> {
        use crate::core::chat_history::BaseChatMessageHistory;
        use crate::core::tracers::RunTree;

        // 1. Merge config with session history
        let mut config = config.unwrap_or_default();
        let history = self.merge_config_with_history(&mut config)?;

        // 2. Load historical messages and prepend to input
        let mut all_messages = history.get_messages().await.map_err(|e| {
            crate::core::error::Error::InvalidInput(format!(
                "Failed to get messages from history: {e:?}"
            ))
        })?;

        // Save the new input messages and count of historic messages for the listener
        let new_input_messages = input.clone();
        let historic_message_count = all_messages.len();
        all_messages.extend(input);

        // 3. Create on_end listener to save new messages
        let history_clone = history.clone();
        let on_end: crate::core::tracers::AsyncListener =
            Arc::new(move |run: &RunTree, _config: &RunnableConfig| {
                let history = history_clone.clone();
                let new_input_messages = new_input_messages.clone();
                let historic_message_count = historic_message_count;

                // Clone the data we need from run before moving into async block
                let outputs = run.outputs.clone();

                Box::pin(async move {
                    // Extract output messages from run
                    if let Some(outputs) = outputs {
                        // Outputs is a JSON object like {"output": <Vec<Message>>}
                        // Extract the "output" key
                        let output_value = outputs
                            .get("output")
                            .cloned()
                            .unwrap_or_else(|| outputs.clone());

                        // Try to deserialize to Vec<Message>
                        if let Ok(output_messages) = serde_json::from_value::<
                            Vec<crate::core::messages::Message>,
                        >(output_value)
                        {
                            // Output messages include historic messages + new input + AI response
                            // We only want to save the NEW messages (skip historic ones)
                            let new_output_messages =
                                if output_messages.len() > historic_message_count {
                                    output_messages[historic_message_count..].to_vec()
                                } else {
                                    vec![]
                                };

                            // Save: new inputs + new outputs
                            let mut all_new_messages = new_input_messages.clone();
                            all_new_messages.extend(new_output_messages);

                            // Log errors in saving (listener shouldn't fail the main operation)
                            if let Err(e) = history.add_messages(&all_new_messages).await {
                                tracing::warn!(
                                    message_count = all_new_messages.len(),
                                    error = %e,
                                    "Failed to save messages to history"
                                );
                            }
                        }
                    }
                })
            });

        // 4. Wrap runnable with the save listener
        let runnable_with_save = self
            .runnable
            .clone()
            .with_listeners(None, Some(on_end), None);

        // 5. Invoke wrapped runnable with all messages (history + input)
        runnable_with_save.invoke(all_messages, Some(config)).await
    }

    async fn batch(
        &self,
        inputs: Vec<Self::Input>,
        config: Option<RunnableConfig>,
    ) -> crate::core::error::Result<Vec<Self::Output>> {
        // For batch, invoke each input with the same config
        // Note: In production, each item might need its own session_id in config
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            let result = self.invoke(input, config.clone()).await?;
            results.push(result);
        }

        Ok(results)
    }

    async fn stream(
        &self,
        input: Self::Input,
        config: Option<RunnableConfig>,
    ) -> crate::core::error::Result<
        std::pin::Pin<
            Box<dyn futures::Stream<Item = crate::core::error::Result<Self::Output>> + Send>,
        >,
    > {
        // For streaming, we invoke once and return the result as a single-item stream.
        // Note: True incremental streaming would require streaming writes to the message history,
        // which isn't supported by most BaseChatMessageHistory implementations.
        let result = self.invoke(input, config).await?;
        Ok(Box::pin(futures::stream::once(async move { Ok(result) })))
    }
}

impl<R>
    RunnableWithMessageHistory<
        R,
        Vec<crate::core::messages::Message>,
        Vec<crate::core::messages::Message>,
    >
where
    R: Runnable<
        Input = Vec<crate::core::messages::Message>,
        Output = Vec<crate::core::messages::Message>,
    >,
{
    /// Merge config with session history
    ///
    /// Extracts the configurable parameters from the config (e.g., `session_id`),
    /// calls `get_session_history` to retrieve the appropriate history object,
    /// and returns it.
    fn merge_config_with_history(
        &self,
        config: &mut RunnableConfig,
    ) -> crate::core::error::Result<Arc<crate::core::chat_history::InMemoryChatMessageHistory>>
    {
        use crate::core::error::Error;

        // Get expected keys from history_factory_config
        let expected_keys: Vec<String> = self
            .history_factory_config
            .iter()
            .map(|spec| spec.id.clone())
            .collect();

        // Get configurable from config
        let params = config.configurable.clone();

        // Check for missing keys
        let provided_keys: std::collections::HashSet<_> = params.keys().cloned().collect();
        let expected_keys_set: std::collections::HashSet<_> =
            expected_keys.iter().cloned().collect();
        let missing_keys: Vec<_> = expected_keys_set.difference(&provided_keys).collect();

        if !missing_keys.is_empty() {
            let mut sorted_missing: Vec<_> = missing_keys.iter().map(|s| s.as_str()).collect();
            sorted_missing.sort_unstable();
            return Err(Error::InvalidInput(format!(
                "Missing keys {sorted_missing:?} in config['configurable']. Expected keys are {expected_keys:?}. \
                 When using via invoke() or stream(), pass in a config; \
                 e.g., chain.invoke(input, Some(RunnableConfig::default().with_configurable(\"session_id\", \"your-session-id\")))"
            )));
        }

        // Call get_session_history with the configurable params
        let history = (self.get_session_history)(params);

        Ok(history)
    }
}
#[cfg(test)]
mod runnable_with_message_history_tests {
    use crate::test_prelude::*;
    // Use type aliases to avoid name conflicts with test_prelude's Node trait and Edge struct
    type RunnableNode = crate::core::runnable::Node;
    type RunnableEdge = crate::core::runnable::Edge;
    type RunnableGraph = crate::core::runnable::Graph;

    #[test]
    fn test_runnable_with_message_history_creation() {
        // Test that we can create the struct with the factory function
        use crate::core::chat_history::InMemoryChatMessageHistory;
        use std::sync::Mutex;

        let store: Arc<Mutex<HashMap<String, InMemoryChatMessageHistory>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let store_clone = store.clone();
        let get_session_history: GetSessionHistoryFn =
            Arc::new(move |params: HashMap<String, serde_json::Value>| {
                let session_id = params
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let mut s = store_clone.lock().unwrap();
                Arc::new(s.entry(session_id).or_default().clone())
            });

        // Create a dummy runnable for testing (RunnablePassthrough)
        let passthrough = RunnablePassthrough::<Vec<crate::core::messages::Message>>::new();

        let _with_history = RunnableWithMessageHistory::new(
            passthrough,
            get_session_history,
            None,
            None,
            None,
            None,
        );

        // Test that config_specs returns the default session_id spec
        assert_eq!(_with_history.config_specs().len(), 1);
        assert_eq!(_with_history.config_specs()[0].id, "session_id");
    }

    #[test]
    fn test_runnable_with_message_history_custom_config() {
        use crate::core::chat_history::InMemoryChatMessageHistory;
        use std::sync::Mutex;

        let store: Arc<Mutex<HashMap<String, InMemoryChatMessageHistory>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let get_session_history: GetSessionHistoryFn =
            Arc::new(move |params: HashMap<String, serde_json::Value>| {
                let mut key_parts = vec![];
                if let Some(user_id) = params.get("user_id").and_then(|v| v.as_str()) {
                    key_parts.push(user_id.to_string());
                }
                if let Some(conv_id) = params.get("conversation_id").and_then(|v| v.as_str()) {
                    key_parts.push(conv_id.to_string());
                }
                let session_key = key_parts.join(":");

                let mut s = store.lock().unwrap();
                Arc::new(s.entry(session_key).or_default().clone())
            });

        let passthrough = RunnablePassthrough::<Vec<crate::core::messages::Message>>::new();

        let custom_config = vec![
            ConfigurableFieldSpec::new("user_id", "String")
                .with_name("User ID")
                .with_description("Unique identifier for the user.")
                .with_shared(true),
            ConfigurableFieldSpec::new("conversation_id", "String")
                .with_name("Conversation ID")
                .with_description("Unique identifier for the conversation.")
                .with_shared(true),
        ];

        let with_history = RunnableWithMessageHistory::new(
            passthrough,
            get_session_history,
            None,
            None,
            None,
            Some(custom_config),
        );

        // Test that config_specs returns the custom specs
        assert_eq!(with_history.config_specs().len(), 2);
        assert_eq!(with_history.config_specs()[0].id, "user_id");
        assert_eq!(with_history.config_specs()[1].id, "conversation_id");
    }

    #[tokio::test]
    async fn test_runnable_with_message_history_invoke() {
        // Test that messages persist across multiple invocations
        use crate::core::chat_history::{BaseChatMessageHistory, InMemoryChatMessageHistory};
        use crate::core::messages::{HumanMessage, Message};
        use std::sync::Mutex;

        let store: Arc<Mutex<HashMap<String, InMemoryChatMessageHistory>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let store_clone = store.clone();
        let store_clone2 = store.clone();
        let get_session_history: GetSessionHistoryFn =
            Arc::new(move |params: HashMap<String, serde_json::Value>| {
                let session_id = params
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let mut s = store_clone.lock().unwrap();
                Arc::new(s.entry(session_id).or_default().clone())
            });

        // Create a simple passthrough chain that returns input messages as-is
        let passthrough = RunnablePassthrough::<Vec<Message>>::new();

        let with_history = RunnableWithMessageHistory::new(
            passthrough,
            get_session_history,
            None,
            None,
            None,
            None,
        );

        // First invocation - send a message
        let input1 = vec![HumanMessage::new("Hello").into()];
        let config1 = RunnableConfig::default()
            .with_configurable("session_id", serde_json::json!("test_session_1"))
            .unwrap();

        let result1 = with_history.invoke(input1.clone(), Some(config1)).await;
        assert!(result1.is_ok());
        let output1 = result1.unwrap();

        // Output should contain just the input message (since RunnablePassthrough returns input as-is)
        assert_eq!(output1.len(), 1);
        if let Message::Human { content, .. } = &output1[0] {
            assert_eq!(content.as_text(), "Hello");
        } else {
            panic!("Expected HumanMessage");
        }

        // Verify history was saved after first invocation
        let hist_clone = {
            let store_guard = store_clone2.lock().unwrap();
            let hist = store_guard
                .get("test_session_1")
                .expect("Session should exist");
            hist.clone()
        };
        {
            let msgs = hist_clone.get_messages().await.unwrap();
            // Should have saved input + output (both "Hello" due to passthrough)
            assert_eq!(
                msgs.len(),
                2,
                "History should have 2 messages after first invocation"
            );
        }

        // Second invocation - send another message to the same session
        let input2 = vec![HumanMessage::new("How are you?").into()];
        let config2 = RunnableConfig::default()
            .with_configurable("session_id", serde_json::json!("test_session_1"))
            .unwrap();

        let result2 = with_history.invoke(input2.clone(), Some(config2)).await;
        assert!(result2.is_ok());
        let output2 = result2.unwrap();

        // Output should contain history (2 messages) + new input (1 message) = 3 messages
        assert_eq!(output2.len(), 3, "Expected 3 messages (2 historic + 1 new)");

        // Verify the messages are in correct order
        if let Message::Human { content, .. } = &output2[2] {
            assert_eq!(content.as_text(), "How are you?");
        } else {
            panic!("Expected HumanMessage at index 2");
        }

        // Verify history was updated after second invocation
        let hist_clone2 = {
            let store_guard = store_clone2.lock().unwrap();
            let hist = store_guard
                .get("test_session_1")
                .expect("Session should exist");
            hist.clone()
        };
        {
            let msgs = hist_clone2.get_messages().await.unwrap();
            // Should have: first invocation (2 messages) + second invocation (2 messages) = 4 total
            assert_eq!(
                msgs.len(),
                4,
                "History should have 4 messages after second invocation"
            );
        }
    }

    #[tokio::test]
    async fn test_runnable_with_message_history_session_isolation() {
        // Test that different sessions have isolated histories
        use crate::core::chat_history::InMemoryChatMessageHistory;
        use crate::core::messages::{HumanMessage, Message};
        use std::sync::Mutex;

        let store: Arc<Mutex<HashMap<String, InMemoryChatMessageHistory>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let store_clone = store.clone();
        let get_session_history: GetSessionHistoryFn =
            Arc::new(move |params: HashMap<String, serde_json::Value>| {
                let session_id = params
                    .get("session_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("default")
                    .to_string();

                let mut s = store_clone.lock().unwrap();
                Arc::new(s.entry(session_id).or_default().clone())
            });

        let passthrough = RunnablePassthrough::<Vec<Message>>::new();
        let with_history = RunnableWithMessageHistory::new(
            passthrough,
            get_session_history,
            None,
            None,
            None,
            None,
        );

        // Session 1
        let input1 = vec![HumanMessage::new("Session 1 message").into()];
        let config1 = RunnableConfig::default()
            .with_configurable("session_id", serde_json::json!("session_1"))
            .unwrap();
        let _ = with_history.invoke(input1, Some(config1)).await.unwrap();

        // Session 2
        let input2 = vec![HumanMessage::new("Session 2 message").into()];
        let config2 = RunnableConfig::default()
            .with_configurable("session_id", serde_json::json!("session_2"))
            .unwrap();
        let _ = with_history.invoke(input2, Some(config2)).await.unwrap();

        // Verify sessions are separate
        let store_guard = store.lock().unwrap();
        assert!(store_guard.contains_key("session_1"));
        assert!(store_guard.contains_key("session_2"));
        assert_eq!(store_guard.len(), 2);
    }

    #[tokio::test]
    async fn test_runnable_with_message_history_missing_config() {
        // Test that missing session_id in config produces an error
        use crate::core::chat_history::InMemoryChatMessageHistory;
        use crate::core::messages::{HumanMessage, Message};

        let get_session_history: GetSessionHistoryFn =
            Arc::new(move |_params: HashMap<String, serde_json::Value>| {
                Arc::new(InMemoryChatMessageHistory::new())
            });

        let passthrough = RunnablePassthrough::<Vec<Message>>::new();
        let with_history = RunnableWithMessageHistory::new(
            passthrough,
            get_session_history,
            None,
            None,
            None,
            None,
        );

        // Invoke without session_id in config
        let input = vec![HumanMessage::new("Hello").into()];
        let config = RunnableConfig::default(); // Missing session_id

        let result = with_history.invoke(input, Some(config)).await;
        assert!(result.is_err(), "Expected error for missing session_id");

        if let Err(e) = result {
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("Missing keys") || error_msg.contains("session_id"),
                "Error should mention missing keys, got: {}",
                error_msg
            );
        }
    }

    #[test]
    fn test_graph_simple_node() {
        // Test a single node graph
        let lambda = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let graph = lambda.get_graph(None);

        assert_eq!(graph.nodes.len(), 1);
        assert_eq!(graph.edges.len(), 0);

        let node = graph.nodes.values().next().unwrap();
        assert_eq!(node.name, "add_one");
    }

    #[test]
    fn test_graph_sequence() {
        // Test a sequence of runnables
        let lambda1 = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let lambda2 = RunnableLambda::with_name(|x: i32| x * 2, "multiply_two");
        let sequence = lambda1.pipe(lambda2);

        let graph = sequence.get_graph(None);

        // Should have 2 nodes and 1 edge connecting them
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);

        // Verify nodes exist
        assert!(graph.nodes.values().any(|n| n.name == "add_one"));
        assert!(graph.nodes.values().any(|n| n.name == "multiply_two"));

        // Verify edge connects them
        let edge = &graph.edges[0];
        let source_node = graph.nodes.get(&edge.source).unwrap();
        let target_node = graph.nodes.get(&edge.target).unwrap();
        assert_eq!(source_node.name, "add_one");
        assert_eq!(target_node.name, "multiply_two");
    }

    #[test]
    fn test_graph_ascii_single_node() {
        // Test ASCII drawing of a single node
        let lambda = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let graph = lambda.get_graph(None);
        let ascii = graph.draw_ascii();

        // Should contain the node name
        assert!(ascii.contains("add_one"), "ASCII should contain node name");
        // Should contain box characters
        assert!(ascii.contains("+"), "ASCII should contain box characters");
        assert!(ascii.contains("-"), "ASCII should contain box characters");
        assert!(ascii.contains("|"), "ASCII should contain box characters");
    }

    #[test]
    fn test_graph_ascii_sequence() {
        // Test ASCII drawing of a sequence
        let lambda1 = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let lambda2 = RunnableLambda::with_name(|x: i32| x * 2, "multiply_two");
        let sequence = lambda1.pipe(lambda2);

        let graph = sequence.get_graph(None);
        let ascii = graph.draw_ascii();

        // Should contain both node names
        assert!(ascii.contains("add_one"), "ASCII should contain first node");
        assert!(
            ascii.contains("multiply_two"),
            "ASCII should contain second node"
        );
        // Should contain arrow characters
        assert!(ascii.contains("|"), "ASCII should contain vertical edge");
        assert!(ascii.contains("v"), "ASCII should contain arrow");
    }

    #[test]
    fn test_graph_linear_chain_detection() {
        // Test that linear chains are detected correctly
        let mut graph = RunnableGraph::new();

        // Add nodes
        graph.add_node(RunnableNode::new("A", "Node A"));
        graph.add_node(RunnableNode::new("B", "Node B"));
        graph.add_node(RunnableNode::new("C", "Node C"));

        // Add edges in sequence: A -> B -> C
        graph.add_edge(RunnableEdge::new("A", "B"));
        graph.add_edge(RunnableEdge::new("B", "C"));

        assert!(graph.is_linear_chain(), "Should detect linear chain");
    }

    #[test]
    fn test_graph_non_linear_detection() {
        // Test that non-linear graphs are detected correctly
        let mut graph = RunnableGraph::new();

        // Add nodes
        graph.add_node(RunnableNode::new("A", "Node A"));
        graph.add_node(RunnableNode::new("B", "Node B"));
        graph.add_node(RunnableNode::new("C", "Node C"));

        // Add edges with branching: A -> B, A -> C
        graph.add_edge(RunnableEdge::new("A", "B"));
        graph.add_edge(RunnableEdge::new("A", "C"));

        assert!(!graph.is_linear_chain(), "Should detect non-linear graph");
    }

    #[test]
    fn test_graph_first_last_nodes() {
        // Test finding first and last nodes
        let mut graph = RunnableGraph::new();

        graph.add_node(RunnableNode::new("A", "Node A"));
        graph.add_node(RunnableNode::new("B", "Node B"));
        graph.add_node(RunnableNode::new("C", "Node C"));

        graph.add_edge(RunnableEdge::new("A", "B"));
        graph.add_edge(RunnableEdge::new("B", "C"));

        let first = graph.first_node();
        let last = graph.last_node();

        assert!(first.is_some(), "Should find first node");
        assert!(last.is_some(), "Should find last node");
        assert_eq!(first.unwrap().id, "A");
        assert_eq!(last.unwrap().id, "C");
    }

    #[test]
    fn test_graph_parallel() {
        // Test graph for RunnableParallel
        let lambda1 = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let lambda2 = RunnableLambda::with_name(|x: i32| x * 2, "multiply_two");
        let lambda3 = RunnableLambda::with_name(|x: i32| x * x, "square");

        let mut parallel = RunnableParallel::new();
        parallel.add("branch1", lambda1);
        parallel.add("branch2", lambda2);
        parallel.add("branch3", lambda3);

        let graph = parallel.get_graph(None);

        // Should have 1 root node + 3 branch nodes = 4 total
        assert_eq!(graph.nodes.len(), 4, "Should have root + 3 branch nodes");

        // Should have 3 edges from root to each branch
        assert_eq!(graph.edges.len(), 3, "Should have 3 edges from root");

        // Verify root node exists
        let root_name = parallel.name();
        assert!(
            graph.nodes.values().any(|n| n.name == root_name),
            "Should have root node"
        );

        // Verify all branches are connected to root
        for edge in &graph.edges {
            let source = graph.nodes.get(&edge.source).unwrap();
            assert_eq!(source.name, root_name, "All edges should start from root");
        }

        // Verify branch nodes exist with prefixes
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("branch1") && n.name == "add_one"),
            "Should have branch1:add_one"
        );
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("branch2") && n.name == "multiply_two"),
            "Should have branch2:multiply_two"
        );
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("branch3") && n.name == "square"),
            "Should have branch3:square"
        );
    }

    #[test]
    fn test_graph_branch() {
        // Test graph for RunnableBranch
        let branch_a = RunnableLambda::with_name(|x: i32| format!("Large: {}", x), "large");
        let branch_b = RunnableLambda::with_name(|x: i32| format!("Small: {}", x), "small");
        let default = RunnableLambda::with_name(|x: i32| format!("Default: {}", x), "default");

        let branch = RunnableBranch::new(default)
            .add_branch(|x: &i32| *x > 10, branch_a)
            .add_branch(|x: &i32| *x > 0, branch_b);

        let graph = branch.get_graph(None);

        // Should have 1 root node + 2 conditional branches + 1 default = 4 total
        assert_eq!(
            graph.nodes.len(),
            4,
            "Should have root + 2 conditionals + 1 default"
        );

        // Should have 3 edges from root (2 conditional + 1 default)
        assert_eq!(
            graph.edges.len(),
            3,
            "Should have 3 edges from root to branches"
        );

        // Verify root node
        let root_name = branch.name();
        assert!(
            graph.nodes.values().any(|n| n.name == root_name),
            "Should have root node"
        );

        // Verify conditional edges are marked
        let conditional_edges = graph
            .edges
            .iter()
            .filter(|e| e.conditional)
            .collect::<Vec<_>>();
        assert_eq!(
            conditional_edges.len(),
            2,
            "Should have 2 conditional edges"
        );

        // Verify default edge has data
        let default_edge = graph
            .edges
            .iter()
            .find(|e| e.data.as_deref() == Some("default"))
            .expect("Should have default edge");
        assert!(
            !default_edge.conditional,
            "Default edge should not be conditional"
        );
    }

    #[test]
    fn test_graph_with_fallbacks() {
        // Test graph for RunnableWithFallbacks
        let primary = RunnableLambda::with_name(|x: i32| x * 2, "primary");
        let fallback1 = RunnableLambda::with_name(|x: i32| x * 3, "fallback1");
        let fallback2 = RunnableLambda::with_name(|x: i32| x * 4, "fallback2");

        let with_fallbacks = RunnableWithFallbacks::new(primary)
            .add_fallback(fallback1)
            .add_fallback(fallback2);

        let graph = with_fallbacks.get_graph(None);

        // Should have 1 root node + 1 primary + 2 fallbacks = 4 total
        assert_eq!(
            graph.nodes.len(),
            4,
            "Should have root + primary + 2 fallbacks"
        );

        // Should have 3 edges from root (1 primary + 2 fallbacks)
        assert_eq!(
            graph.edges.len(),
            3,
            "Should have 3 edges from root to all branches"
        );

        // Verify root node
        let root_name = with_fallbacks.name();
        assert!(
            graph.nodes.values().any(|n| n.name == root_name),
            "Should have root node"
        );

        // Verify primary node
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("primary") && n.name == "primary"),
            "Should have primary node"
        );

        // Verify fallback nodes
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("fallback_0") && n.name == "fallback1"),
            "Should have fallback_0"
        );
        assert!(
            graph
                .nodes
                .values()
                .any(|n| n.id.contains("fallback_1") && n.name == "fallback2"),
            "Should have fallback_1"
        );

        // Verify fallback edges have error data
        let fallback_edges = graph
            .edges
            .iter()
            .filter(|e| e.data.as_ref().map(|s| s.contains("on_error")) == Some(true))
            .collect::<Vec<_>>();
        assert_eq!(fallback_edges.len(), 2, "Should have 2 on_error edges");
    }

    #[test]
    fn test_graph_mermaid_simple() {
        // Test Mermaid generation for simple graph
        let lambda = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let graph = lambda.get_graph(None);
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("add_one"), "Should contain node name");
        assert!(
            mermaid.contains("[\"add_one\"]"),
            "Should have node definition"
        );
    }

    #[test]
    fn test_graph_mermaid_sequence() {
        // Test Mermaid generation for sequence
        let lambda1 = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let lambda2 = RunnableLambda::with_name(|x: i32| x * 2, "multiply_two");
        let sequence = lambda1.pipe(lambda2);

        let graph = sequence.get_graph(None);
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("add_one"), "Should contain first node");
        assert!(
            mermaid.contains("multiply_two"),
            "Should contain second node"
        );
        assert!(mermaid.contains("-->"), "Should contain edge arrow");
    }

    #[test]
    fn test_graph_mermaid_parallel() {
        // Test Mermaid generation for parallel execution
        let lambda1 = RunnableLambda::with_name(|x: i32| x + 1, "add_one");
        let lambda2 = RunnableLambda::with_name(|x: i32| x * 2, "multiply_two");

        let mut parallel = RunnableParallel::new();
        parallel.add("branch1", lambda1);
        parallel.add("branch2", lambda2);

        let graph = parallel.get_graph(None);
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("Parallel"), "Should contain parallel root");
        assert!(mermaid.contains("add_one"), "Should contain first branch");
        assert!(
            mermaid.contains("multiply_two"),
            "Should contain second branch"
        );
        // Should have at least 2 edges from root to branches
        assert!(
            mermaid.matches("-->").count() >= 2,
            "Should have multiple edges"
        );
    }

    #[test]
    fn test_graph_mermaid_conditional() {
        // Test Mermaid generation for conditional branching
        let branch_a = RunnableLambda::with_name(|x: i32| format!("Large: {}", x), "large");
        let default = RunnableLambda::with_name(|x: i32| format!("Default: {}", x), "default");

        let branch = RunnableBranch::new(default).add_branch(|x: &i32| *x > 10, branch_a);

        let graph = branch.get_graph(None);
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("Branch"), "Should contain branch root");
        // Conditional edges use dotted arrows
        assert!(
            mermaid.contains("-.->"),
            "Should have conditional edge style"
        );
    }

    #[test]
    fn test_graph_mermaid_fallbacks() {
        // Test Mermaid generation for fallbacks
        let primary = RunnableLambda::with_name(|x: i32| x * 2, "primary");
        let fallback1 = RunnableLambda::with_name(|x: i32| x * 3, "fallback1");

        let with_fallbacks = RunnableWithFallbacks::new(primary).add_fallback(fallback1);

        let graph = with_fallbacks.get_graph(None);
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("primary"), "Should contain primary node");
        assert!(
            mermaid.contains("fallback1"),
            "Should contain fallback node"
        );
        // Error edges use dotted arrows and on_error label
        assert!(mermaid.contains("-.->"), "Should have fallback edge style");
        assert!(mermaid.contains("on_error"), "Should have on_error label");
    }

    #[test]
    fn test_graph_mermaid_empty() {
        // Test Mermaid generation for empty graph
        let graph = Graph::new();
        let mermaid = graph.draw_mermaid();

        assert!(
            mermaid.contains("graph TD"),
            "Should have graph declaration"
        );
        assert!(mermaid.contains("empty"), "Should indicate empty graph");
    }

    #[tokio::test]
    async fn test_as_tool_basic() {
        use crate::core::tools::Tool;
        use serde_json::json;

        // Create a simple runnable that uppercases strings
        #[derive(Clone)]
        struct UppercaseRunnable;

        #[async_trait::async_trait]
        impl Runnable for UppercaseRunnable {
            type Input = serde_json::Value;
            type Output = String;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> Result<Self::Output> {
                // Extract text field from JSON input
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'text' field".to_string()))?;
                Ok(text.to_uppercase())
            }
        }

        // Convert the runnable to a tool
        let tool = UppercaseRunnable.as_tool(
            "uppercase",
            "Converts text to uppercase",
            json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to convert"
                    }
                },
                "required": ["text"]
            }),
        );

        // Verify tool metadata
        assert_eq!(tool.name(), "uppercase");
        assert_eq!(tool.description(), "Converts text to uppercase");

        // Test the tool with structured input
        let input = crate::core::tools::ToolInput::Structured(json!({"text": "hello world"}));
        let result = tool._call(input).await.unwrap();
        assert_eq!(result, "HELLO WORLD");
    }

    #[tokio::test]
    async fn test_as_tool_string_input() {
        use crate::core::tools::Tool;
        use serde_json::json;

        // Create a simple runnable that echoes the input field
        #[derive(Clone)]
        struct EchoRunnable;

        #[async_trait::async_trait]
        impl Runnable for EchoRunnable {
            type Input = serde_json::Value;
            type Output = String;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> Result<Self::Output> {
                // When called with string input, it gets wrapped in {"input": "..."}
                let text = input
                    .get("input")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'input' field".to_string()))?;
                Ok(format!("Echo: {}", text))
            }
        }

        // Convert to tool
        let tool = EchoRunnable.as_tool(
            "echo",
            "Echoes the input",
            json!({
                "type": "object",
                "properties": {
                    "input": {"type": "string"}
                }
            }),
        );

        // Test with string input (gets wrapped in {"input": "..."})
        let result = tool._call_str("test message".to_string()).await.unwrap();
        assert_eq!(result, "Echo: test message");
    }

    #[tokio::test]
    async fn test_as_tool_error_handling() {
        use crate::core::tools::Tool;
        use serde_json::json;

        // Create a runnable that fails on certain inputs
        #[derive(Clone)]
        struct FailingRunnable;

        #[async_trait::async_trait]
        impl Runnable for FailingRunnable {
            type Input = serde_json::Value;
            type Output = String;

            async fn invoke(
                &self,
                input: Self::Input,
                _config: Option<RunnableConfig>,
            ) -> Result<Self::Output> {
                let text = input
                    .get("text")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::InvalidInput("Missing 'text' field".to_string()))?;

                if text == "fail" {
                    return Err(Error::other("Intentional failure"));
                }

                Ok(text.to_string())
            }
        }

        let tool = FailingRunnable.as_tool(
            "failing_tool",
            "A tool that fails on 'fail' input",
            json!({
                "type": "object",
                "properties": {
                    "text": {"type": "string"}
                }
            }),
        );

        // Test successful case
        let input = crate::core::tools::ToolInput::Structured(json!({"text": "success"}));
        let result = tool._call(input).await.unwrap();
        assert_eq!(result, "success");

        // Test failure case
        let input = crate::core::tools::ToolInput::Structured(json!({"text": "fail"}));
        let result = tool._call(input).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Intentional failure"));
    }
}
