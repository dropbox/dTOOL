//! Example selectors for few-shot prompting
//!
//! Example selectors dynamically choose which examples to include in prompts
//! based on input similarity, length constraints, or other criteria. This is
//! essential for few-shot learning where you want to show the most relevant
//! examples to the LLM.
//!
//! # Overview
//!
//! - [`BaseExampleSelector`] - Core trait for example selection
//! - [`SemanticSimilarityExampleSelector`] - Select by semantic similarity using embeddings
//! - [`MaxMarginalRelevanceExampleSelector`] - Balance similarity and diversity
//! - [`LengthBasedExampleSelector`] - Select examples that fit within token limits
//!
//! # Examples
//!
//! ## Semantic Similarity Selection
//!
//! ```rust,ignore
//! use dashflow::core::prompts::example_selector::SemanticSimilarityExampleSelector;
//! use dashflow::core::vector_stores::InMemoryVectorStore;
//! use dashflow_openai::OpenAIEmbeddings;
//! use std::collections::HashMap;
//!
//! let examples = vec![
//!     HashMap::from([
//!         ("input".to_string(), "What is 2+2?".to_string()),
//!         ("output".to_string(), "4".to_string()),
//!     ]),
//!     HashMap::from([
//!         ("input".to_string(), "What is 3+3?".to_string()),
//!         ("output".to_string(), "6".to_string()),
//!     ]),
//! ];
//!
//! let embeddings = OpenAIEmbeddings::default();
//! let selector = SemanticSimilarityExampleSelector::from_examples(
//!     examples,
//!     embeddings,
//!     InMemoryVectorStore::new(),
//!     4, // k
//!     None, // input_keys
//!     None, // example_keys
//!     None, // vectorstore_kwargs
//! ).await?;
//!
//! let input = HashMap::from([("input".to_string(), "What is 5+5?".to_string())]);
//! let selected = selector.select_examples(&input).await?;
//! ```
//!
//! ## Length-Based Selection
//!
//! ```rust,ignore
//! use dashflow::core::prompts::example_selector::LengthBasedExampleSelector;
//! use dashflow::core::prompts::PromptTemplate;
//!
//! let example_prompt = PromptTemplate::new(
//!     "Input: {input}\nOutput: {output}",
//!     vec!["input".to_string(), "output".to_string()],
//!     PromptTemplateFormat::FString,
//! );
//!
//! let selector = LengthBasedExampleSelector::new(
//!     examples,
//!     example_prompt,
//!     2048, // max_length
//!     None, // get_text_length
//! )?;
//!
//! let selected = selector.select_examples(&input)?;
//! ```

use async_trait::async_trait;
use std::collections::HashMap;

use crate::core::documents::Document;
use crate::core::error::Result;
use crate::core::prompts::PromptTemplate;
use crate::core::vector_stores::VectorStore;

/// Base trait for selecting examples to include in prompts.
///
/// Example selectors are used in few-shot prompting to dynamically choose
/// which examples to show to the LLM based on the input query. Different
/// implementations use different strategies (semantic similarity, length
/// constraints, diversity, etc.).
#[async_trait]
pub trait BaseExampleSelector: Send + Sync {
    /// Add a new example to the selector's store.
    ///
    /// # Arguments
    ///
    /// * `example` - A dictionary with keys as input variables and values as their values
    ///
    /// # Returns
    ///
    /// An ID or other identifier for the added example (implementation-specific)
    async fn add_example(&mut self, example: HashMap<String, String>) -> Result<String>;

    /// Select which examples to use based on the input variables.
    ///
    /// # Arguments
    ///
    /// * `input_variables` - A dictionary with keys as input variables and values as their values
    ///
    /// # Returns
    ///
    /// A list of selected examples (each example is a `HashMap`)
    async fn select_examples(
        &self,
        input_variables: &HashMap<String, String>,
    ) -> Result<Vec<HashMap<String, String>>>;
}

/// Helper function to return a list of values in dict sorted by key.
///
/// This is used internally to create consistent text representations
/// of examples for embedding.
#[must_use]
pub fn sorted_values(values: &HashMap<String, String>) -> Vec<String> {
    let mut keys: Vec<&String> = values.keys().collect();
    keys.sort();
    keys.iter().map(|k| values[*k].clone()).collect()
}

