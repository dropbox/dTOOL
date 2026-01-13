// Allow clippy warnings for character splitters
// - needless_pass_by_value: Split options passed by value for API consistency
#![allow(clippy::needless_pass_by_value)]

//! Character-based text splitters

use crate::error::{Error, Result};
use crate::language::Language;
use crate::split_utils::{split_text_with_compiled_regex, split_text_with_regex};
use crate::traits::{KeepSeparator, TextSplitter};
use regex::{Regex, RegexBuilder};
use std::cell::RefCell;
use std::collections::VecDeque;

/// Maximum size in bytes for compiled regex patterns (256KB).
/// This prevents resource exhaustion from pathologically complex patterns.
const REGEX_SIZE_LIMIT: usize = 256 * 1024;

/// Maximum size in bytes for the DFA cache (256KB).
const REGEX_DFA_SIZE_LIMIT: usize = 256 * 1024;

/// Compile a regex pattern with size limits to prevent resource exhaustion.
fn compile_bounded_regex(pattern: &str) -> std::result::Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
}

// SAFETY (M-194): This thread_local! RefCell pattern is safe in async contexts because:
// 1. thread_local! provides per-thread isolation - each thread gets its own RefCell
// 2. Borrows are scoped within synchronous `.with()` closures, never held across await points
// 3. No borrow can outlive the closure, preventing RefCell borrow conflicts
// 4. We store Vec<usize> indices instead of references to avoid lifetime issues
thread_local! {
    static MERGE_BUFFER: RefCell<VecDeque<usize>> = RefCell::new(VecDeque::with_capacity(100));
}

/// Configuration for text splitters
#[derive(Debug, Clone)]
pub struct TextSplitterConfig {
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub length_function: fn(&str) -> usize,
    pub keep_separator: KeepSeparator,
    pub add_start_index: bool,
    pub strip_whitespace: bool,
}

impl Default for TextSplitterConfig {
    fn default() -> Self {
        Self {
            chunk_size: 4000,
            chunk_overlap: 200,
            length_function: |s: &str| s.len(),
            keep_separator: KeepSeparator::False,
            add_start_index: false,
            strip_whitespace: true,
        }
    }
}

