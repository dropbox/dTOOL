//! Semantic query languages and specialized data format loaders.
//!
//! This module provides loaders for query languages and specialized data formats:
//! - GraphQL (.graphql, .gql) - API query language
//! - `XQuery` (.xq, .xquery) - XML query language
//! - SPARQL (.sparql, .rq) - RDF/semantic web query language
//! - Cypher (.cypher, .cyp, .cql) - Graph database query language
//! - SGML (.sgml, .sgm, .dtd) - Standard Generalized Markup Language
//! - CoNLL-U (.conllu) - Linguistic annotation format
//! - WARC (.warc) - Web archive format
//! - ARFF (.arff) - Weka machine learning data format
//! - NFO (.nfo) - Info/release notes format

use async_trait::async_trait;
use std::path::{Path, PathBuf};

use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::Result;

/// Loader for `XQuery` files (.xq, .xquery).
///
/// `XQuery` is a query language for XML data, standardized by W3C in 2007.
/// Used for querying and transforming XML documents and databases.
/// Based on `XPath` with added programming language features.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::XQueryLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = XQueryLoader::new("query.xq");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct XQueryLoader {
    file_path: PathBuf,
    separate_functions: bool,
}

impl XQueryLoader {
    /// Create a new `XQuery` loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_functions: false,
        }
    }

    /// Enable separation by function declarations.
    #[must_use]
    pub fn with_separate_functions(mut self) -> Self {
        self.separate_functions = true;
        self
    }

    /// Detect `XQuery` function declaration, return function name
    fn detect_function_start(line: &str) -> Option<String> {
        let line = line.trim();

        // declare function local:name(...) or declare function fn:name(...)
        if line.contains("declare function") {
            if let Some(start) = line.find("function ") {
                let after_fn = &line[start + 9..].trim();
                // Extract function name (before the opening paren)
                if let Some(paren_pos) = after_fn.find('(') {
                    let name = after_fn[..paren_pos].trim().to_string();
                    return Some(name);
                }
            }
        }

        None
    }
}

#[async_trait]
impl DocumentLoader for XQueryLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_functions {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "xquery")]);
        }

        // Separate by function declarations
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut function_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect function declaration: declare function local:name(...) {
            if let Some(fn_name) = Self::detect_function_start(line) {
                let mut fn_lines = vec![lines[i]];
                i += 1;

                // Collect until closing brace
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;

                while i < lines.len() && brace_count > 0 {
                    let next_line = lines[i];
                    fn_lines.push(next_line);
                    brace_count += next_line.matches('{').count() as i32
                        - next_line.matches('}').count() as i32;
                    i += 1;
                }

                let fn_content = fn_lines.join("\n");
                documents.push(
                    Document::new(&fn_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "xquery")
                        .with_metadata("function_index", function_index.to_string())
                        .with_metadata("function_name", fn_name),
                );
                function_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "xquery")])
        } else {
            Ok(documents)
        }
    }
}

/// Loader for SPARQL query files (.sparql, .rq).
///
/// SPARQL is a query language for RDF (Resource Description Framework) data, standardized by W3C in 2008.
/// Used for semantic web and linked data queries.
/// Allows querying across distributed RDF graphs.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::SPARQLLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = SPARQLLoader::new("query.sparql");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SPARQLLoader {
    file_path: PathBuf,
    separate_queries: bool,
}

impl SPARQLLoader {
    /// Create a new SPARQL loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_queries: false,
        }
    }

    /// Enable separation by query types (SELECT, CONSTRUCT, ASK, DESCRIBE).
    #[must_use]
    pub fn with_separate_queries(mut self) -> Self {
        self.separate_queries = true;
        self
    }

    /// Detect SPARQL query start, return query type
    fn detect_query_start(line: &str) -> Option<String> {
        let line_upper = line.trim().to_uppercase();

        if line_upper.starts_with("SELECT") {
            return Some("SELECT".to_string());
        }
        if line_upper.starts_with("CONSTRUCT") {
            return Some("CONSTRUCT".to_string());
        }
        if line_upper.starts_with("ASK") {
            return Some("ASK".to_string());
        }
        if line_upper.starts_with("DESCRIBE") {
            return Some("DESCRIBE".to_string());
        }

        None
    }
}

