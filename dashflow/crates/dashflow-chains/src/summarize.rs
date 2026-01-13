//! Summarization chain helpers
//!
//! This module provides convenience functions for creating document summarization chains.
//! These are wrappers around the core `combine_documents` chains with sensible defaults
//! for summarization tasks.

use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::prompts::PromptTemplate;
use std::sync::Arc;

use crate::combine_documents::{
    MapReduceDocumentsChain, RefineDocumentsChain, StuffDocumentsChain,
};

/// Chain types for summarization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummarizeChainType {
    /// Stuff all documents into a single prompt
    Stuff,
    /// Map-reduce: process each document, then combine
    MapReduce,
    /// Refine: iteratively refine answer with each document
    Refine,
}

/// Default stuff prompt for summarization
const DEFAULT_STUFF_PROMPT: &str =
    "Write a concise summary of the following:\n\n{text}\n\nCONCISE SUMMARY:";

/// Default map prompt for map-reduce summarization
const DEFAULT_MAP_PROMPT: &str =
    "Write a concise summary of the following:\n\n{text}\n\nCONCISE SUMMARY:";

/// Default reduce prompt for map-reduce summarization
const DEFAULT_REDUCE_PROMPT: &str =
    "Write a concise summary of the following summaries:\n\n{text}\n\nCONCISE SUMMARY:";

/// Default initial prompt for refine summarization
const DEFAULT_REFINE_INITIAL_PROMPT: &str =
    "Write a concise summary of the following:\n\n{context}\n\nCONCISE SUMMARY:";

/// Default refine prompt for refine summarization
const DEFAULT_REFINE_PROMPT: &str = "Your job is to produce a final summary.\n\
We have provided an existing summary up to a certain point: {existing_answer}\n\
We have the opportunity to refine the existing summary (only if needed) with some more context below.\n\
------------\n\
{context}\n\
------------\n\
Given the new context, refine the original summary.\n\
If the context isn't useful, return the original summary.";

/// Load a summarization chain for `ChatModel`
///
/// # Arguments
///
/// * `llm` - The `ChatModel` to use
/// * `chain_type` - The type of chain to create
///
/// # Returns
///
/// An enum containing the appropriate chain type
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_chains::summarize::{load_summarize_chain_chat, SummarizeChainType};
///
/// let chain = load_summarize_chain_chat(llm, SummarizeChainType::Stuff);
/// ```
pub fn load_summarize_chain_chat(
    llm: Arc<dyn ChatModel>,
    chain_type: SummarizeChainType,
) -> SummarizeChain {
    match chain_type {
        SummarizeChainType::Stuff => {
            #[allow(clippy::expect_used)]
            let prompt = PromptTemplate::from_template(DEFAULT_STUFF_PROMPT)
                .expect("default stuff prompt should be valid");
            SummarizeChain::Stuff(
                StuffDocumentsChain::new_chat(llm)
                    .with_prompt(prompt)
                    .with_document_variable_name("text".to_string()),
            )
        }
        SummarizeChainType::MapReduce => {
            #[allow(clippy::expect_used)]
            let map_prompt = PromptTemplate::from_template(DEFAULT_MAP_PROMPT)
                .expect("default map prompt should be valid");
            #[allow(clippy::expect_used)]
            let reduce_prompt = PromptTemplate::from_template(DEFAULT_REDUCE_PROMPT)
                .expect("default reduce prompt should be valid");

            let reduce_chain = StuffDocumentsChain::new_chat(Arc::clone(&llm))
                .with_prompt(reduce_prompt)
                .with_document_variable_name("text".to_string());

            SummarizeChain::MapReduce(
                MapReduceDocumentsChain::new_chat(llm)
                    .with_map_prompt(map_prompt)
                    .with_reduce_chain(reduce_chain),
            )
        }
        SummarizeChainType::Refine => {
            #[allow(clippy::expect_used)]
            let initial_prompt = PromptTemplate::from_template(DEFAULT_REFINE_INITIAL_PROMPT)
                .expect("default initial prompt should be valid");
            #[allow(clippy::expect_used)]
            let refine_prompt = PromptTemplate::from_template(DEFAULT_REFINE_PROMPT)
                .expect("default refine prompt should be valid");

            SummarizeChain::Refine(
                RefineDocumentsChain::new_chat(llm)
                    .with_initial_prompt(initial_prompt)
                    .with_refine_prompt(refine_prompt)
                    .with_document_variable_name("context".to_string())
                    .with_initial_response_name("existing_answer".to_string()),
            )
        }
    }
}

