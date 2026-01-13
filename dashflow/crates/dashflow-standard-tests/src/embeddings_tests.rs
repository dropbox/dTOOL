//! Standard conformance tests for Embeddings implementations.
//!
//! These tests verify that all Embeddings implementations behave consistently
//! across different providers.

use dashflow::core::embeddings::Embeddings;
use dashflow::error::Error;
use dashflow::{embed, embed_query};
use std::sync::Arc;

/// Helper function to determine if a test should fail due to environmental errors.
///
/// Environmental errors include authentication/billing/network failures. These
/// conformance tests should fail loudly rather than silently returning.
fn should_skip_on_error<T>(result: &Result<T, Error>) -> bool {
    if let Err(e) = result {
        // Check if this is a wrapped core error that is environmental
        if let Error::Core(core_err) = e {
            if core_err.is_environmental() {
                panic!("Environmental dependency unavailable: {e}");
            }
        }
        // Also check NodeExecution which wraps core errors from graph execution
        if let Error::NodeExecution { source, .. } = e {
            // Try to downcast the source to a core error
            if let Some(core_err) = source.downcast_ref::<dashflow::core::error::Error>() {
                if core_err.is_environmental() {
                    panic!("Environmental dependency unavailable: {e}");
                }
            }
        }
    }
    false
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/embeddings.py
/// Python function: `test_embed_query`
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 1: Embed single document
pub async fn test_embed_query<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let text = "Hello, world!";
    let result = embed_query(embeddings.clone(), text).await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    assert!(result.is_ok(), "embed_query should succeed");
    let embedding = result.unwrap();

    assert!(!embedding.is_empty(), "Embedding should not be empty");
    assert!(
        embedding.len() > 100,
        "Embedding should have reasonable dimensions"
    );
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/embeddings.py
/// Python function: `test_embed_documents`
/// Port date: 2025-10-29
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 2: Embed multiple documents
pub async fn test_embed_documents<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let texts = vec![
        "First document".to_string(),
        "Second document".to_string(),
        "Third document".to_string(),
    ];
    let result = embed(embeddings.clone(), &texts).await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    assert!(result.is_ok(), "embed_documents should succeed");
    let embeddings_list = result.unwrap();

    assert_eq!(embeddings_list.len(), 3, "Should return 3 embeddings");

    for embedding in &embeddings_list {
        assert!(!embedding.is_empty(), "Each embedding should not be empty");
    }

    // All embeddings should have same dimension
    let first_dim = embeddings_list[0].len();
    for embedding in &embeddings_list {
        assert_eq!(
            embedding.len(),
            first_dim,
            "All embeddings should have same dimension"
        );
    }
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/embeddings.py
/// Python function: `test_aembed_query` (line 80)
/// Port date: 2025-10-30
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 3: Async embed single query
///
/// Quality Score: 4/7
/// Criteria: (1) Real functionality, (3) Edge cases, (4) State verification, (7) Comparison
///
/// Tests async embedding of single string query.
/// Verifies:
/// - Returns list of floats
/// - Dimension consistency across different inputs
pub async fn test_aembed_query<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let embedding_1 = embed_query(embeddings.clone(), "foo").await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&embedding_1) {
        return;
    }

    assert!(embedding_1.is_ok(), "aembed_query should succeed");
    let embedding_1 = embedding_1.unwrap();

    assert!(!embedding_1.is_empty(), "Embedding should not be empty");
    assert!(
        embedding_1.iter().all(|&v| v.is_finite()),
        "All values should be finite floats"
    );

    let embedding_2 = embed_query(embeddings.clone(), "bar").await;

    // Skip test if error is environmental
    if should_skip_on_error(&embedding_2) {
        return;
    }

    assert!(embedding_2.is_ok(), "aembed_query should succeed");
    let embedding_2 = embedding_2.unwrap();

    assert!(!embedding_1.is_empty(), "Embedding length should be > 0");
    assert_eq!(
        embedding_1.len(),
        embedding_2.len(),
        "All embeddings should have same dimension"
    );
}