/// Select examples based on semantic similarity using a vector store.
///
/// This selector embeds examples and the input query, then retrieves the
/// most similar examples using vector similarity search. This is the most
/// common approach for dynamic few-shot prompting.
///
/// # Python Baseline
///
/// Matches `dashflow_core.example_selectors.semantic_similarity.SemanticSimilarityExampleSelector`
pub struct SemanticSimilarityExampleSelector<V: VectorStore> {
    /// Vector store containing embedded examples
    pub vectorstore: V,
    /// Number of examples to select
    pub k: usize,
    /// Optional keys to filter examples to when returning
    pub example_keys: Option<Vec<String>>,
    /// Optional keys to filter input to when searching (if None, use all keys)
    pub input_keys: Option<Vec<String>>,
    /// Extra arguments passed to `similarity_search`
    pub vectorstore_kwargs: Option<HashMap<String, serde_json::Value>>,
}

impl<V: VectorStore> SemanticSimilarityExampleSelector<V> {
    /// Create a new semantic similarity example selector.
    pub fn new(
        vectorstore: V,
        k: usize,
        example_keys: Option<Vec<String>>,
        input_keys: Option<Vec<String>>,
        vectorstore_kwargs: Option<HashMap<String, serde_json::Value>>,
    ) -> Self {
        Self {
            vectorstore,
            k,
            example_keys,
            input_keys,
            vectorstore_kwargs,
        }
    }

    /// Convert example to text for embedding.
    ///
    /// If `input_keys` is provided, only use those keys. Otherwise use all keys.
    /// Values are joined by space in sorted key order.
    fn example_to_text(
        example: &HashMap<String, String>,
        input_keys: &Option<Vec<String>>,
    ) -> String {
        if let Some(keys) = input_keys {
            let filtered: HashMap<String, String> = keys
                .iter()
                .filter_map(|k| example.get(k).map(|v| (k.clone(), v.clone())))
                .collect();
            sorted_values(&filtered).join(" ")
        } else {
            sorted_values(example).join(" ")
        }
    }

    /// Convert documents to examples by extracting metadata.
    ///
    /// Examples are stored in document metadata. If `example_keys` is provided,
    /// filter to only those keys.
    fn documents_to_examples(&self, documents: Vec<Document>) -> Vec<HashMap<String, String>> {
        let mut examples: Vec<HashMap<String, String>> = documents
            .into_iter()
            .map(|doc| {
                // Convert serde_json::Value to String
                doc.metadata
                    .into_iter()
                    .map(|(k, v)| {
                        // Convert Value to String (use as_str for strings, or to_string for others)
                        let string_val = match v {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (k, string_val)
                    })
                    .collect()
            })
            .collect();

        if let Some(keys) = &self.example_keys {
            examples = examples
                .into_iter()
                .map(|ex| {
                    keys.iter()
                        .filter_map(|k| ex.get(k).map(|v| (k.clone(), v.clone())))
                        .collect()
                })
                .collect();
        }

        examples
    }
}

#[async_trait]
impl<V: VectorStore> BaseExampleSelector for SemanticSimilarityExampleSelector<V> {
    async fn add_example(&mut self, example: HashMap<String, String>) -> Result<String> {
        let text = Self::example_to_text(&example, &self.input_keys);

        // Convert HashMap<String, String> to HashMap<String, serde_json::Value>
        let metadata: HashMap<String, serde_json::Value> = example
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        let texts = vec![text];
        let metadatas = vec![metadata];
        let ids = self
            .vectorstore
            .add_texts(&texts, Some(&metadatas), None)
            .await?;
        Ok(ids.into_iter().next().unwrap_or_default())
    }

    async fn select_examples(
        &self,
        input_variables: &HashMap<String, String>,
    ) -> Result<Vec<HashMap<String, String>>> {
        let query = Self::example_to_text(input_variables, &self.input_keys);

        // Perform similarity search
        let documents = self
            .vectorstore
            ._similarity_search(&query, self.k, None)
            .await?;

        Ok(self.documents_to_examples(documents))
    }
}

/// Select examples based on Max Marginal Relevance.
///
/// MMR balances relevance (similarity to query) with diversity (dissimilarity
/// to already selected examples). This prevents selecting redundant examples
/// and provides better coverage of the example space.
///
/// Reference: <https://arxiv.org/pdf/2211.13892.pdf>
///
/// # Python Baseline
///
/// Matches `dashflow_core.example_selectors.semantic_similarity.MaxMarginalRelevanceExampleSelector`
pub struct MaxMarginalRelevanceExampleSelector<V: VectorStore> {
    /// Vector store containing embedded examples
    pub vectorstore: V,
    /// Number of examples to select
    pub k: usize,
    /// Number of examples to fetch before reranking with MMR
    pub fetch_k: usize,
    /// Optional keys to filter examples to when returning
    pub example_keys: Option<Vec<String>>,
    /// Optional keys to filter input to when searching (if None, use all keys)
    pub input_keys: Option<Vec<String>>,
    /// Extra arguments passed to `max_marginal_relevance_search`
    pub vectorstore_kwargs: Option<HashMap<String, serde_json::Value>>,
}

impl<V: VectorStore> MaxMarginalRelevanceExampleSelector<V> {
    /// Create a new MMR example selector.
    pub fn new(
        vectorstore: V,
        k: usize,
        fetch_k: usize,
        example_keys: Option<Vec<String>>,
        input_keys: Option<Vec<String>>,
        vectorstore_kwargs: Option<HashMap<String, serde_json::Value>>,
    ) -> Self {
        Self {
            vectorstore,
            k,
            fetch_k,
            example_keys,
            input_keys,
            vectorstore_kwargs,
        }
    }

