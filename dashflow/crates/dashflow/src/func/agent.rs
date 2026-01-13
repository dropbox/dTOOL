// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Agent trait for `DashFlow` Functional API
//!
//! Agents are the primary abstraction in the Functional API. They provide
//! `.invoke()` and `.stream()` methods for executing agent logic.

use crate::error::Result;
use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// An agent that can be invoked with input and produces output.
///
/// Agents are created by applying the `#[entrypoint]` macro to an async function.
/// They provide methods for synchronous (`.invoke()`) and streaming (`.stream()`)
/// execution.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_macros::entrypoint;
/// use dashflow::core::messages::Message;
///
/// #[entrypoint]
/// async fn simple_agent(messages: Vec<Message>) -> Result<Message, String> {
///     // Agent logic here
///     Ok(Message::ai("Response"))
/// }
///
/// // Usage
/// let result = simple_agent.invoke(messages).await?;
/// ```
#[async_trait]
pub trait Agent<I, O>: Send + Sync {
    /// Invoke the agent with the given input and wait for the final output.
    ///
    /// This method runs the agent to completion and returns the final output.
    ///
    /// # Errors
    ///
    /// Returns an error if the agent execution fails.
    async fn invoke(&self, input: I) -> Result<O>;

    /// Invoke the agent with the given input and stream intermediate updates.
    ///
    /// This method runs the agent and yields updates as the agent progresses
    /// through its execution. The final item in the stream is the complete output.
    ///
    /// # Errors
    ///
    /// Stream items may be errors if agent execution fails.
    async fn stream(
        &self,
        input: I,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate<O>>> + Send>>>;
}

/// An update from a streaming agent execution.
///
/// Stream updates can be intermediate state updates or the final output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StreamUpdate<O> {
    /// An intermediate update with partial state
    Update {
        /// The node that produced this update
        node: String,
        /// The updated output (may be partial)
        output: O,
    },
    /// The final output from the agent
    Final(O),
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;

    // Test that Agent trait is object-safe
    #[allow(dead_code, clippy::diverging_sub_expression)]
    fn _test_object_safety() {
        let _: Box<dyn Agent<String, String>> = unimplemented!();
    }

    // StreamUpdate tests

