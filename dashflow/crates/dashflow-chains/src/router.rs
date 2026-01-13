//! Router chains for conditional chain execution.
//!
//! Router chains analyze input and dynamically select which downstream chain to execute.
//! They enable building flexible workflows that branch based on input characteristics.
//!
//! # Key Components
//!
//! - **`LLMRouterChain`**: Uses an LLM to make routing decisions
//! - **`MultiPromptChain`**: Routes between multiple prompts based on input
//! - **`MultiRetrievalQAChain`**: Routes between multiple retrieval QA chains based on input
//! - **`RouterOutputParser`**: Parses LLM output into routing decisions
//!
//! # Example
//!
//! ```rust,ignore
//! use dashflow_chains::router::{MultiPromptChain, PromptInfo};
//! use dashflow_openai::ChatOpenAI;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let llm = Arc::new(ChatOpenAI::default());
//!
//! let prompt_infos = vec![
//!     PromptInfo {
//!         name: "physics".to_string(),
//!         description: "Good for answering physics questions".to_string(),
//!         prompt_template: "You are a physics expert. {input}".to_string(),
//!     },
//!     PromptInfo {
//!         name: "math".to_string(),
//!         description: "Good for answering math questions".to_string(),
//!         prompt_template: "You are a math expert. {input}".to_string(),
//!     },
//! ];
//!
//! let chain = MultiPromptChain::from_prompts(llm, prompt_infos, None)?;
//! let result = chain.invoke("What is the speed of light?").await?;
//! # Ok(())
//! # }
//! ```

use dashflow::core::error::{Error, Result};
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::output_parsers::OutputParser;
use dashflow::core::prompts::{PromptTemplate, PromptTemplateFormat};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::Arc;

use crate::llm::LLMChain;
use crate::retrieval_qa::{ChainType, RetrievalQA};
use dashflow::core::retrievers::Retriever;

/// Route to a destination chain.
///
/// Contains the name of the destination chain and the inputs to pass to it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Name of the destination chain, or None for default chain
    pub destination: Option<String>,
    /// Inputs to pass to the destination chain
    pub next_inputs: HashMap<String, JsonValue>,
}

/// Output parser for router chains.
///
/// Parses LLM output in JSON format with "destination" and "`next_inputs`" fields.
/// The "destination" field can be a chain name or "DEFAULT" for the default chain.
/// The "`next_inputs`" field is converted to a `HashMap` with the specified inner key.
///
/// # JSON Format
///
/// ```json
/// {
///     "destination": "chain_name",
///     "next_inputs": "input text"
/// }
/// ```
///
/// Or for default chain:
/// ```json
/// {
///     "destination": "DEFAULT",
///     "next_inputs": "input text"
/// }
/// ```
#[derive(Debug, Clone)]
pub struct RouterOutputParser {
    /// Default destination name (case-insensitive comparison)
    pub default_destination: String,
    /// Key name for wrapping `next_inputs`
    pub next_inputs_inner_key: String,
}

impl Default for RouterOutputParser {
    fn default() -> Self {
        Self {
            default_destination: "DEFAULT".to_string(),
            next_inputs_inner_key: "input".to_string(),
        }
    }
}

impl RouterOutputParser {
    /// Create a new `RouterOutputParser` with custom settings
    #[must_use]
    pub fn new(default_destination: String, next_inputs_inner_key: String) -> Self {
        Self {
            default_destination,
            next_inputs_inner_key,
        }
    }

    /// Parse JSON markdown from LLM output
    fn parse_json_markdown(&self, text: &str) -> Result<JsonValue> {
        // Try to extract JSON from markdown code block
        let json_str = if let Some(start_marker) = text.find("```json") {
            // Skip past "```json" and any newline
            let content_start = start_marker + 7;
            let content_start = if text[content_start..].starts_with('\n') {
                content_start + 1
            } else {
                content_start
            };

            // Find closing backticks after the content
            if let Some(end_offset) = text[content_start..].find("```") {
                &text[content_start..content_start + end_offset]
            } else {
                // No closing backticks, take rest of text
                &text[content_start..]
            }
        } else if let Some(start_marker) = text.find("```") {
            // Generic code block
            let content_start = start_marker + 3;
            let content_start = if text[content_start..].starts_with('\n') {
                content_start + 1
            } else {
                content_start
            };

            if let Some(end_offset) = text[content_start..].find("```") {
                &text[content_start..content_start + end_offset]
            } else {
                &text[content_start..]
            }
        } else {
            // No code block markers, use entire text
            text
        };

        serde_json::from_str(json_str.trim()).map_err(|e| {
            Error::OutputParsing(format!(
                "Failed to parse router output as JSON: {e}. Text: {text}"
            ))
        })
    }
}

