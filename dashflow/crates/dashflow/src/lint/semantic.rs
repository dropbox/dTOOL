//! Semantic search for type introspection.
//!
//! Provides TF-IDF based semantic similarity search for discovering types
//! by description. Works offline without requiring external API calls.
//!
//! # Usage
//!
//! ```rust,ignore
//! use dashflow::lint::semantic::{SemanticIndex, SimilarityResult};
//!
//! // Build index from type descriptions
//! let index = SemanticIndex::from_types(&types);
//!
//! // Search for similar types
//! let results = index.search("keyword search with BM25", 10);
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A TF-IDF vector representing text semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfIdfVector {
    /// Sparse representation: term index -> TF-IDF weight
    pub terms: HashMap<u32, f32>,
    /// L2 norm of the vector (precomputed for cosine similarity)
    pub norm: f32,
}

impl TfIdfVector {
    /// Create a zero vector
    pub fn zero() -> Self {
        Self {
            terms: HashMap::new(),
            norm: 0.0,
        }
    }

    /// Compute cosine similarity with another vector.
    ///
    /// Returns a value between 0.0 (orthogonal) and 1.0 (identical).
    pub fn cosine_similarity(&self, other: &TfIdfVector) -> f32 {
        if self.norm == 0.0 || other.norm == 0.0 {
            return 0.0;
        }

        let dot_product: f32 = self
            .terms
            .iter()
            .filter_map(|(term, weight)| other.terms.get(term).map(|w| weight * w))
            .sum();

        dot_product / (self.norm * other.norm)
    }
}

/// Vocabulary mapping terms to indices.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Vocabulary {
    /// Term to index mapping
    term_to_index: HashMap<String, u32>,
    /// Index to term mapping (for debugging)
    index_to_term: Vec<String>,
    /// Document frequency for each term (number of documents containing term)
    document_freq: Vec<u32>,
    /// Total number of documents in corpus
    total_documents: u32,
}

impl Vocabulary {
    /// Get or create an index for a term
    fn get_or_create_index(&mut self, term: &str) -> u32 {
        if let Some(&idx) = self.term_to_index.get(term) {
            return idx;
        }
        let idx = self.index_to_term.len() as u32;
        self.term_to_index.insert(term.to_string(), idx);
        self.index_to_term.push(term.to_string());
        self.document_freq.push(0);
        idx
    }

    /// Increment document frequency for a term
    fn increment_doc_freq(&mut self, term_idx: u32) {
        if let Some(freq) = self.document_freq.get_mut(term_idx as usize) {
            *freq += 1;
        }
    }

    /// Get IDF (Inverse Document Frequency) for a term
    fn idf(&self, term_idx: u32) -> f32 {
        let df = self
            .document_freq
            .get(term_idx as usize)
            .copied()
            .unwrap_or(1) as f32;
        let n = self.total_documents.max(1) as f32;
        // Standard IDF formula with smoothing
        ((n + 1.0) / (df + 1.0)).ln() + 1.0
    }

    /// Get index for an existing term (returns None if not in vocabulary)
    fn get_index(&self, term: &str) -> Option<u32> {
        self.term_to_index.get(term).copied()
    }
}

/// Semantic index for type descriptions.
///
/// Uses TF-IDF vectors for efficient semantic similarity search.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SemanticIndex {
    /// Vocabulary of terms
    vocabulary: Vocabulary,
    /// TF-IDF vectors for each type (indexed by type path)
    vectors: HashMap<String, TfIdfVector>,
}

