//! Chains for combining documents in various ways
//!
//! This module provides different strategies for combining multiple documents
//! into a single output using language models.

mod map_reduce;
mod refine;
mod stuff;

pub use map_reduce::MapReduceDocumentsChain;
pub use refine::RefineDocumentsChain;
pub use stuff::StuffDocumentsChain;

use dashflow::core::documents::Document;
use dashflow::core::error::Result;
use dashflow::core::prompts::PromptTemplate;
use std::collections::HashMap;

/// Default separator for joining formatted documents
pub const DEFAULT_DOCUMENT_SEPARATOR: &str = "\n\n";

/// Default document variable name in prompts
pub const DEFAULT_DOCUMENTS_KEY: &str = "context";

/// Default prompt template for formatting individual documents
#[must_use]
pub fn default_document_prompt() -> PromptTemplate {
    #[allow(clippy::expect_used)]
    let prompt = PromptTemplate::from_template("{page_content}")
        .expect("default document prompt should be valid");
    prompt
}

/// Format a single document using a prompt template
///
/// # Arguments
///
/// * `document` - The document to format
/// * `prompt` - The prompt template to use for formatting
///
/// # Returns
///
/// Formatted document string
pub fn format_document(document: &Document, prompt: &PromptTemplate) -> Result<String> {
    let mut vars = HashMap::new();

    // Add page_content
    vars.insert("page_content".to_string(), document.page_content.clone());

    // Add all metadata fields
    for (key, value) in &document.metadata {
        if let Some(s) = value.as_str() {
            vars.insert(key.clone(), s.to_string());
        } else {
            vars.insert(key.clone(), value.to_string());
        }
    }

    prompt.format(&vars)
}

/// Format multiple documents into a single string
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_document() {
        let doc = Document::new("Hello world");
        let prompt = PromptTemplate::from_template("Content: {page_content}").unwrap();

        let result = format_document(&doc, &prompt).unwrap();
        assert_eq!(result, "Content: Hello world");
    }

    #[test]
    fn test_format_document_with_metadata() {
        let mut doc = Document::new("Hello");
        doc.metadata
            .insert("source".to_string(), serde_json::json!("test.txt"));

        let prompt = PromptTemplate::from_template("Source: {source}\n{page_content}").unwrap();
        let result = format_document(&doc, &prompt).unwrap();

        assert_eq!(result, "Source: test.txt\nHello");
    }

    #[test]
    fn test_format_documents() {
        let docs = vec![Document::new("First doc"), Document::new("Second doc")];
        let prompt = PromptTemplate::from_template("{page_content}").unwrap();

        let result = format_documents(&docs, &prompt, "\n---\n").unwrap();
        assert_eq!(result, "First doc\n---\nSecond doc");
    }
}