impl OutputParser for RouterOutputParser {
    type Output = Route;

    fn parse(&self, text: &str) -> Result<Self::Output> {
        let parsed = self.parse_json_markdown(text)?;

        // Extract destination
        let destination = parsed
            .get("destination")
            .ok_or_else(|| Error::OutputParsing("Missing 'destination' field".to_string()))?
            .as_str()
            .ok_or_else(|| Error::OutputParsing("'destination' must be a string".to_string()))?;

        // Extract next_inputs
        let next_inputs = parsed
            .get("next_inputs")
            .ok_or_else(|| Error::OutputParsing("Missing 'next_inputs' field".to_string()))?;

        // Convert next_inputs to HashMap
        let next_inputs_map = if next_inputs.is_string() {
            // If next_inputs is a string, wrap it with inner key
            let mut map = HashMap::new();
            map.insert(
                self.next_inputs_inner_key.clone(),
                JsonValue::String(
                    next_inputs
                        .as_str()
                        .ok_or_else(|| {
                            Error::OutputParsing(
                                "Failed to extract string from next_inputs".to_string(),
                            )
                        })?
                        .to_string(),
                ),
            );
            map
        } else if next_inputs.is_object() {
            // If next_inputs is already an object, use it directly
            next_inputs
                .as_object()
                .ok_or_else(|| {
                    Error::OutputParsing("next_inputs must be string or object".to_string())
                })?
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        } else {
            return Err(Error::OutputParsing(
                "next_inputs must be a string or object".to_string(),
            ));
        };

        // Check if destination is default
        let destination = if destination
            .trim()
            .eq_ignore_ascii_case(&self.default_destination)
        {
            None
        } else {
            Some(destination.trim().to_string())
        };

        Ok(Route {
            destination,
            next_inputs: next_inputs_map,
        })
    }

    fn get_format_instructions(&self) -> String {
        format!(
            "Return a JSON object with 'destination' (chain name or '{}') and 'next_inputs' (input text)",
            self.default_destination
        )
    }
}

/// LLM-powered router chain.
///
/// Uses an LLM to analyze input and decide which destination chain to route to.
/// The LLM's output is parsed by a `RouterOutputParser` into a Route object.
///
/// # Type Parameters
///
/// - `M`: The model type (LLM or `ChatModel`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::router::LLMRouterChain;
/// use dashflow_chains::llm::LLMChain;
/// use dashflow_openai::ChatOpenAI;
/// use dashflow::core::prompts::PromptTemplate;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let llm = Arc::new(ChatOpenAI::default());
/// let prompt = PromptTemplate::new("Route this: {input}", vec!["input"], None);
/// let llm_chain = LLMChain::new(llm, prompt);
/// let router = LLMRouterChain::new(llm_chain);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LLMRouterChain<M> {
    pub llm_chain: LLMChain<M>,
}

impl<M> LLMRouterChain<M> {
    /// Create a new `LLMRouterChain`
    #[must_use]
    pub fn new(llm_chain: LLMChain<M>) -> Self {
        Self { llm_chain }
    }
}

impl<M: LLM> LLMRouterChain<M> {
    /// Route inputs to a destination chain
    pub async fn route(&self, inputs: &HashMap<String, String>) -> Result<Route> {
        // Generate LLM response
        let response = self.llm_chain.run(inputs).await?;

        // Parse response using RouterOutputParser
        let parser = RouterOutputParser::default();
        parser.parse(&response)
    }
}

impl<M: ChatModel> LLMRouterChain<M> {
    /// Route inputs to a destination chain (`ChatModel` version)
    ///
    /// # Status: Deferred
    /// ChatModel routing requires different handling than LLM routing because
    /// `LLMChain<M>` doesn't expose a `run()` method for `ChatModel`.
    /// Use `route()` with an LLM instead for now.
    pub async fn route_chat(&self, _inputs: &HashMap<String, String>) -> Result<Route> {
        // Design limitation: LLMChain<M>::run() not available for ChatModel
        // Use route() with LLM instead - more common and fully supported
        Err(Error::Other(
            "ChatModel routing deferred - use route() with LLM instead".to_string(),
        ))
    }
}