impl TextSplitterConfig {
    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.chunk_size == 0 {
            return Err(Error::InvalidConfiguration(format!(
                "chunk_size must be > 0, got {}",
                self.chunk_size
            )));
        }
        if self.chunk_overlap > self.chunk_size {
            return Err(Error::InvalidConfiguration(format!(
                "Got a larger chunk overlap ({}) than chunk size ({}), should be smaller.",
                self.chunk_overlap, self.chunk_size
            )));
        }
        Ok(())
    }

    /// Merge splits into chunks that respect `chunk_size` and `chunk_overlap`
    pub fn merge_splits(&self, splits: &[String], separator: &str) -> Vec<String> {
        let separator_len = (self.length_function)(separator);
        let mut docs = Vec::new();

        // Use thread-local VecDeque to avoid allocation overhead
        // We store indices into the splits array instead of references
        MERGE_BUFFER.with(|buffer| {
            let mut current_doc = buffer.borrow_mut();
            current_doc.clear(); // Reuse existing capacity
            let mut total = 0;

            for (idx, split) in splits.iter().enumerate() {
                let len = (self.length_function)(split);
                let separator_adjustment = if current_doc.is_empty() {
                    0
                } else {
                    separator_len
                };

                // Check if adding this split would exceed chunk_size
                if total + len + separator_adjustment > self.chunk_size {
                    // Warn if we've already exceeded chunk_size
                    if total > self.chunk_size {
                        tracing::warn!(
                            chunk_size = total,
                            max_chunk_size = self.chunk_size,
                            "Created a chunk larger than the specified chunk_size"
                        );
                    }

                    // Save current chunk if it's not empty
                    if !current_doc.is_empty() {
                        // Join the strings referenced by indices
                        let doc = if self.strip_whitespace {
                            let joined: String = current_doc
                                .iter()
                                .map(|&i| splits[i].as_str())
                                .collect::<Vec<_>>()
                                .join(separator);
                            joined.trim().to_string()
                        } else {
                            current_doc
                                .iter()
                                .map(|&i| splits[i].as_str())
                                .collect::<Vec<_>>()
                                .join(separator)
                        };
                        if !doc.is_empty() {
                            docs.push(doc);
                        }

                        // Keep popping from current_doc to maintain overlap constraint (O(1) with VecDeque)
                        // We need to keep removing elements while:
                        // 1. We have more than the allowed overlap (total > chunk_overlap)
                        // 2. OR adding the new split would still exceed chunk_size AND we have content
                        while total > self.chunk_overlap
                            || (total
                                + len
                                + if current_doc.is_empty() {
                                    0
                                } else {
                                    separator_len
                                }
                                > self.chunk_size
                                && total > 0)
                        {
                            if let Some(removed_idx) = current_doc.pop_front() {
                                let removed_len = (self.length_function)(&splits[removed_idx]);
                                let sep_adjustment = if current_doc.is_empty() {
                                    0
                                } else {
                                    separator_len
                                };
                                total -= removed_len + sep_adjustment;
                            } else {
                                break;
                            }
                        }
                    }
                }

                // Add the current split (store index instead of reference)
                current_doc.push_back(idx);
                let sep_adjustment = if current_doc.len() > 1 {
                    separator_len
                } else {
                    0
                };
                total += len + sep_adjustment;
            }

            // Add the final chunk
            if !current_doc.is_empty() {
                let doc = if self.strip_whitespace {
                    let joined: String = current_doc
                        .iter()
                        .map(|&i| splits[i].as_str())
                        .collect::<Vec<_>>()
                        .join(separator);
                    joined.trim().to_string()
                } else {
                    current_doc
                        .iter()
                        .map(|&i| splits[i].as_str())
                        .collect::<Vec<_>>()
                        .join(separator)
                };
                if !doc.is_empty() {
                    docs.push(doc);
                }
            }
        });

        docs
    }
}

/// A text splitter that splits on a single character separator.
///
/// # Example
///
/// ```
/// use dashflow_text_splitters::{CharacterTextSplitter, TextSplitter};
///
/// let splitter = CharacterTextSplitter::new()
///     .with_chunk_size(20)
///     .with_chunk_overlap(0)
///     .with_separator("\n\n");
///
/// let text = "Paragraph 1.\n\nParagraph 2.\n\nParagraph 3.";
/// let chunks = splitter.split_text(text);
/// assert_eq!(chunks.len(), 3);
/// ```
#[derive(Debug)]
pub struct CharacterTextSplitter {
    config: TextSplitterConfig,
    separator: String,
    is_separator_regex: bool,
    // Cached compiled regex for performance
    regex: Option<Regex>,
}

impl CharacterTextSplitter {
    /// Create a new `CharacterTextSplitter` with default settings
    #[must_use]
    pub fn new() -> Self {
        let separator = "\n\n".to_string();
        let sep_pattern = regex::escape(&separator);
        let regex = compile_bounded_regex(&sep_pattern).ok();

        Self {
            config: TextSplitterConfig::default(),
            separator,
            is_separator_regex: false,
            regex,
        }
    }