/// Load a summarization chain for LLM
///
/// # Arguments
///
/// * `llm` - The LLM to use
/// * `chain_type` - The type of chain to create
///
/// # Returns
///
/// An enum containing the appropriate chain type
pub fn load_summarize_chain_llm(
    llm: Arc<dyn LLM>,
    chain_type: SummarizeChainType,
) -> SummarizeChain {
    match chain_type {
        SummarizeChainType::Stuff => {
            #[allow(clippy::expect_used)]
            let prompt = PromptTemplate::from_template(DEFAULT_STUFF_PROMPT)
                .expect("default stuff prompt should be valid");
            SummarizeChain::Stuff(
                StuffDocumentsChain::new_llm(llm)
                    .with_prompt(prompt)
                    .with_document_variable_name("text".to_string()),
            )
        }
        SummarizeChainType::MapReduce => {
            #[allow(clippy::expect_used)]
            let map_prompt = PromptTemplate::from_template(DEFAULT_MAP_PROMPT)
                .expect("default map prompt should be valid");
            #[allow(clippy::expect_used)]
            let reduce_prompt = PromptTemplate::from_template(DEFAULT_REDUCE_PROMPT)
                .expect("default reduce prompt should be valid");

            let reduce_chain = StuffDocumentsChain::new_llm(Arc::clone(&llm))
                .with_prompt(reduce_prompt)
                .with_document_variable_name("text".to_string());

            SummarizeChain::MapReduce(
                MapReduceDocumentsChain::new_llm(llm)
                    .with_map_prompt(map_prompt)
                    .with_reduce_chain(reduce_chain),
            )
        }
        SummarizeChainType::Refine => {
            #[allow(clippy::expect_used)]
            let initial_prompt = PromptTemplate::from_template(DEFAULT_REFINE_INITIAL_PROMPT)
                .expect("default initial prompt should be valid");
            #[allow(clippy::expect_used)]
            let refine_prompt = PromptTemplate::from_template(DEFAULT_REFINE_PROMPT)
                .expect("default refine prompt should be valid");

            SummarizeChain::Refine(
                RefineDocumentsChain::new_llm(llm)
                    .with_initial_prompt(initial_prompt)
                    .with_refine_prompt(refine_prompt)
                    .with_document_variable_name("context".to_string())
                    .with_initial_response_name("existing_answer".to_string()),
            )
        }
    }
}

/// Enum holding different summarization chain types
pub enum SummarizeChain {
    /// Stuff chain
    Stuff(StuffDocumentsChain),
    /// Map-reduce chain
    MapReduce(MapReduceDocumentsChain),
    /// Refine chain
    Refine(RefineDocumentsChain),
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, Generation, LLMResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::{AIMessage, BaseMessage};
    use dashflow::core::Error;

    struct MockChatModel {
        response: String,
    }

    #[async_trait]
    impl ChatModel for MockChatModel {
        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult, Error> {
            let message = AIMessage::new(self.response.clone()).into();
            Ok(ChatResult::new(ChatGeneration::new(message)))
        }
    }

    struct MockLLM {
        response: String,
    }

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            _prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult, Error> {
            Ok(LLMResult::new(Generation::new(&self.response)))
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    #[test]
    fn test_load_stuff_chain() {
        let llm = Arc::new(MockChatModel {
            response: "Summary".to_string(),
        });
        let chain = load_summarize_chain_chat(llm, SummarizeChainType::Stuff);
        assert!(matches!(chain, SummarizeChain::Stuff(_)));
    }

    #[test]
    fn test_load_map_reduce_chain() {
        let llm = Arc::new(MockChatModel {
            response: "Summary".to_string(),
        });
        let chain = load_summarize_chain_chat(llm, SummarizeChainType::MapReduce);
        assert!(matches!(chain, SummarizeChain::MapReduce(_)));
    }

    #[test]
    fn test_load_refine_chain() {
        let llm = Arc::new(MockChatModel {
            response: "Summary".to_string(),
        });
        let chain = load_summarize_chain_chat(llm, SummarizeChainType::Refine);
        assert!(matches!(chain, SummarizeChain::Refine(_)));
    }

    #[test]
    fn test_load_stuff_chain_llm() {
        let llm = Arc::new(MockLLM {
            response: "Summary".to_string(),
        });
        let chain = load_summarize_chain_llm(llm, SummarizeChainType::Stuff);
        assert!(matches!(chain, SummarizeChain::Stuff(_)));
    }
}