    #[test]
    fn test_stream_update_update_variant() {
        let update = StreamUpdate::Update {
            node: "test_node".to_string(),
            output: 42,
        };
        match update {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "test_node");
                assert_eq!(output, 42);
            }
            _ => panic!("Expected Update variant"),
        }
    }

    #[test]
    fn test_stream_update_final_variant() {
        let update = StreamUpdate::Final(100);
        match update {
            StreamUpdate::Final(output) => {
                assert_eq!(output, 100);
            }
            _ => panic!("Expected Final variant"),
        }
    }

    #[test]
    fn test_stream_update_clone() {
        let original = StreamUpdate::Update {
            node: "node1".to_string(),
            output: "data".to_string(),
        };
        let cloned = original.clone();
        match (original, cloned) {
            (
                StreamUpdate::Update {
                    node: n1,
                    output: o1,
                },
                StreamUpdate::Update {
                    node: n2,
                    output: o2,
                },
            ) => {
                assert_eq!(n1, n2);
                assert_eq!(o1, o2);
            }
            _ => panic!("Clone should preserve variant"),
        }
    }

    #[test]
    fn test_stream_update_debug_format() {
        let update = StreamUpdate::Update {
            node: "agent".to_string(),
            output: vec![1, 2, 3],
        };
        let debug_str = format!("{:?}", update);
        assert!(debug_str.contains("Update"));
        assert!(debug_str.contains("agent"));
    }

    #[test]
    fn test_stream_update_final_debug_format() {
        let update: StreamUpdate<&str> = StreamUpdate::Final("result");
        let debug_str = format!("{:?}", update);
        assert!(debug_str.contains("Final"));
        assert!(debug_str.contains("result"));
    }

    #[test]
    fn test_stream_update_serialization() {
        let update = StreamUpdate::Update {
            node: "test".to_string(),
            output: 123,
        };
        let json = serde_json::to_string(&update).expect("serialization should succeed");
        assert!(json.contains("Update"));
        assert!(json.contains("test"));
        assert!(json.contains("123"));
    }

    #[test]
    fn test_stream_update_deserialization() {
        let json = r#"{"Update":{"node":"test","output":123}}"#;
        let update: StreamUpdate<i32> =
            serde_json::from_str(json).expect("deserialization should succeed");
        match update {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "test");
                assert_eq!(output, 123);
            }
            _ => panic!("Expected Update variant"),
        }
    }

    #[test]
    fn test_stream_update_final_serialization() {
        let update: StreamUpdate<String> = StreamUpdate::Final("done".to_string());
        let json = serde_json::to_string(&update).expect("serialization should succeed");
        assert!(json.contains("Final"));
        assert!(json.contains("done"));
    }

    #[test]
    fn test_stream_update_final_deserialization() {
        let json = r#"{"Final":"done"}"#;
        let update: StreamUpdate<String> =
            serde_json::from_str(json).expect("deserialization should succeed");
        match update {
            StreamUpdate::Final(output) => {
                assert_eq!(output, "done");
            }
            _ => panic!("Expected Final variant"),
        }
    }

    #[test]
    fn test_stream_update_roundtrip() {
        let original = StreamUpdate::Update {
            node: "processor".to_string(),
            output: vec!["a", "b", "c"],
        };
        let json = serde_json::to_string(&original).expect("serialization should succeed");
        let deserialized: StreamUpdate<Vec<&str>> =
            serde_json::from_str(&json).expect("deserialization should succeed");

        match (original, deserialized) {
            (
                StreamUpdate::Update {
                    node: n1,
                    output: o1,
                },
                StreamUpdate::Update {
                    node: n2,
                    output: o2,
                },
            ) => {
                assert_eq!(n1, n2);
                assert_eq!(o1, o2);
            }
            _ => panic!("Roundtrip should preserve variant"),
        }
    }

    #[test]
    fn test_stream_update_with_empty_node_name() {
        let update = StreamUpdate::Update {
            node: String::new(),
            output: 0,
        };
        match update {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "");
                assert_eq!(output, 0);
            }
            _ => panic!("Expected Update variant"),
        }
    }

    #[test]
    fn test_stream_update_with_unicode_node_name() {
        let update = StreamUpdate::Update {
            node: "节点🚀".to_string(),
            output: true,
        };
        match update {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "节点🚀");
                assert!(output);
            }
            _ => panic!("Expected Update variant"),
        }
    }

    #[test]
    fn test_stream_update_with_complex_output() {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        struct ComplexData {
            id: u64,
            name: String,
            tags: Vec<String>,
        }

        let data = ComplexData {
            id: 42,
            name: "test".to_string(),
            tags: vec!["a".to_string(), "b".to_string()],
        };

        let update = StreamUpdate::Final(data.clone());
        match update {
            StreamUpdate::Final(output) => {
                assert_eq!(output, data);
            }
            _ => panic!("Expected Final variant"),
        }
    }

    // Agent trait implementation tests

    struct TestAgent {
        result: String,
    }

    #[async_trait]
    impl Agent<String, String> for TestAgent {
        async fn invoke(&self, _input: String) -> Result<String> {
            Ok(self.result.clone())
        }

        async fn stream(
            &self,
            _input: String,
        ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate<String>>> + Send>>> {
            let result = self.result.clone();
            Ok(Box::pin(stream::once(async move {
                Ok(StreamUpdate::Final(result))
            })))
        }
    }

    #[tokio::test]
    async fn test_agent_invoke() {
        let agent = TestAgent {
            result: "success".to_string(),
        };
        let result = agent.invoke("input".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
    }

    #[tokio::test]
    async fn test_agent_stream() {
        use futures::StreamExt;

        let agent = TestAgent {
            result: "streamed".to_string(),
        };
        let mut stream = agent.stream("input".to_string()).await.unwrap();

        let update = stream.next().await;
        assert!(update.is_some());
        let update = update.unwrap().unwrap();
        match update {
            StreamUpdate::Final(output) => {
                assert_eq!(output, "streamed");
            }
            _ => panic!("Expected Final variant"),
        }
    }

    #[test]
    fn test_agent_trait_bounds_send() {
        fn assert_send<T: Send>() {}
        assert_send::<TestAgent>();
    }

    #[test]
    fn test_agent_trait_bounds_sync() {
        fn assert_sync<T: Sync>() {}
        assert_sync::<TestAgent>();
    }

    #[tokio::test]
    async fn test_agent_boxed() {
        let agent: Box<dyn Agent<String, String>> = Box::new(TestAgent {
            result: "boxed".to_string(),
        });
        let result = agent.invoke("input".to_string()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "boxed");
    }

    #[tokio::test]
    async fn test_agent_multiple_stream_updates() {
        use futures::StreamExt;

        struct MultiUpdateAgent;

        #[async_trait]
        impl Agent<(), String> for MultiUpdateAgent {
            async fn invoke(&self, _input: ()) -> Result<String> {
                Ok("final".to_string())
            }

            async fn stream(
                &self,
                _input: (),
            ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate<String>>> + Send>>>
            {
                Ok(Box::pin(stream::iter(vec![
                    Ok(StreamUpdate::Update {
                        node: "step1".to_string(),
                        output: "processing".to_string(),
                    }),
                    Ok(StreamUpdate::Update {
                        node: "step2".to_string(),
                        output: "almost".to_string(),
                    }),
                    Ok(StreamUpdate::Final("done".to_string())),
                ])))
            }
        }

        let agent = MultiUpdateAgent;
        let mut stream = agent.stream(()).await.unwrap();

        let update1 = stream.next().await.unwrap().unwrap();
        match update1 {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "step1");
                assert_eq!(output, "processing");
            }
            _ => panic!("Expected Update variant"),
        }

        let update2 = stream.next().await.unwrap().unwrap();
        match update2 {
            StreamUpdate::Update { node, output } => {
                assert_eq!(node, "step2");
                assert_eq!(output, "almost");
            }
            _ => panic!("Expected Update variant"),
        }

        let update3 = stream.next().await.unwrap().unwrap();
        match update3 {
            StreamUpdate::Final(output) => {
                assert_eq!(output, "done");
            }
            _ => panic!("Expected Final variant"),
        }
    }

    #[tokio::test]
    async fn test_agent_error_handling() {
        use futures::StreamExt;

        struct ErrorAgent;

        #[async_trait]
        impl Agent<(), String> for ErrorAgent {
            async fn invoke(&self, _input: ()) -> Result<String> {
                Err(crate::error::Error::Generic("test error".to_string()))
            }

            async fn stream(
                &self,
                _input: (),
            ) -> Result<Pin<Box<dyn Stream<Item = Result<StreamUpdate<String>>> + Send>>>
            {
                Ok(Box::pin(stream::once(async {
                    Err(crate::error::Error::Generic("stream error".to_string()))
                })))
            }
        }

        let agent = ErrorAgent;

        let result = agent.invoke(()).await;
        assert!(result.is_err());

        let mut stream = agent.stream(()).await.unwrap();
        let update = stream.next().await.unwrap();
        assert!(update.is_err());
    }
}
