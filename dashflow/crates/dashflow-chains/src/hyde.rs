/// Hypothetical Document Embeddings (`HyDE`) Chain
///
/// Based on <https://arxiv.org/abs/2212.10496>
///
/// `HyDE` generates a hypothetical document for a query using an LLM,
/// then embeds that document instead of the query directly. This can
/// improve retrieval quality by generating a document-like representation.
use dashflow::core::embeddings::Embeddings;
use dashflow::core::language_models::{ChatModel, LLM};
use dashflow::core::messages::{BaseMessage, HumanMessage, MessageContent};
use dashflow::embed;
use std::sync::Arc;

/// Prompt templates for different `HyDE` use cases
pub mod prompts {
    pub const WEB_SEARCH: &str =
        "Please write a passage to answer the question\nQuestion: {QUESTION}\nPassage:";
    pub const SCI_FACT: &str = "Please write a scientific paper passage to support/refute the claim\nClaim: {Claim}\nPassage:";
    pub const ARGUANA: &str =
        "Please write a counter argument for the passage\nPassage: {PASSAGE}\nCounter Argument:";
    pub const TREC_COVID: &str = "Please write a scientific paper passage to answer the question\nQuestion: {QUESTION}\nPassage:";
    pub const FIQA: &str = "Please write a financial article passage to answer the question\nQuestion: {QUESTION}\nPassage:";
    pub const DBPEDIA_ENTITY: &str =
        "Please write a passage to answer the question.\nQuestion: {QUESTION}\nPassage:";
    pub const TREC_NEWS: &str =
        "Please write a news passage about the topic.\nTopic: {TOPIC}\nPassage:";
    pub const MR_TYDI: &str = "Please write a passage in Swahili/Korean/Japanese/Bengali to answer the question in detail.\nQuestion: {QUESTION}\nPassage:";

    /// Get a prompt by key
    #[must_use]
    pub fn get_prompt(key: &str) -> Option<&'static str> {
        match key {
            "web_search" => Some(WEB_SEARCH),
            "sci_fact" => Some(SCI_FACT),
            "arguana" => Some(ARGUANA),
            "trec_covid" => Some(TREC_COVID),
            "fiqa" => Some(FIQA),
            "dbpedia_entity" => Some(DBPEDIA_ENTITY),
            "trec_news" => Some(TREC_NEWS),
            "mr_tydi" => Some(MR_TYDI),
            _ => None,
        }
    }
}

/// Hypothetical Document Embedder using `ChatModel`
pub struct HypotheticalDocumentEmbedder<M: ChatModel, E: Embeddings + 'static> {
    chat_model: Arc<M>,
    base_embeddings: Arc<E>,
    prompt_template: String,
    input_variable: String,
}

impl<M: ChatModel, E: Embeddings + 'static> HypotheticalDocumentEmbedder<M, E> {
    /// Create a new `HyDE` embedder with a prompt key
    pub fn from_prompt_key(
        chat_model: Arc<M>,
        base_embeddings: Arc<E>,
        prompt_key: &str,
    ) -> Result<Self, String> {
        let prompt_template = prompts::get_prompt(prompt_key)
            .ok_or_else(|| {
                format!(
                    "Unknown prompt key: {prompt_key}. Must be one of: web_search, sci_fact, arguana, trec_covid, fiqa, dbpedia_entity, trec_news, mr_tydi"
                )
            })?
            .to_string();

        // Extract input variable from template (first occurrence of {VAR})
        let input_variable = extract_input_variable(&prompt_template)?;

        Ok(Self {
            chat_model,
            base_embeddings,
            prompt_template,
            input_variable,
        })
    }

    /// Create a new `HyDE` embedder with a custom prompt template
    pub fn from_custom_prompt(
        chat_model: Arc<M>,
        base_embeddings: Arc<E>,
        prompt_template: String,
        input_variable: String,
    ) -> Self {
        Self {
            chat_model,
            base_embeddings,
            prompt_template,
            input_variable,
        }
    }

    /// Combine multiple embeddings into a single embedding by averaging
    #[allow(clippy::needless_pass_by_value)] // Takes ownership for consistency with async boundary
    fn combine_embeddings(&self, embeddings: Vec<Vec<f32>>) -> Vec<f32> {
        if embeddings.is_empty() {
            return Vec::new();
        }

        let num_vectors = embeddings.len();
        let dim = embeddings[0].len();

        let mut result = vec![0.0f32; dim];
        for embedding in &embeddings {
            for (i, &val) in embedding.iter().enumerate() {
                result[i] += val;
            }
        }

        for val in &mut result {
            *val /= num_vectors as f32;
        }

        result
    }

    /// Generate a hypothetical document for the query, then embed it
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::error::Error> {
        // Format the prompt with the input text
        let prompt = self
            .prompt_template
            .replace(&format!("{{{}}}", self.input_variable), text);

        // Generate hypothetical document using chat model
        let message: BaseMessage = HumanMessage::new(MessageContent::Text(prompt)).into();
        let result = self
            .chat_model
            .generate(&[message], None, None, None, None)
            .await?;

        // Extract text from the first generation
        let hypothetical_doc = result
            .generations
            .first()
            .ok_or_else(|| {
                dashflow::core::error::Error::other("No response generated from LLM")
            })?
            .text();

        // Embed the hypothetical document using graph API
        let embeddings = embed(Arc::clone(&self.base_embeddings), &[hypothetical_doc])
            .await
            .map_err(|e| dashflow::core::error::Error::other(e.to_string()))?;

        // Combine (in this case, just return the single embedding)
        Ok(self.combine_embeddings(embeddings))
    }

    /// Embed multiple queries
    pub async fn embed_queries(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::error::Error> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_query(text).await?);
        }
        Ok(results)
    }

    /// Embed documents using the base embeddings directly
    pub async fn embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::error::Error> {
        embed(Arc::clone(&self.base_embeddings), texts)
            .await
            .map_err(|e| dashflow::core::error::Error::other(e.to_string()))
    }
}

