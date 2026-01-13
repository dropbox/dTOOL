//! # KNNFewShot Optimizer - Retrieval-Augmented Few-Shot Learning
//!
//! This optimizer combines k-nearest neighbor retrieval with few-shot prompting.
//! For each input, it:
//! 1. Finds the k most similar examples from the training set using embeddings
//! 2. Uses those examples as demonstrations for the prediction
//!
//! This provides dynamic, input-aware few-shot learning that adapts demonstrations
//! to each specific query.
//!
//! ## Example
//!
//! ```rust,no_run
//! use dashflow::optimize::{KNNFewShot, Example};
//! use dashflow_openai::embeddings::OpenAIEmbeddings;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let trainset = vec![
//!     Example::from([("question", "What is 2+2?"), ("answer", "4")]),
//!     Example::from([("question", "What is the capital of France?"), ("answer", "Paris")]),
//! ];
//!
//! let embedder = Arc::new(OpenAIEmbeddings::new().with_model("text-embedding-3-small"));
//!
//! let optimizer = KNNFewShot::new(3, trainset, embedder).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## References
//!
//! - **Source**: DSPy framework
//! - **Paper**: "DSPy: Compiling Declarative Language Model Calls into Self-Improving Pipelines"
//! - **Link**: <https://arxiv.org/abs/2310.03714>
//! - **Original**: <https://github.com/stanfordnlp/dspy/blob/main/dspy/teleprompt/>

use super::OptimizerConfig;
use crate::core::embeddings::Embeddings;
use crate::optimize::telemetry::{record_optimization_complete, record_optimization_start};
use crate::optimize::{Example, FewShotExample, KNN};
use crate::Result;
use std::sync::Arc;

/// KNNFewShot optimizer for retrieval-augmented few-shot learning.
///
/// Unlike `LabeledFewShot` which uses fixed demonstrations for all inputs,
/// `KNNFewShot` dynamically selects the most relevant demonstrations for each
/// input based on embedding similarity.
///
/// # Architecture
///
/// The optimizer pre-computes embeddings for the training set during initialization.
/// At inference time, it embeds the input and retrieves the k most similar training
/// examples to use as few-shot demonstrations.
///
/// # Benefits
///
/// - **Adaptive**: Different inputs get different demonstrations based on similarity
/// - **Efficient**: No need to re-train or re-optimize for new examples
/// - **Effective**: Retrieval often outperforms random or fixed few-shot selection
pub struct KNNFewShot<E: Embeddings> {
    /// The KNN retriever for finding similar examples
    knn: KNN<E>,

    /// Maximum number of demonstrations to use (may be less than k if fewer neighbors exist)
    max_demos: usize,
}

impl<E: Embeddings> KNNFewShot<E> {
    /// Create a new KNNFewShot optimizer.
    ///
    /// This will pre-compute embeddings for all training examples.
    ///
    /// # Arguments
    ///
    /// * `k` - Number of nearest neighbors to retrieve per query
    /// * `trainset` - Training examples to retrieve from
    /// * `embedder` - Embedding client for vectorization
    ///
    /// # Returns
    ///
    /// A KNNFewShot optimizer ready to provide demonstrations.
    pub async fn new(k: usize, trainset: Vec<Example>, embedder: Arc<E>) -> Result<Self> {
        use std::time::Instant;
        let start = Instant::now();
        let trainset_len = trainset.len();

        // Record telemetry start
        record_optimization_start("knn_fewshot");

        let knn = KNN::new(k, trainset, embedder).await?;

        // Record telemetry completion
        // For KNN, "optimization" is building the index
        record_optimization_complete(
            "knn_fewshot",
            trainset_len as u64, // iterations = examples processed
            trainset_len as u64, // candidates = total examples
            0.0,                 // No initial score for index building
            1.0,                 // Success
            start.elapsed().as_secs_f64(),
        );

        Ok(Self {
            knn,
            max_demos: 16, // Default from Python DashOpt
        })
    }

    /// Set the maximum number of demonstrations to use.
    ///
    /// This controls how many of the retrieved k-nearest neighbors will be
    /// used as demonstrations. Defaults to 16.
    #[must_use]
    pub fn with_max_demos(mut self, max: usize) -> Self {
        self.max_demos = max;
        self
    }

    /// Retrieve few-shot examples for a given input.
    ///
    /// This method finds the k most similar training examples to the input
    /// and returns them as few-shot demonstrations.
    ///
    /// # Arguments
    ///
    /// * `input` - The input example to find similar demonstrations for
    ///
    /// # Returns
    ///
    /// A vector of few-shot examples (the k most similar training examples)
    pub async fn retrieve_demos(&self, input: &Example) -> Result<Vec<FewShotExample>> {
        // Retrieve k-nearest neighbors
        let nearest_examples = self.knn.retrieve(input).await?;

        // Convert to FewShotExample format
        let demos: Vec<FewShotExample> = nearest_examples
            .iter()
            .take(self.max_demos)
            .map(|example| {
                // Split into inputs and outputs
                let inputs = example.inputs();
                let outputs: serde_json::Map<String, serde_json::Value> = example
                    .data()
                    .iter()
                    .filter(|(key, _)| !inputs.contains_key(key.as_str()))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();

                FewShotExample {
                    input: serde_json::Value::Object(inputs),
                    output: serde_json::Value::Object(outputs),
                    reasoning: None, // KNN doesn't capture reasoning by default
                }
            })
            .collect();

        Ok(demos)
    }