    /// Set the separator to split on
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self.regex = None; // Invalidate cache
        self
    }

    /// Set whether the separator is a regex pattern
    #[must_use]
    pub fn with_separator_regex(mut self, is_regex: bool) -> Self {
        self.is_separator_regex = is_regex;
        self.regex = None; // Invalidate cache
        self
    }

    /// Set the chunk size
    #[must_use]
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.config.chunk_size = size;
        self
    }

    /// Set the chunk overlap
    #[must_use]
    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        self.config.chunk_overlap = overlap;
        self
    }

    /// Set whether to keep the separator and where
    #[must_use]
    pub fn with_keep_separator(mut self, keep: KeepSeparator) -> Self {
        self.config.keep_separator = keep;
        self
    }

    /// Set whether to add `start_index` to metadata
    #[must_use]
    pub fn with_add_start_index(mut self, add: bool) -> Self {
        self.config.add_start_index = add;
        self
    }

    /// Set whether to strip whitespace
    #[must_use]
    pub fn with_strip_whitespace(mut self, strip: bool) -> Self {
        self.config.strip_whitespace = strip;
        self
    }

    /// Build the splitter, validating configuration and compiling regex
    pub fn build(mut self) -> Result<Self> {
        self.config.validate()?;

        // Compile and cache regex if not already done
        if self.regex.is_none() {
            let sep_pattern = if self.is_separator_regex {
                self.separator.clone()
            } else {
                regex::escape(&self.separator)
            };

            self.regex = match compile_bounded_regex(&sep_pattern) {
                Ok(r) => Some(r),
                Err(e) => {
                    return Err(Error::InvalidConfiguration(format!(
                        "Invalid regex pattern: {e}"
                    )))
                }
            };
        }

        Ok(self)
    }
}

impl Default for CharacterTextSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSplitter for CharacterTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        // Use cached regex if available, otherwise compile on-the-fly
        let splits = if let Some(ref regex) = self.regex {
            // Use pre-compiled regex for optimal performance
            split_text_with_compiled_regex(text, regex, self.config.keep_separator)
        } else {
            // Fallback: compile regex on-the-fly (slower path)
            let sep_pattern = if self.is_separator_regex {
                self.separator.clone()
            } else {
                regex::escape(&self.separator)
            };
            split_text_with_regex(text, &sep_pattern, self.config.keep_separator)
        };

        // Check if this is a zero-width lookaround pattern
        let lookaround_prefixes = ["(?=", "(?<!", "(?<=", "(?!"];
        let is_lookaround = self.is_separator_regex
            && lookaround_prefixes
                .iter()
                .any(|prefix| self.separator.starts_with(prefix));

        // Decide merge separator
        let merge_sep = if self.config.keep_separator != KeepSeparator::False || is_lookaround {
            ""
        } else {
            &self.separator
        };

        // Merge splits
        self.config.merge_splits(&splits, merge_sep)
    }

    fn chunk_size(&self) -> usize {
        self.config.chunk_size
    }

    fn chunk_overlap(&self) -> usize {
        self.config.chunk_overlap
    }

    fn add_start_index(&self) -> bool {
        self.config.add_start_index
    }
}

/// A text splitter that recursively tries different separators.
///
/// This splitter tries to split text using multiple separators in order,
/// recursively splitting chunks that are too large using subsequent separators.
///
/// # Example
///
/// ```
/// use dashflow_text_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
///
/// let splitter = RecursiveCharacterTextSplitter::new()
///     .with_chunk_size(100)
///     .with_chunk_overlap(20);
///
/// let text = "This is a long text.\n\nIt has multiple paragraphs.\n\nAnd sentences.";
/// let chunks = splitter.split_text(text);
/// ```
#[derive(Debug, Clone)]
pub struct RecursiveCharacterTextSplitter {
    config: TextSplitterConfig,
    separators: Vec<String>,
    is_separator_regex: bool,
    // Cached compiled regexes for performance (one per separator)
    compiled_regexes: Vec<Option<Regex>>,
}

impl RecursiveCharacterTextSplitter {
    /// Create a new `RecursiveCharacterTextSplitter` with default separators
    #[must_use]
    pub fn new() -> Self {
        let separators = vec![
            "\n\n".to_string(),
            "\n".to_string(),
            " ".to_string(),
            String::new(),
        ];
        let is_separator_regex = false;

        // Pre-compile regexes for all separators for optimal performance
        let compiled_regexes = Self::compile_separator_regexes(&separators, is_separator_regex);

        Self {
            config: TextSplitterConfig {
                keep_separator: KeepSeparator::Start,
                ..Default::default()
            },
            separators,
            is_separator_regex,
            compiled_regexes,
        }
    }

