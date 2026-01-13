//! Document hashing for change detection and deduplication
//!
//! Computes unique IDs for documents based on their content and metadata.
//! These hashes are used to detect when documents have changed and need re-indexing.

use crate::core::documents::Document;
use blake2::Blake2b512;
use serde_json;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Namespace UUID for generating deterministic document IDs
///
/// This constant ensures document hashes are unique to DashFlow and
/// won't collide with other UUID namespaces.
const NAMESPACE_UUID: Uuid = Uuid::from_u128(1984);

/// Hash algorithm for document fingerprinting
///
/// Different algorithms offer different trade-offs between speed and collision resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HashAlgorithm {
    /// SHA-1 (legacy, not recommended)
    ///
    /// **WARNING**: SHA-1 is not collision-resistant. A motivated attacker can
    /// construct different documents that hash to the same ID. Use SHA-256 or
    /// `BLAKE2b` for new applications.
    ///
    /// Provided for compatibility with Python `DashFlow` codebases using SHA-1.
    Sha1,

    /// SHA-256 (recommended, default)
    ///
    /// Good balance of security and performance. Widely supported and
    /// considered secure for most applications.
    #[default]
    Sha256,

    /// SHA-512 (maximum security)
    ///
    /// Slower than SHA-256 but provides maximum collision resistance.
    /// Use when security is paramount.
    Sha512,

    /// `BLAKE2b` (modern, fast)
    ///
    /// Faster than SHA-256 with comparable security. Good choice for
    /// high-throughput applications.
    Blake2b,
}

impl HashAlgorithm {
    /// Compute hash of a string using this algorithm
    fn hash_string(&self, input: &str) -> String {
        match self {
            HashAlgorithm::Sha1 => {
                let mut hasher = Sha1::new();
                hasher.update(input.as_bytes());
                let result = hasher.finalize();
                hex::encode(result)
            }
            HashAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(input.as_bytes());
                let result = hasher.finalize();
                hex::encode(result)
            }
            HashAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(input.as_bytes());
                let result = hasher.finalize();
                hex::encode(result)
            }
            HashAlgorithm::Blake2b => {
                let mut hasher = Blake2b512::new();
                hasher.update(input.as_bytes());
                let result = hasher.finalize();
                hex::encode(result)
            }
        }
    }

    /// Convert hash to UUID v5 (namespace-based)
    ///
    /// Used only for SHA-1 to match Python behavior
    fn hash_to_uuid(&self, hash_hex: &str) -> Uuid {
        Uuid::new_v5(&NAMESPACE_UUID, hash_hex.as_bytes())
    }
}

/// Hash a document to generate a unique ID
///
/// The ID is computed from both the `page_content` and metadata fields.
/// Any change to either will result in a different ID.
///
/// # Algorithm
///
/// 1. Hash the `page_content`
/// 2. Serialize metadata to JSON (sorted keys for consistency)
/// 3. Hash the serialized metadata
/// 4. Concatenate both hashes and hash again
/// 5. For SHA-1, convert to UUID v5; for others, use hex string
///
/// # Arguments
///
/// * `doc` - Document to hash
/// * `algorithm` - Hash algorithm to use
///
/// # Returns
///
/// Unique ID string derived from document content and metadata
///
/// # Panics
///
/// Panics if metadata cannot be serialized to JSON. Metadata should
/// contain only JSON-serializable types (strings, numbers, bools, arrays, objects).
///
/// # Example
///
/// ```
/// use dashflow::core::indexing::{hash_document, HashAlgorithm};
/// use dashflow::core::documents::Document;
///
/// let doc = Document::new("Hello world");
/// let id = hash_document(&doc, HashAlgorithm::Sha256);
/// println!("Document ID: {}", id);
/// ```
#[must_use]
#[allow(clippy::expect_used)] // serde_json serialization of BTreeMap is infallible for JSON-valid types
pub fn hash_document(doc: &Document, algorithm: HashAlgorithm) -> String {
    // Hash the content
    let content_hash = algorithm.hash_string(&doc.page_content);

    // Hash the metadata (serialize with sorted keys for determinism)
    let metadata_json = if doc.metadata.is_empty() {
        "{}".to_string()
    } else {
        // Convert HashMap to BTreeMap for sorted keys
        let sorted_metadata: BTreeMap<_, _> = doc.metadata.iter().collect();
        serde_json::to_string(&sorted_metadata)
            .expect("Failed to serialize document metadata - ensure it contains only JSON-serializable types")
    };
    let metadata_hash = algorithm.hash_string(&metadata_json);

    // Combine content and metadata hashes
    let combined = format!("{content_hash}{metadata_hash}");
    let final_hash = algorithm.hash_string(&combined);

    // For SHA-1, match Python's UUID behavior
    if algorithm == HashAlgorithm::Sha1 {
        algorithm.hash_to_uuid(&final_hash).to_string()
    } else {
        final_hash
    }
}

