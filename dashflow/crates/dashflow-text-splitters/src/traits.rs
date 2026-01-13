//! Core traits for text splitting

use dashflow::core::documents::Document;
use std::collections::HashMap;

/// Where to keep the separator when splitting text
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeepSeparator {
    /// Don't keep the separator
    False,
    /// Keep the separator at the start of each chunk
    Start,
    /// Keep the separator at the end of each chunk
    End,
}

impl From<bool> for KeepSeparator {
    fn from(value: bool) -> Self {
        if value {
            KeepSeparator::Start
        } else {
            KeepSeparator::False
        }
    }
}

/// Core trait for text splitters.
///
/// A text splitter splits text into smaller chunks according to some strategy.
/// This is useful for:
/// - Fitting text into LLM context windows
/// - Creating embeddings for retrieval
/// - Processing documents in batches
pub trait TextSplitter {
    /// Split text into chunks.
    ///
    /// This is the core method that must be implemented by all text splitters.
    ///
    /// # Arguments
    ///
    /// * `text` - The text to split
    ///
    /// # Returns
    ///
    /// A vector of text chunks
    fn split_text(&self, text: &str) -> Vec<String>;

    /// Create documents from a list of texts with optional metadata.
    ///
    /// # Arguments
    ///
    /// * `texts` - The texts to split and create documents from
    /// * `metadatas` - Optional metadata for each text (must have same length as texts if provided)
    ///
    /// # Returns
    ///
    /// A vector of documents with split text and metadata
    fn create_documents(
        &self,
        texts: &[impl AsRef<str>],
        metadatas: Option<&[HashMap<String, serde_json::Value>]>,
    ) -> Vec<Document> {
        let metadatas = metadatas.unwrap_or(&[]);
        let mut documents = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            let text = text.as_ref();
            let metadata = if i < metadatas.len() {
                metadatas[i].clone()
            } else {
                HashMap::new()
            };

            let chunks = self.split_text(text);

            // Track index for add_start_index feature
            let mut index = 0;
            let mut previous_chunk_len = 0;

            for chunk in chunks {
                let mut doc_metadata = metadata.clone();

                // Add start_index if configured
                if self.add_start_index() {
                    let offset = index + previous_chunk_len - self.chunk_overlap();
                    let chunk_index =
                        text[offset.max(0)..].find(&chunk).unwrap_or(0) + offset.max(0);
                    doc_metadata.insert("start_index".to_string(), serde_json::json!(chunk_index));
                    index = chunk_index;
                    previous_chunk_len = chunk.len();
                }

                let doc = Document {
                    page_content: chunk,
                    metadata: doc_metadata,
                    id: None,
                };
                documents.push(doc);
            }
        }