    /// Convert example to text for embedding (same logic as `SemanticSimilarity`).
    fn example_to_text(
        example: &HashMap<String, String>,
        input_keys: &Option<Vec<String>>,
    ) -> String {
        if let Some(keys) = input_keys {
            let filtered: HashMap<String, String> = keys
                .iter()
                .filter_map(|k| example.get(k).map(|v| (k.clone(), v.clone())))
                .collect();
            sorted_values(&filtered).join(" ")
        } else {
            sorted_values(example).join(" ")
        }
    }

    /// Convert documents to examples (same logic as `SemanticSimilarity`).
    fn documents_to_examples(&self, documents: Vec<Document>) -> Vec<HashMap<String, String>> {
        let mut examples: Vec<HashMap<String, String>> = documents
            .into_iter()
            .map(|doc| {
                // Convert serde_json::Value to String
                doc.metadata
                    .into_iter()
                    .map(|(k, v)| {
                        // Convert Value to String (use as_str for strings, or to_string for others)
                        let string_val = match v {
                            serde_json::Value::String(s) => s,
                            other => other.to_string(),
                        };
                        (k, string_val)
                    })
                    .collect()
            })
            .collect();

        if let Some(keys) = &self.example_keys {
            examples = examples
                .into_iter()
                .map(|ex| {
                    keys.iter()
                        .filter_map(|k| ex.get(k).map(|v| (k.clone(), v.clone())))
                        .collect()
                })
                .collect();
        }

        examples
    }
}

#[async_trait]
impl<V: VectorStore> BaseExampleSelector for MaxMarginalRelevanceExampleSelector<V> {
    async fn add_example(&mut self, example: HashMap<String, String>) -> Result<String> {
        let text = Self::example_to_text(&example, &self.input_keys);

        // Convert HashMap<String, String> to HashMap<String, serde_json::Value>
        let metadata: HashMap<String, serde_json::Value> = example
            .iter()
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        let texts = vec![text];
        let metadatas = vec![metadata];
        let ids = self
            .vectorstore
            .add_texts(&texts, Some(&metadatas), None)
            .await?;
        Ok(ids.into_iter().next().unwrap_or_default())
    }

    async fn select_examples(
        &self,
        input_variables: &HashMap<String, String>,
    ) -> Result<Vec<HashMap<String, String>>> {
        let query = Self::example_to_text(input_variables, &self.input_keys);

        // Perform MMR search with default lambda_mult=0.5 (from Python baseline)
        let documents = self
            .vectorstore
            .max_marginal_relevance_search(&query, self.k, self.fetch_k, 0.5, None)
            .await?;

        Ok(self.documents_to_examples(documents))
    }
}

/// Default length function that splits on whitespace and newlines.
///
/// This matches the Python baseline behavior.
fn get_length_based(text: &str) -> usize {
    text.split(|c: char| c.is_whitespace())
        .filter(|s| !s.is_empty())
        .count()
}

/// Select examples based on length to stay within token limits.
///
/// This selector adds examples in order until the cumulative length would
/// exceed the maximum length. This is practical for ensuring prompts don't
/// exceed context windows.
///
/// # Python Baseline
///
/// Matches `dashflow_core.example_selectors.length_based.LengthBasedExampleSelector`
pub struct LengthBasedExampleSelector {
    /// List of available examples
    pub examples: Vec<HashMap<String, String>>,
    /// Prompt template used to format each example
    pub example_prompt: PromptTemplate,
    /// Function to measure text length (defaults to word count)
    pub get_text_length: fn(&str) -> usize,
    /// Maximum length for the prompt
    pub max_length: usize,
    /// Cached lengths of formatted examples
    example_text_lengths: Vec<usize>,
}

impl LengthBasedExampleSelector {
    /// Create a new length-based example selector.
    ///
    /// # Arguments
    ///
    /// * `examples` - List of examples to select from
    /// * `example_prompt` - Template for formatting each example
    /// * `max_length` - Maximum total length (in units defined by `get_text_length`)
    /// * `get_text_length` - Optional custom length function (defaults to word count)
    ///
    /// # Errors
    ///
    /// Returns error if `example_prompt` fails to format any example
    pub fn new(
        examples: Vec<HashMap<String, String>>,
        example_prompt: PromptTemplate,
        max_length: usize,
        get_text_length: Option<fn(&str) -> usize>,
    ) -> Result<Self> {
        let get_text_length = get_text_length.unwrap_or(get_length_based);

        // Pre-compute lengths of all formatted examples
        let mut example_text_lengths = Vec::with_capacity(examples.len());
        for example in &examples {
            let formatted = example_prompt.format(example)?;
            example_text_lengths.push(get_text_length(&formatted));
        }

        Ok(Self {
            examples,
            example_prompt,
            get_text_length,
            max_length,
            example_text_lengths,
        })
    }

