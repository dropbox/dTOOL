//! Chain constructors for common patterns
//!
//! This module provides functional constructors for building common chain patterns
//! using `DashFlow` Expression Language (LCEL). These replace the legacy Chain classes
//! with simpler Runnable compositions.
//!
//! # RAG Patterns
//!
//! - `create_stuff_documents_chain`: Format documents and pass to LLM
//! - `create_retrieval_chain`: Retrieve documents and combine with LLM
//!
//! These constructors follow the modern Python `DashFlow` API design.

use crate::core::documents::{format_document, Document};
use crate::core::error::Result;
use crate::core::prompts::PromptTemplate;

/// Default separator for joining formatted documents
pub const DEFAULT_DOCUMENT_SEPARATOR: &str = "\n\n";

/// Default document variable name in prompts
pub const DEFAULT_DOCUMENTS_KEY: &str = "context";

/// Format a list of documents into a single string
///
/// This helper function formats each document using a prompt template and joins them
/// with a separator string.
///
/// # Arguments
///
/// * `documents` - List of documents to format
/// * `document_prompt` - Prompt template for formatting each document
/// * `separator` - String to join formatted documents
///
/// # Returns
///
/// Single string with all formatted documents joined by separator
pub fn format_documents(
    documents: &[Document],
    document_prompt: &PromptTemplate,
    separator: &str,
) -> Result<String> {
    let formatted: Result<Vec<String>> = documents
        .iter()
        .map(|doc| format_document(doc, document_prompt))
        .collect();

    Ok(formatted?.join(separator))
}

// Note: Full LCEL-style chain constructors (create_stuff_documents_chain, create_retrieval_chain)
// require more Runnable infrastructure than currently implemented. They will be added in future
// iterations once we have:
// - Proper RunnablePassthrough.assign() implementation with type-level dict merging
// - PromptTemplate as a Runnable
// - Better support for dict transformations in LCEL
//
// For now, users can compose chains manually using:
// - format_documents() to prepare context
// - Retriever trait for document retrieval
// - Prompt templates for formatting
// - LLM invocation for generation
//
// Example manual chain composition:
// ```rust,ignore
// let docs = retriever._get_relevant_documents(query, None).await?;
// let context = format_documents(&docs, &doc_prompt, "\n\n")?;
// let mut vars = HashMap::new();
// vars.insert("context".to_string(), context);
// vars.insert("question".to_string(), query.to_string());
// let prompt_text = prompt.format(&vars)?;
// let answer = llm.invoke(prompt_text, None).await?;
// ```

#[cfg(test)]
mod tests {
    use crate::core::prompts::PromptTemplate;
    use crate::test_prelude::*;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn test_default_document_separator() {
        assert_eq!(DEFAULT_DOCUMENT_SEPARATOR, "\n\n");
    }

    #[test]
    fn test_default_documents_key() {
        assert_eq!(DEFAULT_DOCUMENTS_KEY, "context");
    }