/// Hypothetical Document Embedder using LLM
pub struct HypotheticalDocumentEmbedderLLM<L: LLM, E: Embeddings + 'static> {
    llm: Arc<L>,
    base_embeddings: Arc<E>,
    prompt_template: String,
    input_variable: String,
}

impl<L: LLM, E: Embeddings + 'static> HypotheticalDocumentEmbedderLLM<L, E> {
    /// Create a new `HyDE` embedder with a prompt key
    pub fn from_prompt_key(
        llm: Arc<L>,
        base_embeddings: Arc<E>,
        prompt_key: &str,
    ) -> Result<Self, String> {
        let prompt_template = prompts::get_prompt(prompt_key)
            .ok_or_else(|| {
                format!(
                    "Unknown prompt key: {prompt_key}. Must be one of: web_search, sci_fact, arguana, trec_covid, fiqa, dbpedia_entity, trec_news, mr_tydi"
                )
            })?
            .to_string();

        let input_variable = extract_input_variable(&prompt_template)?;

        Ok(Self {
            llm,
            base_embeddings,
            prompt_template,
            input_variable,
        })
    }

    /// Create a new `HyDE` embedder with a custom prompt template
    pub fn from_custom_prompt(
        llm: Arc<L>,
        base_embeddings: Arc<E>,
        prompt_template: String,
        input_variable: String,
    ) -> Self {
        Self {
            llm,
            base_embeddings,
            prompt_template,
            input_variable,
        }
    }

    /// Combine multiple embeddings into a single embedding by averaging
    #[allow(clippy::needless_pass_by_value)] // Takes ownership for consistency with async boundary
    fn combine_embeddings(&self, embeddings: Vec<Vec<f32>>) -> Vec<f32> {
        if embeddings.is_empty() {
            return Vec::new();
        }

        let num_vectors = embeddings.len();
        let dim = embeddings[0].len();

        let mut result = vec![0.0f32; dim];
        for embedding in &embeddings {
            for (i, &val) in embedding.iter().enumerate() {
                result[i] += val;
            }
        }

        for val in &mut result {
            *val /= num_vectors as f32;
        }

        result
    }

    /// Generate a hypothetical document for the query, then embed it
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, dashflow::core::error::Error> {
        // Format the prompt with the input text
        let prompt = self
            .prompt_template
            .replace(&format!("{{{}}}", self.input_variable), text);

        // Generate hypothetical document using LLM
        let result = self.llm.generate(&[prompt], None, None).await?;
        let hypothetical_doc = result
            .generations
            .first()
            .ok_or_else(|| dashflow::core::error::Error::other("No response generated from LLM"))?
            .first()
            .ok_or_else(|| dashflow::core::error::Error::other("Empty generation list from LLM"))?
            .text
            .clone();

        // Embed the hypothetical document using graph API
        let embeddings = embed(Arc::clone(&self.base_embeddings), &[hypothetical_doc])
            .await
            .map_err(|e| dashflow::core::error::Error::other(e.to_string()))?;

        // Combine (in this case, just return the single embedding)
        Ok(self.combine_embeddings(embeddings))
    }

    /// Embed multiple queries
    pub async fn embed_queries(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::error::Error> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed_query(text).await?);
        }
        Ok(results)
    }

    /// Embed documents using the base embeddings directly
    pub async fn embed_documents(
        &self,
        texts: &[String],
    ) -> Result<Vec<Vec<f32>>, dashflow::core::error::Error> {
        embed(Arc::clone(&self.base_embeddings), texts)
            .await
            .map_err(|e| dashflow::core::error::Error::other(e.to_string()))
    }
}