    /// Add a new example to the selector.
    ///
    /// This is a synchronous operation since it doesn't involve any I/O.
    pub fn add_example_sync(&mut self, example: HashMap<String, String>) -> Result<()> {
        let formatted = self.example_prompt.format(&example)?;
        let length = (self.get_text_length)(&formatted);
        self.examples.push(example);
        self.example_text_lengths.push(length);
        Ok(())
    }

    /// Select examples that fit within the maximum length.
    ///
    /// Examples are added in order until adding another would exceed `max_length`.
    ///
    /// # Arguments
    ///
    /// * `input_variables` - Input to the prompt (used to calculate remaining space)
    ///
    /// # Returns
    ///
    /// List of examples that fit within the length constraint
    pub fn select_examples_sync(
        &self,
        input_variables: &HashMap<String, String>,
    ) -> Result<Vec<HashMap<String, String>>> {
        // Calculate space taken by input
        let inputs = sorted_values(input_variables).join(" ");
        let mut remaining_length = self
            .max_length
            .saturating_sub((self.get_text_length)(&inputs));

        // Add examples until we run out of space
        let mut selected = Vec::new();
        for (i, example_length) in self.example_text_lengths.iter().enumerate() {
            if *example_length > remaining_length {
                break;
            }
            selected.push(self.examples[i].clone());
            remaining_length = remaining_length.saturating_sub(*example_length);
        }

        Ok(selected)
    }
}

#[async_trait]
impl BaseExampleSelector for LengthBasedExampleSelector {
    async fn add_example(&mut self, example: HashMap<String, String>) -> Result<String> {
        self.add_example_sync(example)?;
        Ok(String::new()) // Return empty ID (length-based selector doesn't use IDs)
    }

    async fn select_examples(
        &self,
        input_variables: &HashMap<String, String>,
    ) -> Result<Vec<HashMap<String, String>>> {
        self.select_examples_sync(input_variables)
    }
}

#[cfg(test)]
mod tests {
    use super::{get_length_based, sorted_values, BaseExampleSelector};
    use crate::core::prompts::PromptTemplateFormat;
    use crate::test_prelude::*;

    #[test]
    fn test_sorted_values() {
        let mut map = HashMap::new();
        map.insert("b".to_string(), "2".to_string());
        map.insert("a".to_string(), "1".to_string());
        map.insert("c".to_string(), "3".to_string());

        let values = sorted_values(&map);
        assert_eq!(values, vec!["1", "2", "3"]);
    }

    #[test]
    fn test_get_length_based() {
        assert_eq!(get_length_based("hello world"), 2);
        assert_eq!(get_length_based("hello\nworld"), 2);
        assert_eq!(get_length_based("  hello   world  "), 2);
        assert_eq!(get_length_based("one two three"), 3);
    }

    #[tokio::test]
    async fn test_length_based_selector() {
        let examples = vec![
            HashMap::from([
                ("input".to_string(), "2+2".to_string()),
                ("output".to_string(), "4".to_string()),
            ]),
            HashMap::from([
                ("input".to_string(), "3+3".to_string()),
                ("output".to_string(), "6".to_string()),
            ]),
            HashMap::from([
                ("input".to_string(), "4+4".to_string()),
                ("output".to_string(), "8".to_string()),
            ]),
        ];

        let example_prompt = PromptTemplate::new(
            "Input: {input}\nOutput: {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let selector = LengthBasedExampleSelector::new(
            examples.clone(),
            example_prompt,
            25, // max_length in words
            None,
        )
        .unwrap();

        let input = HashMap::from([("input".to_string(), "5+5".to_string())]);
        let selected = selector.select_examples(&input).await.unwrap();

        // Should select examples that fit within max_length
        assert!(!selected.is_empty());
        assert!(selected.len() <= examples.len());
    }