impl SemanticIndex {
    /// Create a new empty semantic index
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a semantic index from type descriptions.
    ///
    /// # Arguments
    ///
    /// * `types` - Iterator of (path, description) pairs
    pub fn from_descriptions<S1, S2>(types: impl Iterator<Item = (S1, S2)>) -> Self
    where
        S1: AsRef<str>,
        S2: AsRef<str>,
    {
        let mut index = Self::new();
        let types_vec: Vec<_> = types
            .map(|(path, desc)| (path.as_ref().to_string(), desc.as_ref().to_string()))
            .collect();

        // First pass: build vocabulary and count document frequencies
        let mut doc_terms: Vec<HashMap<u32, u32>> = Vec::new();
        for (_, description) in &types_vec {
            let terms = tokenize(description);
            let mut term_counts: HashMap<u32, u32> = HashMap::new();

            for term in terms {
                let term_idx = index.vocabulary.get_or_create_index(&term);
                *term_counts.entry(term_idx).or_insert(0) += 1;
            }

            // Count document frequencies (unique terms per document)
            for &term_idx in term_counts.keys() {
                index.vocabulary.increment_doc_freq(term_idx);
            }

            doc_terms.push(term_counts);
        }

        index.vocabulary.total_documents = types_vec.len() as u32;

        // Second pass: compute TF-IDF vectors
        for ((path, _), term_counts) in types_vec.iter().zip(doc_terms.iter()) {
            let vector = index.compute_tfidf_vector(term_counts);
            index.vectors.insert(path.to_string(), vector);
        }

        index
    }

    /// Compute TF-IDF vector from term frequencies
    fn compute_tfidf_vector(&self, term_counts: &HashMap<u32, u32>) -> TfIdfVector {
        let mut terms: HashMap<u32, f32> = HashMap::new();
        let total_terms: u32 = term_counts.values().sum();

        for (&term_idx, &count) in term_counts {
            // Term frequency (normalized)
            let tf = count as f32 / total_terms.max(1) as f32;
            // Inverse document frequency
            let idf = self.vocabulary.idf(term_idx);
            // TF-IDF weight
            terms.insert(term_idx, tf * idf);
        }

        // Compute L2 norm
        let norm = terms.values().map(|v| v * v).sum::<f32>().sqrt();

        TfIdfVector { terms, norm }
    }

    /// Convert a query string to a TF-IDF vector.
    pub fn query_to_vector(&self, query: &str) -> TfIdfVector {
        let terms = tokenize(query);
        let mut term_counts: HashMap<u32, u32> = HashMap::new();

        for term in terms {
            if let Some(term_idx) = self.vocabulary.get_index(&term) {
                *term_counts.entry(term_idx).or_insert(0) += 1;
            }
        }

        self.compute_tfidf_vector(&term_counts)
    }

    /// Search for types similar to the query.
    ///
    /// # Arguments
    ///
    /// * `query` - Natural language query
    /// * `limit` - Maximum number of results
    ///
    /// # Returns
    ///
    /// Vector of (type_path, similarity_score) sorted by descending similarity.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SimilarityResult> {
        let query_vector = self.query_to_vector(query);

        if query_vector.norm == 0.0 {
            return Vec::new();
        }