#[async_trait]
impl DocumentLoader for SPARQLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_queries {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "sparql")]);
        }

        // Separate by SPARQL queries
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut query_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect SPARQL query start
            if let Some(query_type) = Self::detect_query_start(line) {
                let mut query_lines = vec![lines[i]];
                i += 1;

                // Collect until we find a line that ends the query (ends with } or semicolon)
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;

                while i < lines.len() {
                    let next_line = lines[i];
                    query_lines.push(next_line);
                    brace_count += next_line.matches('{').count() as i32
                        - next_line.matches('}').count() as i32;

                    // SPARQL queries end when braces are balanced
                    if brace_count == 0 && !next_line.trim().is_empty() {
                        i += 1;
                        break;
                    }
                    i += 1;
                }

                let query_content = query_lines.join("\n");
                documents.push(
                    Document::new(&query_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "sparql")
                        .with_metadata("query_index", query_index.to_string())
                        .with_metadata("query_type", query_type),
                );
                query_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "sparql")])
        } else {
            Ok(documents)
        }
    }
}

/// Loader for Cypher query files (.cypher, .cyp, .cql).
///
/// Cypher is a declarative graph query language created by Neo4j in 2011.
/// Open-sourced and standardized as openCypher.
/// Designed for querying and updating property graph databases.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::CypherLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = CypherLoader::new("query.cypher");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CypherLoader {
    file_path: PathBuf,
    separate_statements: bool,
}

impl CypherLoader {
    /// Create a new Cypher loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_statements: false,
        }
    }

    /// Enable separation by statements (MATCH, CREATE, MERGE, etc).
    #[must_use]
    pub fn with_separate_statements(mut self) -> Self {
        self.separate_statements = true;
        self
    }

    /// Detect Cypher statement type
    fn detect_statement_type(statement: &str) -> String {
        let stmt_upper = statement.trim().to_uppercase();

        if stmt_upper.starts_with("MATCH") {
            "MATCH".to_string()
        } else if stmt_upper.starts_with("CREATE") {
            "CREATE".to_string()
        } else if stmt_upper.starts_with("MERGE") {
            "MERGE".to_string()
        } else if stmt_upper.starts_with("DELETE") {
            "DELETE".to_string()
        } else if stmt_upper.starts_with("RETURN") {
            "RETURN".to_string()
        } else if stmt_upper.starts_with("WITH") {
            "WITH".to_string()
        } else if stmt_upper.starts_with("UNWIND") {
            "UNWIND".to_string()
        } else if stmt_upper.starts_with("CALL") {
            "CALL".to_string()
        } else {
            "UNKNOWN".to_string()
        }
    }
}

#[async_trait]
impl DocumentLoader for CypherLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_statements {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "cypher")]);
        }

        // Separate by Cypher statements (each ends with semicolon)
        let statements: Vec<&str> = content.split(';').collect();
        let mut documents = Vec::new();

        for (idx, statement) in statements.iter().enumerate() {
            let stmt = statement.trim();
            if stmt.is_empty() {
                continue;
            }

            let stmt_type = Self::detect_statement_type(stmt);
            documents.push(
                Document::new(stmt)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("format", "cypher")
                    .with_metadata("statement_index", idx.to_string())
                    .with_metadata("statement_type", stmt_type),
            );
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "cypher")])
        } else {
            Ok(documents)
        }
    }
}