    #[tokio::test]
    async fn test_length_based_add_example() {
        let examples = vec![HashMap::from([
            ("input".to_string(), "2+2".to_string()),
            ("output".to_string(), "4".to_string()),
        ])];

        let example_prompt = PromptTemplate::new(
            "Input: {input}\nOutput: {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let mut selector =
            LengthBasedExampleSelector::new(examples, example_prompt, 100, None).unwrap();

        assert_eq!(selector.examples.len(), 1);

        let new_example = HashMap::from([
            ("input".to_string(), "3+3".to_string()),
            ("output".to_string(), "6".to_string()),
        ]);

        selector.add_example(new_example).await.unwrap();
        assert_eq!(selector.examples.len(), 2);
        assert_eq!(selector.example_text_lengths.len(), 2);
    }
}

#[cfg(test)]
mod tests_sorted_values {
    use super::sorted_values;
    use std::collections::HashMap;

    #[test]
    fn test_sorted_values_empty() {
        let map: HashMap<String, String> = HashMap::new();
        let values = sorted_values(&map);
        assert!(values.is_empty());
    }

    #[test]
    fn test_sorted_values_single() {
        let map = HashMap::from([("key".to_string(), "value".to_string())]);
        let values = sorted_values(&map);
        assert_eq!(values, vec!["value"]);
    }

    #[test]
    fn test_sorted_values_preserves_order_by_key() {
        // Keys should be sorted alphabetically, values returned in that order
        let map = HashMap::from([
            ("z".to_string(), "last".to_string()),
            ("a".to_string(), "first".to_string()),
            ("m".to_string(), "middle".to_string()),
        ]);
        let values = sorted_values(&map);
        assert_eq!(values, vec!["first", "middle", "last"]);
    }

    #[test]
    fn test_sorted_values_with_numeric_keys() {
        let map = HashMap::from([
            ("2".to_string(), "two".to_string()),
            ("10".to_string(), "ten".to_string()),
            ("1".to_string(), "one".to_string()),
        ]);
        let values = sorted_values(&map);
        // String sorting: "1" < "10" < "2"
        assert_eq!(values, vec!["one", "ten", "two"]);
    }
}

#[cfg(test)]
mod tests_length_based {
    use super::{get_length_based, LengthBasedExampleSelector};
    use crate::core::prompts::{PromptTemplate, PromptTemplateFormat};
    use std::collections::HashMap;

    #[test]
    fn test_get_length_based_empty() {
        assert_eq!(get_length_based(""), 0);
    }

    #[test]
    fn test_get_length_based_whitespace_only() {
        assert_eq!(get_length_based("   \t\n  "), 0);
    }

    #[test]
    fn test_get_length_based_tabs_and_newlines() {
        assert_eq!(get_length_based("hello\tworld\nfoo"), 3);
    }

    #[test]
    fn test_get_length_based_unicode() {
        assert_eq!(get_length_based("hello 世界"), 2);
        assert_eq!(get_length_based("日本語 中文 한국어"), 3);
    }