    /// Create a new `RecursiveCharacterTextSplitter` configured for a specific programming language
    ///
    /// This uses language-specific separators that split along natural code boundaries
    /// like function definitions, class definitions, and control flow statements.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_text_splitters::{RecursiveCharacterTextSplitter, Language, TextSplitter};
    ///
    /// let python_code = r#"
    /// class MyClass:
    ///     def method1(self):
    ///         pass
    ///
    ///     def method2(self):
    ///         pass
    /// "#;
    ///
    /// let splitter = RecursiveCharacterTextSplitter::from_language(Language::Python)
    ///     .with_chunk_size(100)
    ///     .with_chunk_overlap(0);
    ///
    /// let chunks = splitter.split_text(python_code);
    /// ```
    #[must_use]
    pub fn from_language(language: Language) -> Self {
        let separators = language.get_separators();
        let is_separator_regex = false;

        // Pre-compile regexes for all separators for optimal performance
        let compiled_regexes = Self::compile_separator_regexes(&separators, is_separator_regex);

        Self {
            config: TextSplitterConfig {
                keep_separator: KeepSeparator::Start,
                ..Default::default()
            },
            separators,
            is_separator_regex,
            compiled_regexes,
        }
    }

    /// Set custom separators
    #[must_use]
    pub fn with_separators(mut self, separators: Vec<String>) -> Self {
        self.separators = separators.clone();
        // Recompile regexes when separators change
        self.compiled_regexes =
            Self::compile_separator_regexes(&separators, self.is_separator_regex);
        self
    }

    /// Set whether separators are regex patterns
    #[must_use]
    pub fn with_separator_regex(mut self, is_regex: bool) -> Self {
        self.is_separator_regex = is_regex;
        // Recompile regexes when regex mode changes
        self.compiled_regexes = Self::compile_separator_regexes(&self.separators, is_regex);
        self
    }

    /// Set the chunk size
    #[must_use]
    pub fn with_chunk_size(mut self, size: usize) -> Self {
        self.config.chunk_size = size;
        self
    }

    /// Set the chunk overlap
    #[must_use]
    pub fn with_chunk_overlap(mut self, overlap: usize) -> Self {
        self.config.chunk_overlap = overlap;
        self
    }

    /// Set whether to keep the separator and where
    #[must_use]
    pub fn with_keep_separator(mut self, keep: KeepSeparator) -> Self {
        self.config.keep_separator = keep;
        self
    }

    /// Build the splitter, validating configuration
    pub fn build(self) -> Result<Self> {
        self.config.validate()?;
        Ok(self)
    }

    /// Helper method to pre-compile all separator regexes for optimal performance
    fn compile_separator_regexes(
        separators: &[String],
        is_separator_regex: bool,
    ) -> Vec<Option<Regex>> {
        separators
            .iter()
            .map(|sep| {
                if sep.is_empty() {
                    // Empty separator means character-level split - no regex needed
                    None
                } else {
                    let sep_pattern = if is_separator_regex {
                        sep.as_str()
                    } else {
                        &regex::escape(sep)
                    };
                    compile_bounded_regex(sep_pattern).ok()
                }
            })
            .collect()
    }

    /// Recursively split text using the separators
    fn split_text_recursive(&self, text: &str, separators: &[String]) -> Vec<String> {
        // Calculate the starting index in the full separator list
        let start_idx = self.separators.len() - separators.len();
        self.split_text_recursive_indexed(text, separators, start_idx)
    }