/// Loader for SGML (Standard Generalized Markup Language) files.
///
/// SGML is a standard for defining markup languages. It is the ancestor of both HTML and XML.
///
/// # History and Context
///
/// - **Created:** 1986 (ISO 8879 standard)
/// - **Creator:** Charles Goldfarb (IBM)
/// - **Predecessors:** GML (Generalized Markup Language, 1969)
/// - **Purpose:** Meta-language for defining document markup languages
///
/// SGML influenced:
/// - HTML (1991) - Simplified SGML application
/// - XML (1998) - Simplified, stricter SGML subset
/// - `DocBook` - Technical documentation standard
/// - TEI (Text Encoding Initiative) - Humanities texts
///
/// # Historical Significance
///
/// SGML was revolutionary for:
/// - Separating content from presentation
/// - Defining document structure formally
/// - Enabling automated document processing
/// - Platform-independent document interchange
///
/// However, SGML's complexity led to XML's creation as a simpler alternative.
///
/// # Key Features
///
/// SGML supports:
/// - User-defined markup languages via DTDs
/// - Complex document structures
/// - Optional closing tags (unlike XML)
/// - Flexible syntax (more lenient than XML)
/// - Entity declarations
/// - Marked sections
///
/// # File Extensions
///
/// - `.sgml` - SGML document
/// - `.sgm` - SGML document (alternative)
/// - `.dtd` - Document Type Definition
///
/// # Usage
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::SGMLLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SGMLLoader::new("document.sgml");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
///
/// Note: This loader treats SGML as text. Full SGML parsing would require a
/// specialized parser that handles DTDs and optional closing tags.
#[derive(Debug, Clone)]
pub struct SGMLLoader {
    file_path: PathBuf,
}

impl SGMLLoader {
    /// Create a new SGML loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for SGMLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        Ok(vec![Document::new(&content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "sgml")])
    }
}

/// Loads CoNLL-U format files (linguistic annotations).
///
/// The `CoNLLULoader` reads CoNLL-U files containing annotated sentences.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::CoNLLULoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = CoNLLULoader::new("annotations.conllu");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct CoNLLULoader {
    /// Path to the CoNLL-U file
    pub file_path: PathBuf,
    /// Create separate documents per sentence (default: false, concatenate all)
    pub separate_sentences: bool,
}

impl CoNLLULoader {
    /// Create a new `CoNLLULoader` for the given file path.
    pub fn new(file_path: impl AsRef<Path>) -> Self {
        Self {
            file_path: file_path.as_ref().to_path_buf(),
            separate_sentences: false,
        }
    }

    /// Create separate documents per sentence.
    #[must_use]
    pub fn with_separate_sentences(mut self, separate: bool) -> Self {
        self.separate_sentences = separate;
        self
    }
}

#[async_trait]
impl DocumentLoader for CoNLLULoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();
        let mut all_content = String::new();
        let mut sentence_count = 0;

        // CoNLL-U format: sentences separated by blank lines
        // Comments start with #
        // Each line: ID FORM LEMMA UPOS XPOS FEATS HEAD DEPREL DEPS MISC
        let mut current_sentence = String::new();
        let mut current_words = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                // End of sentence
                if !current_sentence.is_empty() {
                    sentence_count += 1;

                    let sentence_text = current_words.join(" ");
                    let full_annotation = format!("Sentence: {sentence_text}\n{current_sentence}");

                    if self.separate_sentences {
                        let doc = Document::new(full_annotation)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("sentence_index", sentence_count - 1)
                            .with_metadata("sentence", sentence_text);

                        documents.push(doc);
                    } else {
                        all_content.push_str(&full_annotation);
                        all_content.push_str("\n\n");
                    }

                    current_sentence.clear();
                    current_words.clear();
                }
            } else if trimmed.starts_with('#') {
                // Comment line
                current_sentence.push_str(line);
                current_sentence.push('\n');
            } else {
                // Token line: ID FORM LEMMA ...
                let fields: Vec<&str> = trimmed.split('\t').collect();
                if fields.len() >= 2 {
                    // Extract the word form (second field)
                    let word = fields[1];
                    // Skip multi-word tokens (IDs like "1-2")
                    if !fields[0].contains('-') {
                        current_words.push(word);
                    }
                }
                current_sentence.push_str(line);
                current_sentence.push('\n');
            }
        }

        // Handle last sentence if file doesn't end with blank line
        if !current_sentence.is_empty() {
            sentence_count += 1;
            let sentence_text = current_words.join(" ");
            let full_annotation = format!("Sentence: {sentence_text}\n{current_sentence}");

            if self.separate_sentences {
                let doc = Document::new(full_annotation)
                    .with_metadata("source", self.file_path.display().to_string())
                    .with_metadata("sentence_index", sentence_count - 1)
                    .with_metadata("sentence", sentence_text);

                documents.push(doc);
            } else {
                all_content.push_str(&full_annotation);
            }
        }

        if !self.separate_sentences {
            let doc = Document::new(all_content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "conllu")
                .with_metadata("sentence_count", sentence_count);

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loader for WARC (Web `ARChive`) format files.
///
/// WARC is the standard format for web crawl archives, used by the Internet
/// Archive and Common Crawl. Parses WARC records and extracts content.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, WARCLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WARCLoader::new("archive.warc");
/// let docs = loader.load().await?;
/// println!("Loaded {} WARC records", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct WARCLoader {
    file_path: PathBuf,
}