    #[test]
    fn test_length_based_selector_creation() {
        let examples = vec![HashMap::from([
            ("input".to_string(), "test".to_string()),
            ("output".to_string(), "result".to_string()),
        ])];

        let example_prompt = PromptTemplate::new(
            "Input: {input}\nOutput: {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let selector = LengthBasedExampleSelector::new(examples, example_prompt, 100, None);
        assert!(selector.is_ok());
    }

    #[test]
    fn test_length_based_selector_with_custom_length_fn() {
        let examples = vec![HashMap::from([
            ("input".to_string(), "test".to_string()),
            ("output".to_string(), "result".to_string()),
        ])];

        let example_prompt = PromptTemplate::new(
            "Input: {input}\nOutput: {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        // Custom length function that counts characters instead of words
        fn char_count(s: &str) -> usize {
            s.len()
        }

        let selector =
            LengthBasedExampleSelector::new(examples, example_prompt, 100, Some(char_count));
        assert!(selector.is_ok());
    }

    #[test]
    fn test_length_based_selector_empty_examples() {
        let examples: Vec<HashMap<String, String>> = vec![];

        let example_prompt = PromptTemplate::new(
            "Input: {input}\nOutput: {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let selector = LengthBasedExampleSelector::new(examples, example_prompt, 100, None);
        assert!(selector.is_ok());
        let selector = selector.unwrap();
        assert!(selector.examples.is_empty());
        assert!(selector.example_text_lengths.is_empty());
    }

    #[test]
    fn test_length_based_selector_select_sync() {
        let examples = vec![
            HashMap::from([
                ("input".to_string(), "short".to_string()),
                ("output".to_string(), "1".to_string()),
            ]),
            HashMap::from([
                ("input".to_string(), "medium length".to_string()),
                ("output".to_string(), "2".to_string()),
            ]),
        ];

        let example_prompt = PromptTemplate::new(
            "{input} -> {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let selector = LengthBasedExampleSelector::new(examples, example_prompt, 10, None).unwrap();

        let input = HashMap::from([("query".to_string(), "test".to_string())]);
        let selected = selector.select_examples_sync(&input).unwrap();

        // With max_length=10 and input taking 1 word, should fit some examples
        assert!(!selected.is_empty());
    }

    #[test]
    fn test_length_based_selector_max_length_exceeded() {
        let examples = vec![HashMap::from([
            ("input".to_string(), "very long input text here".to_string()),
            ("output".to_string(), "long output".to_string()),
        ])];

        let example_prompt = PromptTemplate::new(
            "{input} -> {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        // Very small max_length
        let selector = LengthBasedExampleSelector::new(examples, example_prompt, 2, None).unwrap();

        let input = HashMap::from([("query".to_string(), "test".to_string())]);
        let selected = selector.select_examples_sync(&input).unwrap();

        // Example is too long, should select nothing
        assert!(selected.is_empty());
    }

    #[test]
    fn test_length_based_selector_add_example_sync() {
        let examples = vec![HashMap::from([
            ("input".to_string(), "first".to_string()),
            ("output".to_string(), "1".to_string()),
        ])];

        let example_prompt = PromptTemplate::new(
            "{input} -> {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        let mut selector =
            LengthBasedExampleSelector::new(examples, example_prompt, 100, None).unwrap();

        assert_eq!(selector.examples.len(), 1);
        assert_eq!(selector.example_text_lengths.len(), 1);

        let new_example = HashMap::from([
            ("input".to_string(), "second".to_string()),
            ("output".to_string(), "2".to_string()),
        ]);

        selector.add_example_sync(new_example).unwrap();

        assert_eq!(selector.examples.len(), 2);
        assert_eq!(selector.example_text_lengths.len(), 2);
    }

    #[test]
    fn test_length_based_selector_respects_order() {
        let examples = vec![
            HashMap::from([
                ("input".to_string(), "first".to_string()),
                ("output".to_string(), "1".to_string()),
            ]),
            HashMap::from([
                ("input".to_string(), "second".to_string()),
                ("output".to_string(), "2".to_string()),
            ]),
            HashMap::from([
                ("input".to_string(), "third".to_string()),
                ("output".to_string(), "3".to_string()),
            ]),
        ];

        let example_prompt = PromptTemplate::new(
            "{input} -> {output}".to_string(),
            vec!["input".to_string(), "output".to_string()],
            PromptTemplateFormat::FString,
        );

        // Large max_length to fit all
        let selector =
            LengthBasedExampleSelector::new(examples.clone(), example_prompt, 1000, None).unwrap();

        let input = HashMap::new();
        let selected = selector.select_examples_sync(&input).unwrap();

        // Should return examples in order
        assert_eq!(selected.len(), 3);
        assert_eq!(selected[0].get("output").unwrap(), "1");
        assert_eq!(selected[1].get("output").unwrap(), "2");
        assert_eq!(selected[2].get("output").unwrap(), "3");
    }
}

#[cfg(test)]
mod tests_semantic_similarity {
    use super::SemanticSimilarityExampleSelector;
    use crate::core::embeddings::MockEmbeddings;
    use crate::core::vector_stores::InMemoryVectorStore;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_mock_vectorstore() -> InMemoryVectorStore {
        let embeddings = Arc::new(MockEmbeddings::new(3));
        InMemoryVectorStore::new(embeddings)
    }

    #[test]
    fn test_semantic_similarity_selector_creation() {
        let vectorstore = create_mock_vectorstore();
        let selector = SemanticSimilarityExampleSelector::new(
            vectorstore,
            5,    // k
            None, // example_keys
            None, // input_keys
            None, // vectorstore_kwargs
        );

        assert_eq!(selector.k, 5);
        assert!(selector.example_keys.is_none());
        assert!(selector.input_keys.is_none());
        assert!(selector.vectorstore_kwargs.is_none());
    }

    #[test]
    fn test_semantic_similarity_selector_with_options() {
        let vectorstore = create_mock_vectorstore();
        let selector = SemanticSimilarityExampleSelector::new(
            vectorstore,
            3,
            Some(vec!["input".to_string(), "output".to_string()]),
            Some(vec!["input".to_string()]),
            Some(HashMap::new()),
        );

        assert_eq!(selector.k, 3);
        assert!(selector.example_keys.is_some());
        assert!(selector.input_keys.is_some());
        assert!(selector.vectorstore_kwargs.is_some());
    }

    #[test]
    fn test_example_to_text_all_keys() {
        let example = HashMap::from([
            ("b".to_string(), "second".to_string()),
            ("a".to_string(), "first".to_string()),
        ]);

        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example, &None,
        );
        // sorted_values joins with space in sorted key order
        assert_eq!(text, "first second");
    }

    #[test]
    fn test_example_to_text_with_input_keys() {
        let example = HashMap::from([
            ("input".to_string(), "question".to_string()),
            ("output".to_string(), "answer".to_string()),
            ("metadata".to_string(), "extra".to_string()),
        ]);

        let input_keys = Some(vec!["input".to_string()]);
        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example,
            &input_keys,
        );
        // Only input key should be included
        assert_eq!(text, "question");
    }

    #[test]
    fn test_example_to_text_with_multiple_input_keys() {
        let example = HashMap::from([
            ("input".to_string(), "question".to_string()),
            ("context".to_string(), "background".to_string()),
            ("output".to_string(), "answer".to_string()),
        ]);

        let input_keys = Some(vec!["context".to_string(), "input".to_string()]);
        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example,
            &input_keys,
        );
        // Should be sorted by key: context comes before input
        assert_eq!(text, "background question");
    }

    #[test]
    fn test_example_to_text_missing_input_key() {
        let example = HashMap::from([("input".to_string(), "question".to_string())]);

        let input_keys = Some(vec!["input".to_string(), "missing".to_string()]);
        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example,
            &input_keys,
        );
        // Only existing key should be included
        assert_eq!(text, "question");
    }

    #[test]
    fn test_example_to_text_empty() {
        let example: HashMap<String, String> = HashMap::new();
        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example, &None,
        );
        assert_eq!(text, "");
    }

    #[test]
    fn test_example_to_text_single_key() {
        let example = HashMap::from([("only".to_string(), "value".to_string())]);
        let text = SemanticSimilarityExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example, &None,
        );
        assert_eq!(text, "value");
    }
}

#[cfg(test)]
mod tests_mmr {
    use super::MaxMarginalRelevanceExampleSelector;
    use crate::core::embeddings::MockEmbeddings;
    use crate::core::vector_stores::InMemoryVectorStore;
    use std::collections::HashMap;
    use std::sync::Arc;