/// Extract the first input variable from a prompt template
fn extract_input_variable(template: &str) -> Result<String, String> {
    let start = template
        .find('{')
        .ok_or_else(|| "No input variable found in template".to_string())?;
    let end = template[start + 1..]
        .find('}')
        .ok_or_else(|| "Unclosed input variable in template".to_string())?;
    Ok(template[start + 1..start + 1 + end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use dashflow::core::language_models::{ChatGeneration, ChatResult, Generation, LLMResult};
    use dashflow::core::language_models::{ToolChoice, ToolDefinition};
    use dashflow::core::messages::AIMessage;

    struct MockChatModel;

    #[async_trait]
    impl ChatModel for MockChatModel {
        async fn _generate(
            &self,
            _messages: &[BaseMessage],
            _stop: Option<&[String]>,
            _tools: Option<&[ToolDefinition]>,
            _tool_choice: Option<&ToolChoice>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<ChatResult, dashflow::core::error::Error> {
            // Return a mock hypothetical document
            Ok(ChatResult {
                generations: vec![ChatGeneration {
                    message: AIMessage::new(MessageContent::Text(
                        "This is a hypothetical document about the query.".to_string(),
                    ))
                    .into(),
                    generation_info: None,
                }],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
    }

    struct MockLLM;

    #[async_trait]
    impl LLM for MockLLM {
        async fn _generate(
            &self,
            _prompts: &[String],
            _stop: Option<&[String]>,
            _run_manager: Option<&dashflow::core::callbacks::CallbackManager>,
        ) -> Result<LLMResult, dashflow::core::error::Error> {
            Ok(LLMResult {
                generations: vec![vec![Generation {
                    text: "This is a hypothetical document from LLM.".to_string(),
                    generation_info: None,
                }]],
                llm_output: None,
            })
        }

        fn llm_type(&self) -> &str {
            "mock"
        }
    }

    struct MockEmbeddings;

    #[async_trait]
    impl Embeddings for MockEmbeddings {
        async fn _embed_documents(
            &self,
            texts: &[String],
        ) -> Result<Vec<Vec<f32>>, dashflow::core::error::Error> {
            // Return mock embeddings (3-dimensional for simplicity)
            Ok(texts.iter().map(|_| vec![0.1, 0.2, 0.3]).collect())
        }

        async fn _embed_query(&self, _text: &str) -> Result<Vec<f32>, dashflow::core::error::Error> {
            Ok(vec![0.1, 0.2, 0.3])
        }
    }

    #[tokio::test]
    async fn test_hyde_embedder_from_prompt_key() {
        let chat_model = Arc::new(MockChatModel);
        let embeddings = Arc::new(MockEmbeddings);

        let hyde =
            HypotheticalDocumentEmbedder::from_prompt_key(chat_model, embeddings, "web_search")
                .unwrap();

        let result = hyde
            .embed_query("What is the capital of France?")
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn test_hyde_embedder_llm_from_prompt_key() {
        let llm = Arc::new(MockLLM);
        let embeddings = Arc::new(MockEmbeddings);

        let hyde = HypotheticalDocumentEmbedderLLM::from_prompt_key(llm, embeddings, "web_search")
            .unwrap();

        let result = hyde
            .embed_query("What is the capital of France?")
            .await
            .unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result, vec![0.1, 0.2, 0.3]);
    }

    #[tokio::test]
    async fn test_hyde_embedder_custom_prompt() {
        let chat_model = Arc::new(MockChatModel);
        let embeddings = Arc::new(MockEmbeddings);

        let hyde = HypotheticalDocumentEmbedder::from_custom_prompt(
            chat_model,
            embeddings,
            "Answer this: {query}".to_string(),
            "query".to_string(),
        );

        let result = hyde.embed_query("test query").await.unwrap();
        assert_eq!(result.len(), 3);
    }

    #[tokio::test]
    async fn test_combine_embeddings() {
        let chat_model = Arc::new(MockChatModel);
        let embeddings = Arc::new(MockEmbeddings);

        let hyde =
            HypotheticalDocumentEmbedder::from_prompt_key(chat_model, embeddings, "web_search")
                .unwrap();

        let combined = hyde.combine_embeddings(vec![vec![0.0, 0.0, 0.0], vec![1.0, 2.0, 3.0]]);
        assert_eq!(combined, vec![0.5, 1.0, 1.5]);
    }

    #[tokio::test]
    async fn test_extract_input_variable() {
        assert_eq!(extract_input_variable("Test {var} here").unwrap(), "var");
        assert_eq!(extract_input_variable("{QUESTION}").unwrap(), "QUESTION");
        assert!(extract_input_variable("No variables").is_err());
    }
}