/// Information about a prompt destination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInfo {
    /// Name of the prompt (used for routing)
    pub name: String,
    /// Description of when to use this prompt
    pub description: String,
    /// Template string for the prompt
    pub prompt_template: String,
}

/// Default router template for `MultiPromptChain`
pub const MULTI_PROMPT_ROUTER_TEMPLATE: &str = r#"Given a raw text input to a language model select the model prompt best suited for the input. You will be given the names of the available prompts and a description of what the prompt is best suited for. You may also revise the original input if you think that revising it will ultimately lead to a better response from the language model.

<< FORMATTING >>
Return a markdown code snippet with a JSON object formatted to look like:
```json
{{
    "destination": string \\ name of the prompt to use or "DEFAULT"
    "next_inputs": string \\ a potentially modified version of the original input
}}
```

REMEMBER: "destination" MUST be one of the candidate prompt names specified below OR it can be "DEFAULT" if the input is not well suited for any of the candidate prompts.
REMEMBER: "next_inputs" can just be the original input if you don't think any modifications are needed.

<< CANDIDATE PROMPTS >>
{destinations}

<< INPUT >>
{input}

<< OUTPUT (must include ```json at the start of the response) >>
<< OUTPUT (must end with ```) >>
"#;

/// Multi-route chain that uses an LLM router to choose between multiple prompts.
///
/// Routes input to different `LLMChains` based on the input content. Uses an `LLMRouterChain`
/// to analyze input and select the most appropriate destination chain. Falls back to a
/// default chain if no suitable destination is found.
///
/// # Type Parameters
///
/// - `M`: The model type (LLM or `ChatModel`)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::router::{MultiPromptChain, PromptInfo};
/// use dashflow_openai::ChatOpenAI;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let llm = Arc::new(ChatOpenAI::default());
///
/// let prompts = vec![
///     PromptInfo {
///         name: "physics".to_string(),
///         description: "Good for physics questions".to_string(),
///         prompt_template: "You are a physics expert. {input}".to_string(),
///     },
///     PromptInfo {
///         name: "math".to_string(),
///         description: "Good for math questions".to_string(),
///         prompt_template: "You are a math expert. {input}".to_string(),
///     },
/// ];
///
/// let chain = MultiPromptChain::from_prompts(llm, prompts, None)?;
/// let result = chain.invoke("What is E=mc^2?").await?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct MultiPromptChain<M: LLM> {
    pub router_chain: LLMRouterChain<M>,
    pub destination_chains: HashMap<String, LLMChain<M>>,
    pub default_chain: LLMChain<M>,
    pub silent_errors: bool,
}

impl<M: LLM + Clone> MultiPromptChain<M> {
    /// Create a `MultiPromptChain` from a set of prompt configurations
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model to use for routing and destination chains
    /// * `prompt_infos` - List of prompt configurations
    /// * `default_chain` - Optional default chain (creates simple default if None)
    ///
    /// # Returns
    ///
    /// A configured `MultiPromptChain` ready to route inputs
    pub fn from_prompts(
        llm: Arc<M>,
        prompt_infos: Vec<PromptInfo>,
        default_chain: Option<LLMChain<M>>,
    ) -> Result<Self> {
        // Build destination list for router prompt
        let destinations: Vec<String> = prompt_infos
            .iter()
            .map(|p| format!("{}: {}", p.name, p.description))
            .collect();
        let destinations_str = destinations.join("\n");

        // Create router prompt
        let router_template =
            MULTI_PROMPT_ROUTER_TEMPLATE.replace("{destinations}", &destinations_str);
        let router_prompt = PromptTemplate::new(
            router_template,
            vec!["input".to_string()],
            PromptTemplateFormat::FString,
        );
        let router_llm_chain = LLMChain::new(Arc::clone(&llm), router_prompt);
        let router_chain = LLMRouterChain::new(router_llm_chain);

        // Create destination chains
        let mut destination_chains = HashMap::new();
        for p_info in prompt_infos {
            let prompt = PromptTemplate::new(
                p_info.prompt_template,
                vec!["input".to_string()],
                PromptTemplateFormat::FString,
            );
            let chain = LLMChain::new(Arc::clone(&llm), prompt);
            destination_chains.insert(p_info.name, chain);
        }

        // Create default chain
        let default = if let Some(chain) = default_chain {
            chain
        } else {
            // Create a simple default chain
            let default_prompt = PromptTemplate::new(
                "The following is a friendly conversation between a human and an AI. The AI is talkative and provides lots of specific details from its context. If the AI does not know the answer to a question, it truthfully says it does not know.\n\nCurrent conversation:\n\nHuman: {input}\nAI:".to_string(),
                vec!["input".to_string()],
                PromptTemplateFormat::FString,
            );
            LLMChain::new(llm, default_prompt)
        };

        Ok(Self {
            router_chain,
            destination_chains,
            default_chain: default,
            silent_errors: false,
        })
    }