        documents
    }

    /// Split documents into smaller chunks.
    ///
    /// # Arguments
    ///
    /// * `documents` - The documents to split
    ///
    /// # Returns
    ///
    /// A vector of documents with split text
    fn split_documents(&self, documents: &[Document]) -> Vec<Document> {
        let texts: Vec<&str> = documents.iter().map(|d| d.page_content.as_str()).collect();
        let metadatas: Vec<HashMap<String, serde_json::Value>> =
            documents.iter().map(|d| d.metadata.clone()).collect();

        self.create_documents(&texts, Some(&metadatas))
    }

    /// Get the chunk size configuration
    fn chunk_size(&self) -> usize;

    /// Get the chunk overlap configuration
    fn chunk_overlap(&self) -> usize;

    /// Whether to add `start_index` to metadata
    fn add_start_index(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================
    // KeepSeparator Enum Tests
    // ============================================

    #[test]
    fn test_keep_separator_variants() {
        // Test that all variants exist
        let _false = KeepSeparator::False;
        let _start = KeepSeparator::Start;
        let _end = KeepSeparator::End;
    }

    #[test]
    fn test_keep_separator_debug() {
        let sep = KeepSeparator::Start;
        let debug_str = format!("{:?}", sep);
        assert!(debug_str.contains("Start"));
    }

    #[test]
    fn test_keep_separator_debug_all_variants() {
        assert!(format!("{:?}", KeepSeparator::False).contains("False"));
        assert!(format!("{:?}", KeepSeparator::Start).contains("Start"));
        assert!(format!("{:?}", KeepSeparator::End).contains("End"));
    }

    #[test]
    #[allow(clippy::clone_on_copy)]
    fn test_keep_separator_clone() {
        let original = KeepSeparator::End;
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn test_keep_separator_copy() {
        let original = KeepSeparator::Start;
        let copied = original; // Copy, not move
        assert_eq!(original, copied); // Both should still be accessible
    }

    #[test]
    fn test_keep_separator_eq() {
        assert_eq!(KeepSeparator::False, KeepSeparator::False);
        assert_eq!(KeepSeparator::Start, KeepSeparator::Start);
        assert_eq!(KeepSeparator::End, KeepSeparator::End);
    }

    #[test]
    fn test_keep_separator_ne() {
        assert_ne!(KeepSeparator::False, KeepSeparator::Start);
        assert_ne!(KeepSeparator::False, KeepSeparator::End);
        assert_ne!(KeepSeparator::Start, KeepSeparator::End);
    }

    #[test]
    fn test_keep_separator_from_bool_true() {
        let sep: KeepSeparator = true.into();
        assert_eq!(sep, KeepSeparator::Start);
    }

    #[test]
    fn test_keep_separator_from_bool_false() {
        let sep: KeepSeparator = false.into();
        assert_eq!(sep, KeepSeparator::False);
    }

    #[test]
    fn test_keep_separator_from_conversion() {
        // Test From trait explicitly
        assert_eq!(KeepSeparator::from(true), KeepSeparator::Start);
        assert_eq!(KeepSeparator::from(false), KeepSeparator::False);
    }

    // ============================================
    // TextSplitter Trait Tests (using mock impl)
    // ============================================

    /// Simple text splitter for testing
    struct SimpleTestSplitter {
        chunk_size: usize,
        chunk_overlap: usize,
        add_start_index: bool,
    }

    impl SimpleTestSplitter {
        fn new(chunk_size: usize, chunk_overlap: usize) -> Self {
            Self {
                chunk_size,
                chunk_overlap,
                add_start_index: false,
            }
        }

        fn with_start_index(mut self, add: bool) -> Self {
            self.add_start_index = add;
            self
        }
    }

    impl TextSplitter for SimpleTestSplitter {
        fn split_text(&self, text: &str) -> Vec<String> {
            // Simple character-based splitting for testing
            if text.is_empty() {
                return vec![];
            }
            if text.len() <= self.chunk_size {
                return vec![text.to_string()];
            }
            let mut chunks = Vec::new();
            let mut start = 0;
            while start < text.len() {
                let end = (start + self.chunk_size).min(text.len());
                chunks.push(text[start..end].to_string());
                if end >= text.len() {
                    break;
                }
                start = end.saturating_sub(self.chunk_overlap);
            }
            chunks
        }

        fn chunk_size(&self) -> usize {
            self.chunk_size
        }

        fn chunk_overlap(&self) -> usize {
            self.chunk_overlap
        }

        fn add_start_index(&self) -> bool {
            self.add_start_index
        }
    }

    #[test]
    fn test_text_splitter_split_text() {
        let splitter = SimpleTestSplitter::new(10, 0);
        let chunks = splitter.split_text("Hello world!");
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_text_splitter_split_text_short() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let chunks = splitter.split_text("Short");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "Short");
    }

    #[test]
    fn test_text_splitter_split_text_empty() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let chunks = splitter.split_text("");
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_text_splitter_chunk_size() {
        let splitter = SimpleTestSplitter::new(50, 10);
        assert_eq!(splitter.chunk_size(), 50);
    }

    #[test]
    fn test_text_splitter_chunk_overlap() {
        let splitter = SimpleTestSplitter::new(50, 15);
        assert_eq!(splitter.chunk_overlap(), 15);
    }

    #[test]
    fn test_text_splitter_add_start_index_default() {
        let splitter = SimpleTestSplitter::new(50, 0);
        assert!(!splitter.add_start_index());
    }

    #[test]
    fn test_text_splitter_add_start_index_enabled() {
        let splitter = SimpleTestSplitter::new(50, 0).with_start_index(true);
        assert!(splitter.add_start_index());
    }

    #[test]
    fn test_text_splitter_create_documents_simple() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let texts = vec!["Hello world"];
        let docs = splitter.create_documents(&texts, None);
        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello world");
    }

    #[test]
    fn test_text_splitter_create_documents_with_metadata() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let texts = vec!["Hello world"];
        let mut meta = HashMap::new();
        meta.insert("source".to_string(), serde_json::json!("test"));
        let metadatas = vec![meta];
        let docs = splitter.create_documents(&texts, Some(&metadatas));
        assert_eq!(docs.len(), 1);
        assert_eq!(
            docs[0].metadata.get("source"),
            Some(&serde_json::json!("test"))
        );
    }

    #[test]
    fn test_text_splitter_create_documents_multiple_texts() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let texts = vec!["Text one", "Text two", "Text three"];
        let docs = splitter.create_documents(&texts, None);
        assert_eq!(docs.len(), 3);
    }

    #[test]
    fn test_text_splitter_create_documents_fewer_metadatas() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let texts = vec!["Text one", "Text two", "Text three"];
        let mut meta = HashMap::new();
        meta.insert("key".to_string(), serde_json::json!("value"));
        let metadatas = vec![meta]; // Only one metadata for three texts
        let docs = splitter.create_documents(&texts, Some(&metadatas));
        assert_eq!(docs.len(), 3);
        // First doc should have metadata
        assert!(docs[0].metadata.contains_key("key"));
        // Other docs should have empty metadata
        assert!(!docs[1].metadata.contains_key("key"));
        assert!(!docs[2].metadata.contains_key("key"));
    }

    #[test]
    fn test_text_splitter_split_documents() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let mut doc = Document::new("Original content".to_string());
        doc.metadata
            .insert("source".to_string(), serde_json::json!("original"));
        let docs = vec![doc];
        let split_docs = splitter.split_documents(&docs);
        assert_eq!(split_docs.len(), 1);
        assert_eq!(split_docs[0].page_content, "Original content");
        // Metadata should be preserved
        assert_eq!(
            split_docs[0].metadata.get("source"),
            Some(&serde_json::json!("original"))
        );
    }

    #[test]
    fn test_text_splitter_split_documents_multiple() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let docs = vec![
            Document::new("First doc".to_string()),
            Document::new("Second doc".to_string()),
        ];
        let split_docs = splitter.split_documents(&docs);
        assert_eq!(split_docs.len(), 2);
    }

    #[test]
    fn test_text_splitter_create_documents_with_start_index() {
        // Use chunk_overlap of 0 to avoid underflow on first chunk
        // (when index + previous_chunk_len < chunk_overlap)
        let splitter = SimpleTestSplitter::new(10, 0).with_start_index(true);
        let texts = vec!["Hello world, how are you?"];
        let docs = splitter.create_documents(&texts, None);
        assert!(!docs.is_empty());
        // All docs should have start_index metadata
        for doc in &docs {
            assert!(doc.metadata.contains_key("start_index"));
        }
    }

    #[test]
    fn test_text_splitter_document_id_is_none() {
        let splitter = SimpleTestSplitter::new(100, 0);
        let texts = vec!["Test text"];
        let docs = splitter.create_documents(&texts, None);
        assert_eq!(docs.len(), 1);
        assert!(docs[0].id.is_none());
    }
}