    /// Internal recursive split method that tracks separator indices for cached regex lookup
    fn split_text_recursive_indexed(
        &self,
        text: &str,
        separators: &[String],
        start_idx: usize,
    ) -> Vec<String> {
        let mut final_chunks = Vec::new();

        // Find the first separator that matches in the text
        // Use pre-compiled regexes from cache for optimal performance
        let mut sep_index = None;

        for (i, sep) in separators.iter().enumerate() {
            if sep.is_empty() {
                // Empty separator means split into characters - use it immediately
                sep_index = Some(i);
                break;
            }

            let global_idx = start_idx + i;
            // Use cached compiled regex (zero-cost lookup)
            if let Some(Some(ref re)) = self.compiled_regexes.get(global_idx) {
                if re.is_match(text) {
                    sep_index = Some(i);
                    break;
                }
            }
        }

        // If no separator found, use the last one
        let sep_idx = sep_index.unwrap_or_else(|| separators.len().saturating_sub(1));

        // Get the separator to use (must handle case where separators is empty)
        let empty_string = String::new();
        let sep_for_split = separators.get(sep_idx).unwrap_or(&empty_string);

        // Get remaining separators for recursive calls
        let new_separators = if sep_idx + 1 < separators.len() {
            &separators[sep_idx + 1..]
        } else {
            &[]
        };

        // Use cached compiled regex for splitting
        let global_idx = start_idx + sep_idx;
        let splits = if let Some(Some(ref re)) = self.compiled_regexes.get(global_idx) {
            // Use pre-compiled regex from cache (optimal path)
            match self.config.keep_separator {
                KeepSeparator::False => re
                    .split(text)
                    .filter(|s| !s.is_empty())
                    .map(std::string::ToString::to_string)
                    .collect(),
                _ => {
                    // For other keep modes, use the utility function with cached regex
                    split_text_with_compiled_regex(text, re, self.config.keep_separator)
                }
            }
        } else {
            // Fallback for empty separator or compilation failure
            let sep_pattern = if self.is_separator_regex {
                sep_for_split.as_str()
            } else {
                &regex::escape(sep_for_split)
            };
            split_text_with_regex(text, sep_pattern, self.config.keep_separator)
        };

        // Merge splits, recursively splitting larger texts
        let mut good_splits = Vec::new();
        let separator_for_merge = if self.config.keep_separator == KeepSeparator::False {
            sep_for_split.as_str()
        } else {
            ""
        };

        for split in splits {
            let split_len = (self.config.length_function)(&split);
            if split_len < self.config.chunk_size {
                good_splits.push(split);
            } else {
                // Current split is too large, need to handle it
                if !good_splits.is_empty() {
                    let merged = self.config.merge_splits(&good_splits, separator_for_merge);
                    final_chunks.extend(merged);
                    good_splits.clear();
                }

                // Recursively split if we have more separators
                if new_separators.is_empty() {
                    final_chunks.push(split);
                } else {
                    let new_start_idx = start_idx + sep_idx + 1;
                    let other_chunks =
                        self.split_text_recursive_indexed(&split, new_separators, new_start_idx);
                    final_chunks.extend(other_chunks);
                }
            }
        }

        // Don't forget remaining good splits
        if !good_splits.is_empty() {
            let merged = self.config.merge_splits(&good_splits, separator_for_merge);
            final_chunks.extend(merged);
        }

        final_chunks
    }
}

impl Default for RecursiveCharacterTextSplitter {
    fn default() -> Self {
        Self::new()
    }
}

impl TextSplitter for RecursiveCharacterTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        self.split_text_recursive(text, &self.separators)
    }

    fn chunk_size(&self) -> usize {
        self.config.chunk_size
    }

    fn chunk_overlap(&self) -> usize {
        self.config.chunk_overlap
    }

    fn add_start_index(&self) -> bool {
        self.config.add_start_index
    }
}

#[cfg(test)]
#[path = "character_tests.rs"]
mod character_tests;