    fn create_mock_vectorstore() -> InMemoryVectorStore {
        let embeddings = Arc::new(MockEmbeddings::new(3));
        InMemoryVectorStore::new(embeddings)
    }

    #[test]
    fn test_mmr_selector_creation() {
        let vectorstore = create_mock_vectorstore();
        let selector = MaxMarginalRelevanceExampleSelector::new(
            vectorstore,
            5,    // k
            20,   // fetch_k
            None, // example_keys
            None, // input_keys
            None, // vectorstore_kwargs
        );

        assert_eq!(selector.k, 5);
        assert_eq!(selector.fetch_k, 20);
        assert!(selector.example_keys.is_none());
        assert!(selector.input_keys.is_none());
        assert!(selector.vectorstore_kwargs.is_none());
    }

    #[test]
    fn test_mmr_selector_with_options() {
        let vectorstore = create_mock_vectorstore();
        let selector = MaxMarginalRelevanceExampleSelector::new(
            vectorstore,
            3,
            10,
            Some(vec!["input".to_string(), "output".to_string()]),
            Some(vec!["input".to_string()]),
            Some(HashMap::new()),
        );

        assert_eq!(selector.k, 3);
        assert_eq!(selector.fetch_k, 10);
        assert!(selector.example_keys.is_some());
        assert!(selector.input_keys.is_some());
        assert!(selector.vectorstore_kwargs.is_some());
    }

    #[test]
    fn test_mmr_example_to_text_all_keys() {
        let example = HashMap::from([
            ("z".to_string(), "last".to_string()),
            ("a".to_string(), "first".to_string()),
        ]);

        let text = MaxMarginalRelevanceExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example, &None,
        );
        assert_eq!(text, "first last");
    }

    #[test]
    fn test_mmr_example_to_text_with_input_keys() {
        let example = HashMap::from([
            ("input".to_string(), "query".to_string()),
            ("output".to_string(), "response".to_string()),
        ]);

        let input_keys = Some(vec!["input".to_string()]);
        let text = MaxMarginalRelevanceExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example,
            &input_keys,
        );
        assert_eq!(text, "query");
    }

    #[test]
    fn test_mmr_k_less_than_fetch_k() {
        let vectorstore = create_mock_vectorstore();
        // k should be less than fetch_k for MMR to work properly
        let selector = MaxMarginalRelevanceExampleSelector::new(
            vectorstore,
            3,  // k - final number of examples
            10, // fetch_k - candidates to consider
            None,
            None,
            None,
        );

        assert!(selector.k < selector.fetch_k);
    }