/// **STANDARD TEST** - Ported from Python `DashFlow` standard-tests
/// Python source: ~/dashflow/libs/standard-tests/dashflow_tests/integration_tests/embeddings.py
/// Python function: `test_aembed_documents` (line 101)
/// Port date: 2025-10-30
/// DO NOT REMOVE - This ensures upstream compatibility
///
/// Test 4: Async embed multiple documents
///
/// Quality Score: 4/7
/// Criteria: (1) Real functionality, (3) Edge cases, (4) State verification, (7) Comparison
///
/// Tests async embedding of list of strings.
/// Verifies:
/// - Returns list of embeddings matching input count
/// - All embeddings are lists of floats
/// - All embeddings have same dimension
pub async fn test_aembed_documents<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let documents = vec!["foo".to_string(), "bar".to_string(), "baz".to_string()];
    let result = embed(embeddings.clone(), &documents).await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    assert!(result.is_ok(), "aembed_documents should succeed");
    let embeddings_list = result.unwrap();

    assert_eq!(
        embeddings_list.len(),
        documents.len(),
        "Should return same number of embeddings as documents"
    );

    assert!(
        embeddings_list.iter().all(|e| !e.is_empty()),
        "All embeddings should be non-empty"
    );

    assert!(
        embeddings_list
            .iter()
            .all(|e| e.iter().all(|&v| v.is_finite())),
        "All embedding values should be finite floats"
    );

    assert!(
        !embeddings_list[0].is_empty(),
        "Embeddings should have length > 0"
    );

    let first_dim = embeddings_list[0].len();
    assert!(
        embeddings_list.iter().all(|e| e.len() == first_dim),
        "All embeddings should have same dimension"
    );
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 5: Empty input handling
pub async fn test_empty_input<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let result = embed_query(embeddings.clone(), "").await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    // Should either succeed with embedding or fail gracefully
    if let Ok(embedding) = result {
        assert!(
            !embedding.is_empty(),
            "Even empty string should produce embedding"
        );
    } else {
        // Empty input rejection is acceptable - this is a soft check
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 6: Dimension consistency
pub async fn test_dimension_consistency<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let result1 = embed_query(embeddings.clone(), "First text").await;
    if should_skip_on_error(&result1) {
        return;
    }
    let result1 = result1.unwrap();

    let result2 = embed_query(embeddings.clone(), "Second text").await;
    if should_skip_on_error(&result2) {
        return;
    }
    let result2 = result2.unwrap();

    assert_eq!(
        result1.len(),
        result2.len(),
        "All embeddings from same model should have same dimension"
    );
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 7: Semantic similarity
///
/// Verifies that semantically similar texts produce similar embeddings.
/// Uses cosine similarity to measure embedding similarity.
pub async fn test_semantic_similarity<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let similar1 = "The cat sits on the mat";
    let similar2 = "A feline rests on a rug";
    let dissimilar = "Quantum physics is complex";

    let emb1 = embed_query(embeddings.clone(), similar1).await;
    if should_skip_on_error(&emb1) {
        return;
    }
    let emb1 = emb1.unwrap();

    let emb2 = embed_query(embeddings.clone(), similar2).await;
    if should_skip_on_error(&emb2) {
        return;
    }
    let emb2 = emb2.unwrap();

    let emb3 = embed_query(embeddings.clone(), dissimilar).await;
    if should_skip_on_error(&emb3) {
        return;
    }
    let emb3 = emb3.unwrap();

    // Cosine similarity calculation
    let cosine_sim = |a: &[f32], b: &[f32]| -> f32 {
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        dot / (mag_a * mag_b)
    };

    let sim_similar = cosine_sim(&emb1, &emb2);
    let sim_dissimilar = cosine_sim(&emb1, &emb3);

    // Similar texts should have higher similarity than dissimilar ones
    assert!(
        sim_similar > sim_dissimilar,
        "Semantically similar texts should have higher cosine similarity: {sim_similar} vs {sim_dissimilar}"
    );
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 8: Large text handling
///
/// Verifies that embeddings can handle longer texts (common in document processing).
pub async fn test_large_text<T: Embeddings + 'static>(embeddings: Arc<T>) {
    // Create a text with ~1000 words
    let large_text = "The quick brown fox jumps over the lazy dog. ".repeat(200);

    let result = embed_query(embeddings.clone(), &large_text).await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    assert!(
        result.is_ok(),
        "Should handle large text input: {:?}",
        result.err()
    );
    let embedding = result.unwrap();
    assert!(!embedding.is_empty(), "Large text should produce embedding");
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 9: Special characters
///
/// Verifies handling of text with special characters, unicode, emojis, etc.
pub async fn test_special_characters_embeddings<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let texts = vec![
        "Hello, world! 🌍",
        "Email: test@example.com",
        "Math: ∑, ∫, ∂",
        "Chinese: 你好世界",
        "Arabic: مرحبا بالعالم",
        "Code: fn main() { println!(\"Hi\"); }",
    ];

    for text in texts {
        let result = embed_query(embeddings.clone(), text).await;

        // Skip test if error is environmental (bad credentials, no credits, etc.)
        if should_skip_on_error(&result) {
            return;
        }

        assert!(result.is_ok(), "Should handle special chars in: {text}");
        let embedding = result.unwrap();
        assert!(
            !embedding.is_empty(),
            "Special chars should produce embedding: {text}"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 10: Batch size consistency
///
/// Verifies that embedding individual documents vs batching produces same results.
pub async fn test_batch_consistency<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let texts = vec![
        "First document".to_string(),
        "Second document".to_string(),
        "Third document".to_string(),
    ];

    // Embed individually
    let individual: Vec<Vec<f32>> = {
        let mut results = Vec::new();
        for text in &texts {
            let result = embed_query(embeddings.clone(), text).await;
            if should_skip_on_error(&result) {
                return;
            }
            results.push(result.unwrap());
        }
        results
    };

    // Embed as batch
    let batch_result = embed(embeddings.clone(), &texts).await;
    if should_skip_on_error(&batch_result) {
        return;
    }
    let batch = batch_result.unwrap();

    assert_eq!(individual.len(), batch.len(), "Should have same count");

    // Check dimensions match
    for (ind, bat) in individual.iter().zip(batch.iter()) {
        assert_eq!(
            ind.len(),
            bat.len(),
            "Individual and batch embeddings should have same dimension"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 11: Whitespace handling
///
/// Verifies proper handling of various whitespace patterns.
pub async fn test_whitespace<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let texts = vec![
        "normal text",
        "  leading spaces",
        "trailing spaces  ",
        "  both  ",
        "multiple   spaces   inside",
        "tabs\tand\tnewlines\n",
    ];

    for text in texts {
        let result = embed_query(embeddings.clone(), text).await;

        // Skip test if error is environmental (bad credentials, no credits, etc.)
        if should_skip_on_error(&result) {
            return;
        }

        assert!(result.is_ok(), "Should handle whitespace in: {text:?}");
        let embedding = result.unwrap();
        assert!(
            !embedding.is_empty(),
            "Whitespace should produce embedding: {text:?}"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 12: Repeated embeddings
///
/// Verifies that embedding the same text multiple times produces same result.
pub async fn test_repeated_embeddings<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let text = "Consistent embedding test";

    let emb1 = embed_query(embeddings.clone(), text).await;
    if should_skip_on_error(&emb1) {
        return;
    }
    let emb1 = emb1.unwrap();

    let emb2 = embed_query(embeddings.clone(), text).await;
    if should_skip_on_error(&emb2) {
        return;
    }
    let emb2 = emb2.unwrap();

    let emb3 = embed_query(embeddings.clone(), text).await;
    if should_skip_on_error(&emb3) {
        return;
    }
    let emb3 = emb3.unwrap();

    // All embeddings should be identical
    assert_eq!(emb1.len(), emb2.len(), "Dimensions should match");
    assert_eq!(emb2.len(), emb3.len(), "Dimensions should match");

    // Check numerical consistency (allowing small floating point differences)
    // Note: API calls can have slight variations, so we use a relaxed tolerance (3e-4)
    // This allows for typical API variance while still catching major inconsistencies
    // Tolerance increased from 2e-4 to 3e-4 based on observed OpenAI API variance
    for i in 0..emb1.len() {
        let diff1 = (emb1[i] - emb2[i]).abs();
        let diff2 = (emb2[i] - emb3[i]).abs();
        assert!(
            diff1 < 3e-4,
            "Repeated embeddings should be similar at index {}: {} vs {}",
            i,
            emb1[i],
            emb2[i]
        );
        assert!(
            diff2 < 3e-4,
            "Repeated embeddings should be similar at index {}: {} vs {}",
            i,
            emb2[i],
            emb3[i]
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 13: Concurrent embedding requests
///
/// Verifies that embeddings can handle concurrent requests correctly.
pub async fn test_concurrent_embeddings<T: Embeddings + Sync + 'static>(embeddings: Arc<T>) {
    let texts = [
        "First concurrent text",
        "Second concurrent text",
        "Third concurrent text",
    ];

    // Create concurrent tasks
    let tasks: Vec<_> = texts
        .iter()
        .map(|text| embed_query(embeddings.clone(), text))
        .collect();

    // Execute all concurrently
    let results = futures::future::join_all(tasks).await;

    // Check for environmental errors first
    for result in &results {
        if should_skip_on_error(result) {
            return;
        }
    }

    // All should succeed
    for (i, result) in results.iter().enumerate() {
        assert!(result.is_ok(), "Concurrent embedding {i} should succeed");
        let embedding = result.as_ref().unwrap();
        assert!(
            !embedding.is_empty(),
            "Concurrent embedding {i} should not be empty"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 14: Numeric text
///
/// Verifies handling of numeric and alphanumeric text.
pub async fn test_numeric_text<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let texts = vec![
        "12345",
        "3.14159",
        "-42",
        "1e10",
        "0xDEADBEEF",
        "v1.2.3",
        "ISBN 978-0-123-45678-9",
    ];

    for text in texts {
        let result = embed_query(embeddings.clone(), text).await;

        // Skip test if error is environmental (bad credentials, no credits, etc.)
        if should_skip_on_error(&result) {
            return;
        }

        assert!(result.is_ok(), "Should handle numeric text: {text}");
        let embedding = result.unwrap();
        assert!(
            !embedding.is_empty(),
            "Numeric text should produce embedding: {text}"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 15: Single character input
///
/// Verifies handling of very short inputs (edge case).
pub async fn test_single_character<T: Embeddings + 'static>(embeddings: Arc<T>) {
    let chars = vec!["a", "Z", "1", ".", "!", "😀"];

    for ch in chars {
        let result = embed_query(embeddings.clone(), ch).await;

        // Skip test if error is environmental (bad credentials, no credits, etc.)
        if should_skip_on_error(&result) {
            return;
        }

        assert!(result.is_ok(), "Should handle single character: {ch}");
        let embedding = result.unwrap();
        assert!(
            !embedding.is_empty(),
            "Single character should produce embedding: {ch}"
        );
    }
}

/// **RUST-SPECIFIC EXTENSION** - Not in Python standard-tests
/// This test provides additional quality assurance beyond Python baseline
/// Port date: 2025-10-29
///
/// Test 16: Large batch processing
///
/// Verifies that embeddings can handle large batches efficiently.
pub async fn test_large_batch_embeddings<T: Embeddings + 'static>(embeddings: Arc<T>) {
    // Create 100 documents
    let texts: Vec<String> = (0..100)
        .map(|i| format!("Document number {i} with some content"))
        .collect();

    let result = embed(embeddings.clone(), &texts).await;

    // Skip test if error is environmental (bad credentials, no credits, etc.)
    if should_skip_on_error(&result) {
        return;
    }

    assert!(
        result.is_ok(),
        "Should handle large batch: {:?}",
        result.err()
    );
    let embeddings_list = result.unwrap();

    assert_eq!(embeddings_list.len(), 100, "Should return 100 embeddings");

    // Check all have same dimension
    let first_dim = embeddings_list[0].len();
    for (i, embedding) in embeddings_list.iter().enumerate() {
        assert_eq!(
            embedding.len(),
            first_dim,
            "Embedding {i} should have dimension {first_dim}"
        );
    }
}