impl WARCLoader {
    /// Create a new WARC loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for WARCLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse WARC format line by line
        // Records start with WARC/version, followed by headers, blank line, then content
        let mut documents = Vec::new();
        let lines: Vec<&str> = content.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            if lines[i].starts_with("WARC/") {
                // Start of a new WARC record
                let mut url = String::new();
                let mut date = String::new();
                i += 1; // Move past WARC/ line

                // Read headers
                while i < lines.len() && !lines[i].trim().is_empty() {
                    let line = lines[i];
                    if let Some(stripped) = line.strip_prefix("WARC-Target-URI:") {
                        url = stripped.trim().to_string();
                    } else if let Some(stripped) = line.strip_prefix("WARC-Date:") {
                        date = stripped.trim().to_string();
                    }
                    i += 1;
                }

                // Skip blank line
                if i < lines.len() && lines[i].trim().is_empty() {
                    i += 1;
                }

                // Read content until next WARC record or end
                let mut content_lines = Vec::new();
                while i < lines.len() && !lines[i].starts_with("WARC/") {
                    content_lines.push(lines[i]);
                    i += 1;
                }

                // Create document from content
                let content_text = content_lines.join("\n").trim().to_string();
                if !content_text.is_empty() {
                    documents.push(
                        Document::new(content_text)
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "warc")
                            .with_metadata("url", url)
                            .with_metadata("date", date),
                    );
                }
            } else {
                i += 1;
            }
        }

        Ok(documents)
    }
}

/// Loader for ARFF (Attribute-Relation File Format) data files.
///
/// ARFF is used by the Weka machine learning toolkit. Parses attribute
/// definitions and data instances, creating documents from data rows.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, ARFFLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = ARFFLoader::new("dataset.arff");
/// let docs = loader.load().await?;
/// println!("Loaded {} ARFF instances", docs.len());
/// # Ok(())
/// # }
/// ```
pub struct ARFFLoader {
    file_path: PathBuf,
}

impl ARFFLoader {
    /// Create a new ARFF loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for ARFFLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse ARFF format
        // @RELATION, @ATTRIBUTE, @DATA sections
        let mut attributes: Vec<String> = Vec::new();
        let mut in_data_section = false;
        let mut documents = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Skip comments and empty lines
            if trimmed.is_empty() || trimmed.starts_with('%') {
                continue;
            }

            if trimmed.to_uppercase().starts_with("@ATTRIBUTE") {
                // Extract attribute name
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    attributes.push(parts[1].to_string());
                }
            } else if trimmed.to_uppercase().starts_with("@DATA") {
                in_data_section = true;
            } else if in_data_section && !trimmed.starts_with('@') {
                // Data instance - parse CSV-like format
                let values: Vec<&str> = trimmed.split(',').map(str::trim).collect();

                // Create formatted text for this instance
                let mut instance_text = Vec::new();
                for (i, value) in values.iter().enumerate() {
                    let attr_name = attributes.get(i).map_or("?", std::string::String::as_str);
                    instance_text.push(format!("{attr_name}: {value}"));
                }

                if !instance_text.is_empty() {
                    documents.push(
                        Document::new(instance_text.join("\n"))
                            .with_metadata("source", self.file_path.display().to_string())
                            .with_metadata("format", "arff")
                            .with_metadata("type", "ml_data"),
                    );
                }
            }
        }

        Ok(documents)
    }
}