        let mut results: Vec<_> = self
            .vectors
            .iter()
            .map(|(path, vector)| {
                let score = query_vector.cosine_similarity(vector);
                SimilarityResult {
                    type_path: path.clone(),
                    score,
                }
            })
            .filter(|r| r.score > 0.0)
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results.truncate(limit);
        results
    }

    /// Get the number of indexed types
    pub fn type_count(&self) -> usize {
        self.vectors.len()
    }

    /// Get the vocabulary size
    pub fn vocabulary_size(&self) -> usize {
        self.vocabulary.index_to_term.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Result of a semantic similarity search.
#[derive(Debug, Clone)]
pub struct SimilarityResult {
    /// Path to the matching type
    pub type_path: String,
    /// Similarity score (0.0 to 1.0)
    pub score: f32,
}

/// Tokenize text into terms for TF-IDF.
///
/// Performs:
/// - Lowercasing
/// - Splitting on whitespace and punctuation
/// - Removing common stop words (English and Rust-specific)
/// - Stemming (simple suffix stripping)
fn tokenize(text: &str) -> Vec<String> {
    // Common English stop words
    let english_stop_words: std::collections::HashSet<&str> = [
        "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
        "do", "does", "did", "will", "would", "could", "should", "may", "might", "must", "shall",
        "can", "need", "dare", "ought", "used", "to", "of", "in", "for", "on", "with", "at", "by",
        "from", "as", "into", "through", "during", "before", "after", "above", "below", "between",
        "under", "again", "further", "then", "once", "here", "there", "when", "where", "why",
        "how", "all", "each", "few", "more", "most", "other", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very", "just", "and", "but", "if", "or",
        "because", "until", "while", "this", "that", "these", "those", "it", "its",
    ]
    .into_iter()
    .collect();

    // Rust-specific stop words (common keywords that don't carry semantic meaning)
    let rust_stop_words: std::collections::HashSet<&str> = [
        "fn", "pub", "struct", "impl", "trait", "enum", "type", "const", "let", "mut", "ref",
        "self", "crate", "super", "async", "await", "dyn", "where", "mod", "use", "static",
        "unsafe", "extern", "move", "return", "match", "loop", "break", "continue", "default",
        "true", "false", "none", "some", "ok", "err", "option", "result", "string", "str", "vec",
        "box", "arc", "rc", "cell", "refcell", "mutex", "rwlock", "new", "clone",
    ]
    .into_iter()
    .collect();

    // Combine both stop word sets
    let stop_words: std::collections::HashSet<&str> = english_stop_words
        .union(&rust_stop_words)
        .copied()
        .collect();

    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty() && s.len() > 1 && !stop_words.contains(s))
        .map(stem)
        .collect()
}

/// Simple stemming (suffix stripping).
///
/// This is a simplified Porter stemmer for common English suffixes.
fn stem(word: &str) -> String {
    let word = word.to_lowercase();

    // Skip short words
    if word.len() <= 3 {
        return word;
    }

    // Common suffixes to strip (longer suffixes first to avoid partial matches)
    let suffixes = [
        "ation", "ition", "ement", "ment", "ness", "ious", "eous", "able", "ible", "ical", "ally",
        "ings", "ful", "less", "ive", "ize", "ise", "ing", "ed", "er", "est", "ly", "es", "s",
    ];

    for suffix in suffixes {
        if word.len() > suffix.len() + 2 && word.ends_with(suffix) {
            return word[..word.len() - suffix.len()].to_string();
        }
    }

    word
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("The quick brown fox jumps over the lazy dog");
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        // "the" should be filtered as stop word
        assert!(!tokens.contains(&"the".to_string()));
    }

    #[test]
    fn test_stem() {
        assert_eq!(stem("searching"), "search");
        assert_eq!(stem("retriever"), "retriev");
        assert_eq!(stem("embeddings"), "embedd");
    }

    #[test]
    fn test_semantic_index() {
        let types = vec![
            (
                "mod::BM25Retriever",
                "BM25 keyword search retriever for finding documents",
            ),
            ("mod::VectorStore", "Vector store for semantic embeddings"),
            ("mod::CostTracker", "Track API costs and token usage"),
        ];

        let index = SemanticIndex::from_descriptions(types.iter().map(|(p, d)| (*p, *d)));

        assert_eq!(index.type_count(), 3);

        // Search should find BM25Retriever for keyword-related query
        let results = index.search("keyword search", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].type_path, "mod::BM25Retriever");

        // Search should find VectorStore for embedding-related query
        let results = index.search("vector embeddings", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].type_path, "mod::VectorStore");
    }

    #[test]
    fn test_cosine_similarity() {
        let mut v1_terms = HashMap::new();
        v1_terms.insert(0, 1.0);
        v1_terms.insert(1, 0.0);
        let v1 = TfIdfVector {
            terms: v1_terms,
            norm: 1.0,
        };

        let mut v2_terms = HashMap::new();
        v2_terms.insert(0, 1.0);
        v2_terms.insert(1, 0.0);
        let v2 = TfIdfVector {
            terms: v2_terms,
            norm: 1.0,
        };

        // Identical vectors should have similarity 1.0
        assert!((v1.cosine_similarity(&v2) - 1.0).abs() < 0.001);
    }
}