    #[test]
    fn test_format_documents_empty() {
        let documents = vec![];
        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR).unwrap();
        assert_eq!(result, "");
    }

    #[test]
    fn test_format_documents_single() {
        let doc = Document::new("Hello, world!");
        let documents = vec![doc];
        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR).unwrap();
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_format_documents_multiple_default_separator() {
        let doc1 = Document::new("First document");
        let doc2 = Document::new("Second document");
        let doc3 = Document::new("Third document");
        let documents = vec![doc1, doc2, doc3];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR).unwrap();

        assert_eq!(
            result,
            "First document\n\nSecond document\n\nThird document"
        );
    }

    #[test]
    fn test_format_documents_multiple_custom_separator() {
        let doc1 = Document::new("First");
        let doc2 = Document::new("Second");
        let doc3 = Document::new("Third");
        let documents = vec![doc1, doc2, doc3];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, " | ").unwrap();

        assert_eq!(result, "First | Second | Third");
    }

    #[test]
    fn test_format_documents_with_template() {
        let doc1 = Document::new("Content 1");
        let doc2 = Document::new("Content 2");
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("Document: {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, "\n").unwrap();

        assert_eq!(result, "Document: Content 1\nDocument: Content 2");
    }

    #[test]
    fn test_format_documents_with_metadata() {
        let doc1 = Document::new("Content 1").with_metadata("source", "file1.txt".to_string());
        let doc2 = Document::new("Content 2").with_metadata("source", "file2.txt".to_string());
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("[{source}] {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, "\n").unwrap();

        assert_eq!(result, "[file1.txt] Content 1\n[file2.txt] Content 2");
    }

    #[test]
    fn test_format_documents_with_multiple_metadata_fields() {
        let mut metadata1 = HashMap::new();
        metadata1.insert("source".to_string(), json!("file1.txt"));
        metadata1.insert("page".to_string(), json!(1));

        let mut metadata2 = HashMap::new();
        metadata2.insert("source".to_string(), json!("file2.txt"));
        metadata2.insert("page".to_string(), json!(2));

        let doc1 = Document {
            page_content: "Content 1".to_string(),
            metadata: metadata1,
            id: None,
        };
        let doc2 = Document {
            page_content: "Content 2".to_string(),
            metadata: metadata2,
            id: None,
        };
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("[{source}:p{page}] {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, " | ").unwrap();

        assert_eq!(
            result,
            "[file1.txt:p1] Content 1 | [file2.txt:p2] Content 2"
        );
    }

    #[test]
    fn test_format_documents_empty_separator() {
        let doc1 = Document::new("First");
        let doc2 = Document::new("Second");
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, "").unwrap();

        assert_eq!(result, "FirstSecond");
    }

    #[test]
    fn test_format_documents_multiline_content() {
        let doc1 = Document::new("Line 1\nLine 2\nLine 3");
        let doc2 = Document::new("Another\nMultiline\nDocument");
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, "\n---\n").unwrap();

        assert_eq!(
            result,
            "Line 1\nLine 2\nLine 3\n---\nAnother\nMultiline\nDocument"
        );
    }

    #[test]
    fn test_format_documents_complex_template() {
        let doc1 = Document::new("Content about AI")
            .with_metadata("title", "AI Paper".to_string())
            .with_metadata("author", "Alice".to_string());

        let doc2 = Document::new("Content about ML")
            .with_metadata("title", "ML Paper".to_string())
            .with_metadata("author", "Bob".to_string());

        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template(
            "Title: {title}\nAuthor: {author}\nContent: {page_content}",
        )
        .unwrap();
        let result = format_documents(&documents, &prompt, "\n\n---\n\n").unwrap();

        let expected = "Title: AI Paper\nAuthor: Alice\nContent: Content about AI\n\n---\n\n\
                        Title: ML Paper\nAuthor: Bob\nContent: Content about ML";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_format_documents_error_missing_variable() {
        let doc = Document::new("Content without required metadata");
        let documents = vec![doc];

        // Template requires 'title' which document doesn't have
        let prompt = PromptTemplate::from_template("{title}: {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR);

        // Should return error because 'title' is missing
        assert!(result.is_err());
    }

    #[test]
    fn test_format_documents_single_with_custom_separator() {
        // Single document should work with any separator (separator not used)
        let doc = Document::new("Only document");
        let documents = vec![doc];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, "SHOULD_NOT_APPEAR").unwrap();

        assert_eq!(result, "Only document");
        assert!(!result.contains("SHOULD_NOT_APPEAR"));
    }

    #[test]
    fn test_format_documents_special_characters() {
        let doc1 = Document::new("Content with \"quotes\" and 'apostrophes'");
        let doc2 = Document::new("Content with <tags> and &ampersands&");
        let documents = vec![doc1, doc2];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, " || ").unwrap();

        assert_eq!(
            result,
            "Content with \"quotes\" and 'apostrophes' || Content with <tags> and &ampersands&"
        );
    }

    #[test]
    fn test_format_documents_unicode() {
        let doc1 = Document::new("日本語のコンテンツ");
        let doc2 = Document::new("Содержимое на русском");
        let doc3 = Document::new("محتوى عربي");
        let documents = vec![doc1, doc2, doc3];

        let prompt = PromptTemplate::from_template("{page_content}").unwrap();
        let result = format_documents(&documents, &prompt, " • ").unwrap();

        assert_eq!(
            result,
            "日本語のコンテンツ • Содержимое на русском • محتوى عربي"
        );
    }

    #[test]
    fn test_format_documents_numeric_metadata() {
        let mut metadata1 = HashMap::new();
        metadata1.insert("page".to_string(), json!(42));
        metadata1.insert("score".to_string(), json!(0.95));

        let doc = Document {
            page_content: "Content with numbers".to_string(),
            metadata: metadata1,
            id: None,
        };
        let documents = vec![doc];

        let prompt =
            PromptTemplate::from_template("Page {page} (score: {score}): {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR).unwrap();

        assert_eq!(result, "Page 42 (score: 0.95): Content with numbers");
    }

    #[test]
    fn test_format_documents_boolean_metadata() {
        let mut metadata = HashMap::new();
        metadata.insert("is_verified".to_string(), json!(true));

        let doc = Document {
            page_content: "Verified content".to_string(),
            metadata,
            id: None,
        };
        let documents = vec![doc];

        let prompt =
            PromptTemplate::from_template("[verified={is_verified}] {page_content}").unwrap();
        let result = format_documents(&documents, &prompt, DEFAULT_DOCUMENT_SEPARATOR).unwrap();

        assert_eq!(result, "[verified=true] Verified content");
    }
}