    #[test]
    fn test_mmr_example_to_text_empty() {
        let example: HashMap<String, String> = HashMap::new();
        let text = MaxMarginalRelevanceExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example, &None,
        );
        assert_eq!(text, "");
    }

    #[test]
    fn test_mmr_example_to_text_empty_input_keys() {
        let example = HashMap::from([
            ("a".to_string(), "value_a".to_string()),
            ("b".to_string(), "value_b".to_string()),
        ]);
        let input_keys = Some(vec![]); // Empty input keys
        let text = MaxMarginalRelevanceExampleSelector::<InMemoryVectorStore>::example_to_text(
            &example,
            &input_keys,
        );
        // Empty input_keys means filter to nothing
        assert_eq!(text, "");
    }
}

#[cfg(test)]
mod tests_documents_to_examples {
    use super::{MaxMarginalRelevanceExampleSelector, SemanticSimilarityExampleSelector};
    use crate::core::documents::Document;
    use crate::core::embeddings::MockEmbeddings;
    use crate::core::vector_stores::InMemoryVectorStore;
    use std::sync::Arc;

    fn create_mock_vectorstore() -> InMemoryVectorStore {
        let embeddings = Arc::new(MockEmbeddings::new(3));
        InMemoryVectorStore::new(embeddings)
    }

    #[test]
    fn test_documents_to_examples_string_values() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs = vec![Document::new("content")
            .with_metadata("input", serde_json::json!("question"))
            .with_metadata("output", serde_json::json!("answer"))];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].get("input").unwrap(), "question");
        assert_eq!(examples[0].get("output").unwrap(), "answer");
    }

    #[test]
    fn test_documents_to_examples_non_string_values() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs = vec![Document::new("content")
            .with_metadata("number", serde_json::json!(42))
            .with_metadata("boolean", serde_json::json!(true))];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 1);
        // Non-string values should be converted via to_string()
        assert_eq!(examples[0].get("number").unwrap(), "42");
        assert_eq!(examples[0].get("boolean").unwrap(), "true");
    }

    #[test]
    fn test_documents_to_examples_with_example_keys_filter() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(
                vectorstore,
                5,
                Some(vec!["input".to_string()]), // Only keep 'input' key
                None,
                None,
            );

        let docs = vec![Document::new("content")
            .with_metadata("input", serde_json::json!("question"))
            .with_metadata("output", serde_json::json!("answer"))
            .with_metadata("metadata", serde_json::json!("extra"))];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].len(), 1);
        assert!(examples[0].contains_key("input"));
        assert!(!examples[0].contains_key("output"));
        assert!(!examples[0].contains_key("metadata"));
    }

    #[test]
    fn test_documents_to_examples_empty() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs: Vec<Document> = vec![];
        let examples = selector.documents_to_examples(docs);
        assert!(examples.is_empty());
    }

    #[test]
    fn test_mmr_documents_to_examples() {
        let vectorstore = create_mock_vectorstore();
        let selector: MaxMarginalRelevanceExampleSelector<InMemoryVectorStore> =
            MaxMarginalRelevanceExampleSelector::new(vectorstore, 3, 10, None, None, None);

        let docs = vec![
            Document::new("content1").with_metadata("key", serde_json::json!("value1")),
            Document::new("content2").with_metadata("key", serde_json::json!("value2")),
        ];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 2);
        assert_eq!(examples[0].get("key").unwrap(), "value1");
        assert_eq!(examples[1].get("key").unwrap(), "value2");
    }

    #[test]
    fn test_documents_to_examples_array_metadata() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs =
            vec![Document::new("content").with_metadata("array", serde_json::json!([1, 2, 3]))];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 1);
        // Array values converted to string
        assert_eq!(examples[0].get("array").unwrap(), "[1,2,3]");
    }

    #[test]
    fn test_documents_to_examples_null_metadata() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs =
            vec![Document::new("content").with_metadata("null_field", serde_json::json!(null))];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 1);
        assert_eq!(examples[0].get("null_field").unwrap(), "null");
    }

    #[test]
    fn test_documents_to_examples_multiple_documents() {
        let vectorstore = create_mock_vectorstore();
        let selector: SemanticSimilarityExampleSelector<InMemoryVectorStore> =
            SemanticSimilarityExampleSelector::new(vectorstore, 5, None, None, None);

        let docs = vec![
            Document::new("content1").with_metadata("idx", serde_json::json!("0")),
            Document::new("content2").with_metadata("idx", serde_json::json!("1")),
            Document::new("content3").with_metadata("idx", serde_json::json!("2")),
        ];

        let examples = selector.documents_to_examples(docs);
        assert_eq!(examples.len(), 3);
        assert_eq!(examples[0].get("idx").unwrap(), "0");
        assert_eq!(examples[1].get("idx").unwrap(), "1");
        assert_eq!(examples[2].get("idx").unwrap(), "2");
    }
}
