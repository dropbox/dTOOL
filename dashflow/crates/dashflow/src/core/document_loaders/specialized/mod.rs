//! Domain-specific and specialized format loaders.
//!
//! This module provides loaders for specialized formats:
//! - Academic formats (BibTeX, LaTeX, Texinfo)
//! - Semantic/query languages (`XQuery`, SPARQL, Cypher, SGML)
//! - Specialized data formats (`CoNLLU`, WARC, ARFF, NFO)

pub mod academic;
pub mod semantic;

pub use academic::*;
pub use semantic::*;