/// Loader for NFO (info) text files.
///
/// NFO files are plain text information files with ASCII art, commonly used
/// for software releases, documentation, and README files. Preserves ASCII
/// art and formatting.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, NFOLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = NFOLoader::new("release.nfo");
/// let docs = loader.load().await?;
/// println!("Loaded NFO file");
/// # Ok(())
/// # }
/// ```
pub struct NFOLoader {
    file_path: PathBuf,
}

impl NFOLoader {
    /// Create a new NFO loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for NFOLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        // NFO files are often encoded in CP437 (DOS encoding), but we'll read as UTF-8
        // and handle decoding errors gracefully
        let content = blob.as_string()?;

        // NFO files are plain text with ASCII art - preserve formatting
        // Just load as-is, preserving all whitespace and special characters
        Ok(vec![Document::new(content)
            .with_metadata("source", self.file_path.display().to_string())
            .with_metadata("format", "nfo")
            .with_metadata("type", "info")])
    }
}

/// Loader for GraphQL query language files (.graphql, .gql).
///
/// GraphQL is a query language for APIs and a runtime for executing queries, created by Facebook in 2012.
/// Released as open-source in 2015, widely adopted for API development.
/// Allows clients to request exactly the data they need.
///
/// # Example
///
/// ```rust,no_run
/// # use dashflow::core::document_loaders::GraphQLLoader;
/// # use dashflow::core::documents::DocumentLoader;
/// # async fn example() -> dashflow::core::error::Result<()> {
/// let loader = GraphQLLoader::new("schema.graphql");
/// let docs = loader.load().await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GraphQLLoader {
    file_path: PathBuf,
    separate_operations: bool,
}

impl GraphQLLoader {
    /// Create a new GraphQL loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            separate_operations: false,
        }
    }

    /// Enable separation by operations (query, mutation, subscription, type, interface, etc).
    #[must_use]
    pub fn with_separate_operations(mut self) -> Self {
        self.separate_operations = true;
        self
    }
}

#[async_trait]
impl DocumentLoader for GraphQLLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        if !self.separate_operations {
            return Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "graphql")]);
        }

        // Separate by GraphQL operations and type definitions
        let lines: Vec<&str> = content.lines().collect();
        let mut documents = Vec::new();
        let mut i = 0;
        let mut operation_index = 0;

        while i < lines.len() {
            let line = lines[i].trim();

            // Detect operation/type start
            if let Some(op_info) = Self::detect_operation_start(line) {
                let (op_type, op_name) = op_info;
                let mut op_lines = vec![lines[i]];
                i += 1;

                // Collect until closing brace
                let mut brace_count =
                    line.matches('{').count() as i32 - line.matches('}').count() as i32;

                while i < lines.len() && brace_count > 0 {
                    let next_line = lines[i];
                    op_lines.push(next_line);
                    brace_count += next_line.matches('{').count() as i32
                        - next_line.matches('}').count() as i32;
                    i += 1;
                }

                let op_content = op_lines.join("\n");
                documents.push(
                    Document::new(&op_content)
                        .with_metadata("source", self.file_path.display().to_string())
                        .with_metadata("format", "graphql")
                        .with_metadata("operation_index", operation_index.to_string())
                        .with_metadata("operation_type", op_type)
                        .with_metadata("operation_name", op_name),
                );
                operation_index += 1;
            } else {
                i += 1;
            }
        }

        if documents.is_empty() {
            Ok(vec![Document::new(&content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "graphql")])
        } else {
            Ok(documents)
        }
    }
}

impl GraphQLLoader {
    /// Detect GraphQL operation or type definition start, return (type, name)
    fn detect_operation_start(line: &str) -> Option<(String, String)> {
        let line = line.trim();

        // query GetUser {
        if line.starts_with("query ") {
            let after_query = line.strip_prefix("query ")?.trim();
            let name = after_query
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("query".to_string(), name));
        }