    /// Get a reference to the underlying KNN retriever.
    pub fn knn(&self) -> &KNN<E> {
        &self.knn
    }

    /// Get the number of neighbors this optimizer retrieves.
    pub fn k(&self) -> usize {
        self.knn.k()
    }

    /// Get the size of the training set.
    pub fn trainset_size(&self) -> usize {
        self.knn.trainset_size()
    }

    /// Get optimizer configuration.
    pub fn get_config(&self) -> OptimizerConfig {
        OptimizerConfig {
            max_few_shot_examples: self.max_demos,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Error;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Mock embedding client for testing
    struct MockEmbedder {
        embeddings: Vec<Vec<f32>>,
        counter: Mutex<usize>,
    }

    impl MockEmbedder {
        fn new(embeddings: Vec<Vec<f32>>) -> Self {
            Self {
                embeddings,
                counter: Mutex::new(0),
            }
        }
    }

    #[async_trait]
    impl Embeddings for MockEmbedder {
        async fn _embed_documents(&self, texts: &[String]) -> std::result::Result<Vec<Vec<f32>>, Error> {
            let mut counter = self.counter.lock().unwrap();
            let mut result = Vec::new();
            for _ in texts {
                if *counter < self.embeddings.len() {
                    result.push(self.embeddings[*counter].clone());
                    *counter += 1;
                } else {
                    result.push(vec![0.0; self.embeddings[0].len()]);
                }
            }
            Ok(result)
        }

        async fn _embed_query(&self, _text: &str) -> std::result::Result<Vec<f32>, Error> {
            let mut counter = self.counter.lock().unwrap();
            if *counter < self.embeddings.len() {
                let embedding = self.embeddings[*counter].clone();
                *counter += 1;
                Ok(embedding)
            } else {
                Ok(vec![0.0; self.embeddings[0].len()])
            }
        }
    }

    #[tokio::test]
    async fn test_knn_fewshot_creation() {
        let trainset = vec![
            Example::from([("question", "What is 2+2?"), ("answer", "4")]),
            Example::from([("question", "What is 3+3?"), ("answer", "6")]),
        ];

        let embedder = Arc::new(MockEmbedder::new(vec![vec![1.0, 0.0], vec![0.0, 1.0]]));

        let optimizer = KNNFewShot::new(2, trainset, embedder).await.unwrap();
        assert_eq!(optimizer.k(), 2);
        assert_eq!(optimizer.trainset_size(), 2);
    }

    #[tokio::test]
    async fn test_knn_fewshot_retrieve_demos() {
        let trainset = vec![
            Example::from([("question", "What is 2+2?"), ("answer", "4")]),
            Example::from([
                ("question", "What is the capital of France?"),
                ("answer", "Paris"),
            ]),
            Example::from([("question", "What is 10-3?"), ("answer", "7")]),
        ];

        // Mock embeddings: math questions get similar embeddings
        // [1, 0] for "2+2", [0, 1] for "capital", [0.9, 0.1] for "10-3", [0.95, 0.05] for query
        let embedder = Arc::new(MockEmbedder::new(vec![
            vec![1.0, 0.0],   // "2+2" embedding
            vec![0.0, 1.0],   // "capital" embedding
            vec![0.9, 0.1],   // "10-3" embedding
            vec![0.95, 0.05], // query embedding (similar to math questions)
        ]));

        let optimizer = KNNFewShot::new(2, trainset, embedder).await.unwrap();

        // Query with a math question
        let query = Example::from([("question", "What is 5+5?")]);
        let demos = optimizer.retrieve_demos(&query).await.unwrap();

        // Should retrieve 2 most similar examples (math questions)
        assert_eq!(demos.len(), 2);

        // First demo should be "2+2" (most similar to query [0.95, 0.05])
        let first_input = demos[0].input.as_object().unwrap();
        assert_eq!(
            first_input.get("question").and_then(|v| v.as_str()),
            Some("What is 2+2?")
        );

        let first_output = demos[0].output.as_object().unwrap();
        assert_eq!(
            first_output.get("answer").and_then(|v| v.as_str()),
            Some("4")
        );
    }

    #[tokio::test]
    async fn test_knn_fewshot_with_max_demos() {
        let trainset = vec![
            Example::from([("input", "a")]),
            Example::from([("input", "b")]),
            Example::from([("input", "c")]),
        ];

        let embedder = Arc::new(MockEmbedder::new(vec![
            vec![1.0, 0.0],
            vec![0.9, 0.1],
            vec![0.8, 0.2],
            vec![1.0, 0.0],
        ]));

        // Set k=3 but max_demos=1
        let optimizer = KNNFewShot::new(3, trainset, embedder)
            .await
            .unwrap()
            .with_max_demos(1);

        let query = Example::from([("input", "query")]);
        let demos = optimizer.retrieve_demos(&query).await.unwrap();

        // Should only return 1 demo despite k=3
        assert_eq!(demos.len(), 1);
    }

    #[tokio::test]
    async fn test_knn_fewshot_config() {
        let trainset = vec![Example::from([("input", "test")])];

        let embedder = Arc::new(MockEmbedder::new(vec![vec![1.0, 0.0]]));

        let optimizer = KNNFewShot::new(3, trainset, embedder)
            .await
            .unwrap()
            .with_max_demos(8);

        let config = optimizer.get_config();
        assert_eq!(config.max_few_shot_examples, 8);
    }
}