    /// Set whether to silently use default chain on routing errors
    #[must_use]
    pub fn with_silent_errors(mut self, silent_errors: bool) -> Self {
        self.silent_errors = silent_errors;
        self
    }

    /// Invoke the chain with an input string
    pub async fn invoke(&self, input: &str) -> Result<String> {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());

        // Route the input
        let route = self.router_chain.route(&inputs).await?;

        // Execute the appropriate chain
        if let Some(destination) = &route.destination {
            if let Some(chain) = self.destination_chains.get(destination) {
                // Convert route.next_inputs to HashMap<String, String>
                let chain_inputs: HashMap<String, String> = route
                    .next_inputs
                    .into_iter()
                    .map(|(k, v)| {
                        let v_str = match v {
                            JsonValue::String(s) => s,
                            _ => v.to_string(),
                        };
                        (k, v_str)
                    })
                    .collect();
                return chain.run(&chain_inputs).await;
            } else if !self.silent_errors {
                return Err(Error::InvalidInput(format!(
                    "Invalid destination chain: {destination}"
                )));
            }
        }

        // Use default chain
        let chain_inputs: HashMap<String, String> = route
            .next_inputs
            .into_iter()
            .map(|(k, v)| {
                let v_str = match v {
                    JsonValue::String(s) => s,
                    _ => v.to_string(),
                };
                (k, v_str)
            })
            .collect();
        self.default_chain.run(&chain_inputs).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_output_parser_basic() {
        let parser = RouterOutputParser::default();

        let text = r#"```json
{
    "destination": "physics",
    "next_inputs": "What is the speed of light?"
}
```"#;

        let route = parser.parse(text).unwrap();
        assert_eq!(route.destination, Some("physics".to_string()));
        assert_eq!(
            route.next_inputs.get("input").unwrap().as_str().unwrap(),
            "What is the speed of light?"
        );
    }

    #[test]
    fn test_router_output_parser_default() {
        let parser = RouterOutputParser::default();

        let text = r#"```json
{
    "destination": "DEFAULT",
    "next_inputs": "Hello"
}
```"#;

        let route = parser.parse(text).unwrap();
        assert_eq!(route.destination, None);
        assert_eq!(
            route.next_inputs.get("input").unwrap().as_str().unwrap(),
            "Hello"
        );
    }

    #[test]
    fn test_router_output_parser_case_insensitive() {
        let parser = RouterOutputParser::default();

        let text = r#"```json
{
    "destination": "default",
    "next_inputs": "test"
}
```"#;

        let route = parser.parse(text).unwrap();
        assert_eq!(route.destination, None);
    }

    #[test]
    fn test_router_output_parser_without_markdown() {
        let parser = RouterOutputParser::default();

        let text = r#"{
    "destination": "math",
    "next_inputs": "What is 2+2?"
}"#;

        let route = parser.parse(text).unwrap();
        assert_eq!(route.destination, Some("math".to_string()));
    }

    #[test]
    fn test_router_output_parser_object_next_inputs() {
        let parser = RouterOutputParser::default();

        let text = r#"```json
{
    "destination": "test",
    "next_inputs": {"input": "value", "extra": "data"}
}
```"#;

        let route = parser.parse(text).unwrap();
        assert_eq!(route.destination, Some("test".to_string()));
        assert_eq!(
            route.next_inputs.get("input").unwrap().as_str().unwrap(),
            "value"
        );
        assert_eq!(
            route.next_inputs.get("extra").unwrap().as_str().unwrap(),
            "data"
        );
    }

    #[test]
    fn test_multi_retrieval_router_template() {
        // Verify template has expected structure
        assert!(MULTI_RETRIEVAL_ROUTER_TEMPLATE.contains("FORMATTING"));
        assert!(MULTI_RETRIEVAL_ROUTER_TEMPLATE.contains("CANDIDATE PROMPTS"));
        assert!(MULTI_RETRIEVAL_ROUTER_TEMPLATE.contains("{destinations}"));
        assert!(MULTI_RETRIEVAL_ROUTER_TEMPLATE.contains("{input}"));
        assert!(MULTI_RETRIEVAL_ROUTER_TEMPLATE.contains("DEFAULT"));
    }

    // Property-based tests
    mod property_tests {
        use super::*;
        use proptest::prelude::*;

        // Generate valid destination names (alphanumeric + underscore)
        fn destination_name() -> impl Strategy<Value = String> {
            "[a-z][a-z0-9_]{0,20}"
        }

        // Generate valid input text (exclude control characters, quotes, backslash, and backticks for valid JSON)
        fn input_text() -> impl Strategy<Value = String> {
            "[a-zA-Z0-9 !#$%&()*+,./:<=>?@\\[\\]^_{|}~-]{1,50}"
        }

        proptest! {
            /// Property: Routing decision is deterministic for same input
            /// Parsing the same JSON twice produces identical Route objects
            #[test]
            fn prop_routing_deterministic(
                dest in destination_name(),
                input in input_text(),
            ) {
                let parser = RouterOutputParser::default();

                let json = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, dest, input);

                let route1 = parser.parse(&json).unwrap();
                let route2 = parser.parse(&json).unwrap();

                // Same input produces same routing decision
                prop_assert_eq!(route1.destination, route2.destination, "Destination should be deterministic");
                prop_assert_eq!(
                    route1.next_inputs.get("input"),
                    route2.next_inputs.get("input"),
                    "Next inputs should be deterministic"
                );
            }

            /// Property: All named destinations are reachable
            /// Any valid destination name can be parsed successfully
            #[test]
            fn prop_all_routes_reachable(
                dest in destination_name(),
                input in input_text(),
            ) {
                let parser = RouterOutputParser::default();

                let json = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, dest, input);

                let route = parser.parse(&json);
                prop_assert!(route.is_ok(), "Should successfully parse any valid destination name");

                let route = route.unwrap();
                prop_assert_eq!(route.destination, Some(dest), "Should route to specified destination");
                prop_assert_eq!(
                    route.next_inputs.get("input").unwrap().as_str().unwrap(),
                    input,
                    "Should preserve input text"
                );
            }

            /// Property: DEFAULT destination routes to None (default chain)
            /// Case-insensitive variants of "DEFAULT" all route to default
            #[test]
            fn prop_default_route_used(
                input in input_text(),
                case_variant in prop::sample::select(vec!["DEFAULT", "default", "Default", "DeFaUlT"]),
            ) {
                let parser = RouterOutputParser::default();

                let json = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, case_variant, input);

                let route = parser.parse(&json).unwrap();

                // All case variants of DEFAULT route to None
                prop_assert_eq!(route.destination, None, "DEFAULT variants should route to default chain");
                prop_assert_eq!(
                    route.next_inputs.get("input").unwrap().as_str().unwrap(),
                    input,
                    "Should preserve input text"
                );
            }

            /// Property: Parser handles markdown code blocks correctly
            /// JSON wrapped in ```json ... ``` is equivalent to unwrapped JSON
            #[test]
            fn prop_markdown_wrapping_equivalent(
                dest in destination_name(),
                input in input_text(),
            ) {
                let parser = RouterOutputParser::default();

                let json_bare = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, dest, input);

                let json_wrapped = format!(r#"```json
{{
    "destination": "{}",
    "next_inputs": "{}"
}}
```"#, dest, input);

                let route_bare = parser.parse(&json_bare).unwrap();
                let route_wrapped = parser.parse(&json_wrapped).unwrap();

                // Wrapped and unwrapped should produce identical routes
                prop_assert_eq!(route_bare.destination, route_wrapped.destination, "Markdown wrapping should not affect destination");
                prop_assert_eq!(
                    route_bare.next_inputs.get("input"),
                    route_wrapped.next_inputs.get("input"),
                    "Markdown wrapping should not affect next_inputs"
                );
            }

            /// Property: next_inputs can be string or object
            /// Parser handles both string values and JSON objects for next_inputs
            #[test]
            fn prop_next_inputs_flexible(
                dest in destination_name(),
                input in input_text(),
            ) {
                let parser = RouterOutputParser::default();

                // Test with string next_inputs
                let json_string = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, dest, input);

                // Test with object next_inputs
                let json_object = format!(r#"{{
    "destination": "{}",
    "next_inputs": {{"input": "{}", "extra": "data"}}
}}"#, dest, input);

                let route_string = parser.parse(&json_string);
                let route_object = parser.parse(&json_object);

                prop_assert!(route_string.is_ok(), "Should parse string next_inputs");
                prop_assert!(route_object.is_ok(), "Should parse object next_inputs");

                let route_string = route_string.unwrap();
                let route_object = route_object.unwrap();

                // Both should have same destination and contain input
                prop_assert_eq!(&route_string.destination, &route_object.destination);
                prop_assert_eq!(
                    route_string.next_inputs.get("input").unwrap().as_str().unwrap(),
                    input.as_str(),
                    "String variant should have input"
                );
                prop_assert_eq!(
                    route_object.next_inputs.get("input").unwrap().as_str().unwrap(),
                    input.as_str(),
                    "Object variant should have input"
                );
            }

            /// Property: Custom default_destination is respected
            #[test]
            fn prop_custom_default_destination(
                custom_default in "[A-Z]{3,10}",
                input in input_text(),
            ) {
                let parser = RouterOutputParser {
                    default_destination: custom_default.clone(),
                    next_inputs_inner_key: "input".to_string(),
                };

                let json = format!(r#"{{
    "destination": "{}",
    "next_inputs": "{}"
}}"#, custom_default, input);

                let route = parser.parse(&json).unwrap();

                // Custom default should route to None
                prop_assert_eq!(route.destination, None, "Custom default destination should route to None");
            }
        }
    }
}

/// Router template for `MultiRetrievalQAChain`
pub const MULTI_RETRIEVAL_ROUTER_TEMPLATE: &str = r#"Given a query to a question answering system select the system best suited for the input. You will be given the names of the available systems and a description of what questions the system is best suited for. You may also revise the original input if you think that revising it will ultimately lead to a better response.

<< FORMATTING >>
Return a markdown code snippet with a JSON object formatted to look like:
```json
{{
    "destination": string \\ name of the question answering system to use or "DEFAULT"
    "next_inputs": string \\ a potentially modified version of the original input
}}
```

REMEMBER: "destination" MUST be one of the candidate prompt names specified below OR it can be "DEFAULT" if the input is not well suited for any of the candidate prompts.
REMEMBER: "next_inputs" can just be the original input if you don't think any modifications are needed.

<< CANDIDATE PROMPTS >>
{destinations}

<< INPUT >>
{input}

<< OUTPUT (must include ```json at the start of the response) >>
<< OUTPUT (must end with ```) >>
"#;

/// Information about a retriever destination
#[derive(Debug, Clone)]
pub struct RetrieverInfo<R: Retriever> {
    /// Name of the retriever (used for routing)
    pub name: String,
    /// Description of when to use this retriever
    pub description: String,
    /// The retriever instance
    pub retriever: Arc<R>,
    /// Optional custom prompt template for this retriever's QA chain
    pub prompt: Option<PromptTemplate>,
}

/// Multi-route chain that uses an LLM router to choose between multiple retrieval QA chains.
///
/// Routes input to different `RetrievalQA` chains based on the input content. Uses an `LLMRouterChain`
/// to analyze input and select the most appropriate retriever. Falls back to a default retriever
/// if no suitable destination is found.
///
/// This is similar to `MultiPromptChain` but specifically for retrieval-based QA.
///
/// # Type Parameters
///
/// - `M`: The model type (must implement LLM)
/// - `R`: The retriever type (must implement Retriever)
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::router::{MultiRetrievalQAChain, RetrieverInfo};
/// use dashflow_openai::ChatOpenAI;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let llm = Arc::new(ChatOpenAI::default());
///
/// let retriever_infos = vec![
///     RetrieverInfo {
///         name: "science".to_string(),
///         description: "Good for answering science questions".to_string(),
///         retriever: Arc::new(science_retriever),
///         prompt: None,
///     },
///     RetrieverInfo {
///         name: "history".to_string(),
///         description: "Good for answering history questions".to_string(),
///         retriever: Arc::new(history_retriever),
///         prompt: None,
///     },
/// ];
///
/// let chain = MultiRetrievalQAChain::from_retrievers(
///     llm,
///     retriever_infos,
///     None, // default_retriever
/// )?;
/// let result = chain.invoke("What is photosynthesis?").await?;
/// # Ok(())
/// # }
/// ```
pub struct MultiRetrievalQAChain<M: LLM, R: Retriever> {
    pub router_chain: LLMRouterChain<M>,
    pub destination_chains: HashMap<String, RetrievalQA<M, R>>,
    pub default_chain: RetrievalQA<M, R>,
    pub silent_errors: bool,
}

impl<M: LLM + Clone + 'static, R: Retriever + Clone + 'static> MultiRetrievalQAChain<M, R> {
    /// Create a `MultiRetrievalQAChain` from a set of retriever configurations
    ///
    /// # Arguments
    ///
    /// * `llm` - Language model to use for routing and destination QA chains
    /// * `retriever_infos` - List of retriever configurations
    /// * `default_retriever` - Default retriever to use when routing fails
    ///
    /// # Returns
    ///
    /// A configured `MultiRetrievalQAChain` ready to route inputs
    #[allow(clippy::needless_pass_by_value)] // Arc passed by value for multiple clones inside
    pub fn from_retrievers(
        llm: Arc<M>,
        retriever_infos: Vec<RetrieverInfo<R>>,
        default_retriever: Option<R>,
    ) -> Result<Self> {
        if retriever_infos.is_empty() {
            return Err(Error::InvalidInput(
                "retriever_infos cannot be empty".to_string(),
            ));
        }

        // Build destination list for router prompt
        let destinations: Vec<String> = retriever_infos
            .iter()
            .map(|r| format!("{}: {}", r.name, r.description))
            .collect();
        let destinations_str = destinations.join("\n");

        // Create router prompt
        let router_template =
            MULTI_RETRIEVAL_ROUTER_TEMPLATE.replace("{destinations}", &destinations_str);
        let router_prompt = PromptTemplate::new(
            router_template,
            vec!["input".to_string()],
            PromptTemplateFormat::FString,
        );
        let router_llm_chain = LLMChain::new(Arc::clone(&llm), router_prompt);
        let router_chain = LLMRouterChain::new(router_llm_chain);

        // Create destination RetrievalQA chains
        let mut destination_chains = HashMap::new();
        let first_retriever = retriever_infos[0].retriever.as_ref().clone();

        for r_info in retriever_infos {
            let mut qa_chain = RetrievalQA::new(
                (*llm).clone(),
                r_info.retriever.as_ref().clone(),
                ChainType::Stuff,
            );

            // Use custom prompt if provided
            if let Some(prompt) = r_info.prompt {
                qa_chain = qa_chain.with_prompt(prompt);
            }

            // Set input/output keys to match router expectations
            qa_chain = qa_chain.with_input_key("input");
            qa_chain = qa_chain.with_output_key("result");

            destination_chains.insert(r_info.name, qa_chain);
        }

        // Create default chain
        let default = if let Some(retriever) = default_retriever {
            RetrievalQA::new((*llm).clone(), retriever, ChainType::Stuff)
                .with_input_key("input")
                .with_output_key("result")
        } else {
            // Use the first retriever as default if none provided
            RetrievalQA::new((*llm).clone(), first_retriever, ChainType::Stuff)
                .with_input_key("input")
                .with_output_key("result")
        };

        Ok(Self {
            router_chain,
            destination_chains,
            default_chain: default,
            silent_errors: false,
        })
    }

    /// Set whether to silently use default chain on routing errors
    #[must_use]
    pub fn with_silent_errors(mut self, silent_errors: bool) -> Self {
        self.silent_errors = silent_errors;
        self
    }

    /// Invoke the chain with an input string
    pub async fn invoke(&self, input: &str) -> Result<String> {
        let mut inputs = HashMap::new();
        inputs.insert("input".to_string(), input.to_string());

        // Route the input
        let route = self.router_chain.route(&inputs).await?;

        // Execute the appropriate chain
        if let Some(destination) = &route.destination {
            if let Some(chain) = self.destination_chains.get(destination) {
                return chain.run(input).await;
            } else if !self.silent_errors {
                return Err(Error::InvalidInput(format!(
                    "Invalid destination chain: {destination}"
                )));
            }
        }

        // Use default chain
        self.default_chain.run(input).await
    }
}