        // mutation CreateUser {
        if line.starts_with("mutation ") {
            let after_mutation = line.strip_prefix("mutation ")?.trim();
            let name = after_mutation
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("mutation".to_string(), name));
        }

        // subscription OnMessageReceived {
        if line.starts_with("subscription ") {
            let after_sub = line.strip_prefix("subscription ")?.trim();
            let name = after_sub
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("subscription".to_string(), name));
        }

        // type User {
        if line.starts_with("type ") {
            let after_type = line.strip_prefix("type ")?.trim();
            let name = after_type
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("type".to_string(), name));
        }

        // interface Node {
        if line.starts_with("interface ") {
            let after_interface = line.strip_prefix("interface ")?.trim();
            let name = after_interface
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("interface".to_string(), name));
        }

        // enum Status {
        if line.starts_with("enum ") {
            let after_enum = line.strip_prefix("enum ")?.trim();
            let name = after_enum
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("enum".to_string(), name));
        }

        // input CreateUserInput {
        if line.starts_with("input ") {
            let after_input = line.strip_prefix("input ")?.trim();
            let name = after_input
                .split_whitespace()
                .next()?
                .trim_end_matches('{')
                .trim()
                .to_string();
            return Some(("input".to_string(), name));
        }

        // union SearchResult = User | Post
        if line.starts_with("union ") {
            let after_union = line.strip_prefix("union ")?.trim();
            let name = after_union
                .split_whitespace()
                .next()?
                .trim_end_matches('=')
                .trim()
                .to_string();
            return Some(("union".to_string(), name));
        }

        // scalar DateTime
        if line.starts_with("scalar ") {
            let after_scalar = line.strip_prefix("scalar ")?.trim();
            let name = after_scalar.split_whitespace().next()?.to_string();
            return Some(("scalar".to_string(), name));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_graphql_loader() {
        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.graphql");

        let schema_content = r#"type User {
  id: ID!
  name: String!
  email: String!
}"#;

        fs::write(&schema_path, schema_content).unwrap();

        let loader = GraphQLLoader::new(&schema_path);
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].page_content.contains("type User"));
        assert_eq!(
            docs[0].get_metadata("format").and_then(|v| v.as_str()),
            Some("graphql")
        );
    }

    #[tokio::test]
    async fn test_graphql_loader_separate_operations() {
        let temp_dir = TempDir::new().unwrap();
        let schema_path = temp_dir.path().join("schema.graphql");

        let schema_content = r#"type User {
  id: ID!
  name: String!
}

query GetUser {
  user(id: "123") {
    name
  }
}

mutation CreateUser {
  createUser(input: {name: "Alice"}) {
    id
  }
}

enum Status {
  ACTIVE
  INACTIVE
}"#;

        fs::write(&schema_path, schema_content).unwrap();

        let loader = GraphQLLoader::new(&schema_path).with_separate_operations();
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 4);
        assert!(docs[0].page_content.contains("type User"));
        assert!(docs[1].page_content.contains("query GetUser"));
        assert!(docs[2].page_content.contains("mutation CreateUser"));
        assert!(docs[3].page_content.contains("enum Status"));
        assert_eq!(
            docs[0]
                .get_metadata("operation_type")
                .and_then(|v| v.as_str()),
            Some("type")
        );
        assert_eq!(
            docs[0]
                .get_metadata("operation_name")
                .and_then(|v| v.as_str()),
            Some("User")
        );
        assert_eq!(
            docs[1]
                .get_metadata("operation_type")
                .and_then(|v| v.as_str()),
            Some("query")
        );
        assert_eq!(
            docs[1]
                .get_metadata("operation_name")
                .and_then(|v| v.as_str()),
            Some("GetUser")
        );
        assert_eq!(
            docs[2]
                .get_metadata("operation_type")
                .and_then(|v| v.as_str()),
            Some("mutation")
        );
        assert_eq!(
            docs[2]
                .get_metadata("operation_name")
                .and_then(|v| v.as_str()),
            Some("CreateUser")
        );
    }
}