/// Hash a document with a custom key encoder function
///
/// Allows full control over ID generation. The function receives the document
/// and returns a string ID.
///
/// # Arguments
///
/// * `doc` - Document to hash
/// * `key_encoder` - Function that generates ID from document
///
/// # Example
///
/// ```
/// # use dashflow::core::indexing::hashing::hash_document_with_encoder;
/// # use dashflow::core::documents::Document;
/// let doc = Document::new("content").with_metadata("source", "file.txt");
/// let id = hash_document_with_encoder(&doc, |d| {
///     format!("{}:{}", d.metadata.get("source").unwrap().as_str().unwrap(), d.page_content.len())
/// });
/// assert_eq!(id, "file.txt:7");
/// ```
pub fn hash_document_with_encoder<F>(doc: &Document, key_encoder: F) -> String
where
    F: Fn(&Document) -> String,
{
    key_encoder(doc)
}

/// Deduplicate documents by ID, preserving order
///
/// Removes documents with duplicate IDs, keeping only the first occurrence.
/// This is useful when processing document batches that may contain duplicates.
///
/// # Arguments
///
/// * `docs` - Documents to deduplicate (must have IDs set)
///
/// # Returns
///
/// Vector of unique documents in original order
#[must_use]
pub fn deduplicate_documents(docs: Vec<Document>) -> Vec<Document> {
    let mut seen_ids = std::collections::HashSet::new();
    let mut result = Vec::new();

    for doc in docs {
        if let Some(ref id) = doc.id {
            if seen_ids.insert(id.clone()) {
                result.push(doc);
            }
        } else {
            // Document without ID is kept (shouldn't happen in indexing context)
            result.push(doc);
        }
    }

    result
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Test code uses unwrap for assertions
mod tests {
    use super::hash_document_with_encoder;
    use crate::test_prelude::*;

    #[test]
    fn test_hash_document_sha256() {
        let doc = Document::new("Hello world");
        let id = hash_document(&doc, HashAlgorithm::Sha256);

        // Hash should be deterministic
        let id2 = hash_document(&doc, HashAlgorithm::Sha256);
        assert_eq!(id, id2);

        // Hash should be hex string
        assert!(!id.is_empty());
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_document_different_content() {
        let doc1 = Document::new("Hello");
        let doc2 = Document::new("World");

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);

        // Different content should produce different hashes
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_hash_document_with_metadata() {
        let doc1 = Document::new("Hello");
        let doc2 = Document::new("Hello").with_metadata("source", "file.txt");

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);

        // Metadata difference should produce different hash
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_hash_document_metadata_order_independent() {
        // Metadata with different insertion order should produce same hash
        let mut doc1 = Document::new("Hello");
        doc1.metadata
            .insert("a".to_string(), serde_json::json!("1"));
        doc1.metadata
            .insert("b".to_string(), serde_json::json!("2"));

        let mut doc2 = Document::new("Hello");
        doc2.metadata
            .insert("b".to_string(), serde_json::json!("2"));
        doc2.metadata
            .insert("a".to_string(), serde_json::json!("1"));

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);

        assert_eq!(id1, id2);
    }

    #[test]
    fn test_hash_algorithms() {
        let doc = Document::new("Test document");

        let sha1_id = hash_document(&doc, HashAlgorithm::Sha1);
        let sha256_id = hash_document(&doc, HashAlgorithm::Sha256);
        let sha512_id = hash_document(&doc, HashAlgorithm::Sha512);
        let blake2b_id = hash_document(&doc, HashAlgorithm::Blake2b);

        // Different algorithms produce different hashes
        assert_ne!(sha1_id, sha256_id);
        assert_ne!(sha256_id, sha512_id);
        assert_ne!(sha512_id, blake2b_id);

        // SHA-1 produces UUID format
        assert!(Uuid::parse_str(&sha1_id).is_ok());

        // Others produce hex strings
        assert!(sha256_id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(sha512_id.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(blake2b_id.chars().all(|c| c.is_ascii_hexdigit()));

        // SHA-512 should be longer than SHA-256
        assert!(sha512_id.len() > sha256_id.len());
    }

    #[test]
    fn test_hash_document_with_encoder() {
        let doc = Document::new("content").with_metadata("source", "file.txt");

        let id = hash_document_with_encoder(&doc, |d| {
            format!(
                "custom_{}_{}",
                d.metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown"),
                d.page_content.len()
            )
        });

        assert_eq!(id, "custom_file.txt_7");
    }

    #[test]
    fn test_deduplicate_documents() {
        let doc1 = Document::new("First").with_id("id1");
        let doc2 = Document::new("Second").with_id("id2");
        let doc3 = Document::new("First duplicate").with_id("id1");
        let doc4 = Document::new("Third").with_id("id3");

        let docs = vec![doc1.clone(), doc2.clone(), doc3, doc4.clone()];
        let deduped = deduplicate_documents(docs);

        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].id.as_ref().unwrap(), "id1");
        assert_eq!(deduped[0].page_content, "First"); // First occurrence kept
        assert_eq!(deduped[1].id.as_ref().unwrap(), "id2");
        assert_eq!(deduped[2].id.as_ref().unwrap(), "id3");
    }

    #[test]
    fn test_default_algorithm() {
        let default = HashAlgorithm::default();
        assert_eq!(default, HashAlgorithm::Sha256);
    }

    // --- Edge Case Tests ---

    #[test]
    fn test_hash_empty_document() {
        let doc = Document::new("");
        let id = hash_document(&doc, HashAlgorithm::Sha256);

        // Empty document should produce valid hash
        assert!(!id.is_empty());
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));

        // Should be deterministic
        let id2 = hash_document(&doc, HashAlgorithm::Sha256);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_hash_very_large_document() {
        // 1MB document
        let large_content = "x".repeat(1024 * 1024);
        let doc = Document::new(large_content);

        let id = hash_document(&doc, HashAlgorithm::Sha256);
        assert!(!id.is_empty());

        // Should be deterministic even for large inputs
        let id2 = hash_document(&doc, HashAlgorithm::Sha256);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_hash_unicode_content() {
        let doc1 = Document::new("Hello 世界 🌍");
        let doc2 = Document::new("Different 文字 🚀");

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);

        // Different Unicode content produces different hashes
        assert_ne!(id1, id2);

        // Unicode hashing is deterministic
        let id1_repeat = hash_document(&doc1, HashAlgorithm::Sha256);
        assert_eq!(id1, id1_repeat);
    }

    #[test]
    fn test_hash_special_characters() {
        let doc = Document::new("Special: !@#$%^&*()[]{}\\|;:'\",<>?/\n\t\r");
        let id = hash_document(&doc, HashAlgorithm::Sha256);

        assert!(!id.is_empty());

        // Should be deterministic
        let id2 = hash_document(&doc, HashAlgorithm::Sha256);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_hash_whitespace_variations() {
        let doc1 = Document::new("Hello  World"); // two spaces
        let doc2 = Document::new("Hello World"); // one space
        let doc3 = Document::new("Hello\tWorld"); // tab

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);
        let id3 = hash_document(&doc3, HashAlgorithm::Sha256);

        // Different whitespace produces different hashes
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_hash_metadata_nested_json() {
        let mut doc = Document::new("content");
        doc.metadata.insert(
            "nested".to_string(),
            serde_json::json!({
                "level1": {
                    "level2": {
                        "value": 42
                    }
                }
            }),
        );

        let id = hash_document(&doc, HashAlgorithm::Sha256);
        assert!(!id.is_empty());

        // Should be deterministic with nested JSON
        let id2 = hash_document(&doc, HashAlgorithm::Sha256);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_hash_metadata_array() {
        let mut doc = Document::new("content");
        doc.metadata.insert(
            "tags".to_string(),
            serde_json::json!(["tag1", "tag2", "tag3"]),
        );

        let id = hash_document(&doc, HashAlgorithm::Sha256);

        // Different array should produce different hash
        let mut doc2 = Document::new("content");
        doc2.metadata
            .insert("tags".to_string(), serde_json::json!(["tag1", "tag2"]));

        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);
        assert_ne!(id, id2);
    }

    #[test]
    fn test_hash_metadata_number_types() {
        let mut doc1 = Document::new("content");
        doc1.metadata
            .insert("number".to_string(), serde_json::json!(42));

        let mut doc2 = Document::new("content");
        doc2.metadata
            .insert("number".to_string(), serde_json::json!(42.0));

        // Hash the documents to verify consistent behavior
        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);

        // Note: serde_json treats integer 42 and float 42.0 as different JSON values,
        // so they should produce different hashes. This test verifies this behavior.
        assert_ne!(id1, id2, "Integer and float with same numeric value should hash differently due to JSON serialization")
    }

    #[test]
    fn test_deduplicate_empty() {
        let deduped = deduplicate_documents(vec![]);
        assert_eq!(deduped.len(), 0);
    }

    #[test]
    fn test_deduplicate_single_document() {
        let doc = Document::new("content").with_id("id1");
        let deduped = deduplicate_documents(vec![doc.clone()]);

        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].id, doc.id);
    }

    #[test]
    fn test_deduplicate_all_duplicates() {
        let doc1 = Document::new("First").with_id("id1");
        let doc2 = Document::new("Second").with_id("id1");
        let doc3 = Document::new("Third").with_id("id1");

        let docs = vec![doc1.clone(), doc2, doc3];
        let deduped = deduplicate_documents(docs);

        // Only first occurrence kept
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].page_content, "First");
    }

    #[test]
    fn test_deduplicate_no_ids() {
        let doc1 = Document::new("First");
        let doc2 = Document::new("Second");
        let doc3 = Document::new("Third");

        let docs = vec![doc1.clone(), doc2.clone(), doc3.clone()];
        let deduped = deduplicate_documents(docs);

        // Documents without IDs are all kept
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn test_deduplicate_mixed_with_and_without_ids() {
        let doc1 = Document::new("With ID").with_id("id1");
        let doc2 = Document::new("No ID");
        let doc3 = Document::new("With ID duplicate").with_id("id1");
        let doc4 = Document::new("Another no ID");

        let docs = vec![doc1.clone(), doc2.clone(), doc3, doc4.clone()];
        let deduped = deduplicate_documents(docs);

        // One with ID, two without IDs
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].page_content, "With ID");
        assert_eq!(deduped[1].page_content, "No ID");
        assert_eq!(deduped[2].page_content, "Another no ID");
    }

    #[test]
    fn test_deduplicate_preserves_order() {
        let doc1 = Document::new("First").with_id("id1");
        let doc2 = Document::new("Second").with_id("id2");
        let doc3 = Document::new("Third").with_id("id3");
        let doc4 = Document::new("Second duplicate").with_id("id2");

        let docs = vec![doc1.clone(), doc2.clone(), doc3.clone(), doc4];
        let deduped = deduplicate_documents(docs);

        // Order preserved, duplicates removed
        assert_eq!(deduped.len(), 3);
        assert_eq!(deduped[0].page_content, "First");
        assert_eq!(deduped[1].page_content, "Second");
        assert_eq!(deduped[2].page_content, "Third");
    }

    #[test]
    fn test_hash_document_with_encoder_empty() {
        let doc = Document::new("");

        let id = hash_document_with_encoder(&doc, |d| format!("len:{}", d.page_content.len()));

        assert_eq!(id, "len:0");
    }

    #[test]
    fn test_hash_document_with_encoder_complex() {
        let doc = Document::new("content")
            .with_metadata("source", "file.txt")
            .with_metadata("page", 42);

        let id = hash_document_with_encoder(&doc, |d| {
            let source = d
                .metadata
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let page = d.metadata.get("page").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("{}:{}:{}", source, page, d.page_content.len())
        });

        assert_eq!(id, "file.txt:42:7");
    }

    #[test]
    fn test_sha1_uuid_format() {
        let doc = Document::new("Test");
        let id = hash_document(&doc, HashAlgorithm::Sha1);

        // Should be valid UUID format
        let uuid = Uuid::parse_str(&id);
        assert!(uuid.is_ok());

        // Should be deterministic
        let id2 = hash_document(&doc, HashAlgorithm::Sha1);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_hash_algorithms_determinism() {
        let doc = Document::new("Determinism test");

        // Each algorithm should be deterministic across multiple calls
        for algorithm in [
            HashAlgorithm::Sha1,
            HashAlgorithm::Sha256,
            HashAlgorithm::Sha512,
            HashAlgorithm::Blake2b,
        ] {
            let id1 = hash_document(&doc, algorithm);
            let id2 = hash_document(&doc, algorithm);
            let id3 = hash_document(&doc, algorithm);
            assert_eq!(id1, id2);
            assert_eq!(id2, id3);
        }
    }

    #[test]
    fn test_hash_content_vs_metadata_priority() {
        // Test that both content and metadata contribute to hash
        let doc1 = Document::new("content1").with_metadata("key", "value");
        let doc2 = Document::new("content2").with_metadata("key", "value");
        let doc3 = Document::new("content1").with_metadata("key", "different");

        let id1 = hash_document(&doc1, HashAlgorithm::Sha256);
        let id2 = hash_document(&doc2, HashAlgorithm::Sha256);
        let id3 = hash_document(&doc3, HashAlgorithm::Sha256);

        // Different content produces different hash
        assert_ne!(id1, id2);

        // Different metadata produces different hash
        assert_ne!(id1, id3);

        // All three should be unique
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_blake2b_output_length() {
        let doc = Document::new("test");
        let id = hash_document(&doc, HashAlgorithm::Blake2b);

        // BLAKE2b-512 produces 512 bits = 64 bytes = 128 hex characters
        assert_eq!(id.len(), 128);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sha512_output_length() {
        let doc = Document::new("test");
        let id = hash_document(&doc, HashAlgorithm::Sha512);

        // SHA-512 produces 512 bits = 64 bytes = 128 hex characters
        assert_eq!(id.len(), 128);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_sha256_output_length() {
        let doc = Document::new("test");
        let id = hash_document(&doc, HashAlgorithm::Sha256);

        // SHA-256 produces 256 bits = 32 bytes = 64 hex characters
        assert_eq!(id.len(), 64);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_deduplicate_stress() {
        // 1000 documents with 500 unique IDs (each ID appears twice)
        let mut docs = Vec::new();
        for i in 0..1000 {
            let id = format!("id{}", i / 2); // Each ID appears twice
            docs.push(Document::new(format!("content{}", i)).with_id(id));
        }

        let deduped = deduplicate_documents(docs);

        // Should have 500 unique documents (first occurrence of each ID)
        assert_eq!(deduped.len(), 500);

        // Verify all IDs are unique
        let mut seen_ids = std::collections::HashSet::new();
        for doc in &deduped {
            assert!(seen_ids.insert(doc.id.clone()));
        }
    }
}
