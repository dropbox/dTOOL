use crate::core::error::Result;

use super::{AgentDecision, AgentStep};

/// Core trait for agent implementations
///
/// An agent decides what actions to take given the current state of execution.
/// This trait defines the interface that all agents must implement.
///
/// # Lifecycle
///
/// 1. Agent receives input and intermediate steps
/// 2. Agent plans the next action (tool call) or decides to finish
/// 3. If action: tool is executed, observation added to steps
/// 4. Loop continues until agent returns finish decision
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::agents::{Agent, AgentAction, AgentFinish};
///
/// struct MyAgent {
///     // Agent state
/// }
///
/// #[async_trait::async_trait]
/// impl Agent for MyAgent {
///     async fn plan(
///         &self,
///         input: &str,
///         intermediate_steps: &[AgentStep],
///     ) -> Result<AgentDecision> {
///         // Decide next action or finish
///         Ok(AgentDecision::Action(AgentAction {
///             tool: "calculator".to_string(),
///             tool_input: ToolInput::from_str("2 + 2"),
///             log: "Need to calculate 2 + 2".to_string(),
///         }))
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait Agent: Send + Sync {
    /// Plan the next step given the input and history
    ///
    /// # Arguments
    ///
    /// * `input` - The original user input/question
    /// * `intermediate_steps` - History of actions taken and observations received
    ///
    /// # Returns
    ///
    /// Either an action to take next or a final answer
    async fn plan(&self, input: &str, intermediate_steps: &[AgentStep]) -> Result<AgentDecision>;

    /// Return the list of input keys expected by this agent
    ///
    /// Most agents expect a single "input" key, but some may require
    /// additional context like "`chat_history`" or "context".
    fn input_keys(&self) -> Vec<String> {
        vec!["input".to_string()]
    }

    /// Return the list of output keys produced by this agent
    ///
    /// Most agents produce a single "output" key.
    fn output_keys(&self) -> Vec<String> {
        vec!["output".to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal agent implementation for testing default trait methods
    struct TestAgent;

    #[async_trait::async_trait]
    impl Agent for TestAgent {
        async fn plan(
            &self,
            _input: &str,
            _intermediate_steps: &[AgentStep],
        ) -> Result<AgentDecision> {
            // Return a finish decision for testing purposes
            Ok(AgentDecision::Finish(super::super::types::AgentFinish::new(
                "test output",
                "test log",
            )))
        }
    }

    #[test]
    fn test_default_input_keys() {
        let agent = TestAgent;
        let keys = agent.input_keys();
        assert_eq!(keys, vec!["input".to_string()]);
    }

    #[test]
    fn test_default_output_keys() {
        let agent = TestAgent;
        let keys = agent.output_keys();
        assert_eq!(keys, vec!["output".to_string()]);
    }

    /// Agent with custom input/output keys
    struct CustomKeyAgent;

    #[async_trait::async_trait]
    impl Agent for CustomKeyAgent {
        async fn plan(
            &self,
            _input: &str,
            _intermediate_steps: &[AgentStep],
        ) -> Result<AgentDecision> {
            Ok(AgentDecision::Finish(super::super::types::AgentFinish::new(
                "test",
                "log",
            )))
        }

        fn input_keys(&self) -> Vec<String> {
            vec!["input".to_string(), "context".to_string()]
        }

        fn output_keys(&self) -> Vec<String> {
            vec!["answer".to_string(), "confidence".to_string()]
        }
    }

    #[test]
    fn test_custom_input_keys() {
        let agent = CustomKeyAgent;
        let keys = agent.input_keys();
        assert_eq!(keys, vec!["input".to_string(), "context".to_string()]);
    }

    #[test]
    fn test_custom_output_keys() {
        let agent = CustomKeyAgent;
        let keys = agent.output_keys();
        assert_eq!(keys, vec!["answer".to_string(), "confidence".to_string()]);
    }

    #[tokio::test]
    async fn test_plan_returns_finish_decision() {
        let agent = TestAgent;
        let result = agent.plan("test input", &[]).await;
        assert!(result.is_ok());
        let decision = result.unwrap();
        assert!(decision.is_finish());
    }
}
