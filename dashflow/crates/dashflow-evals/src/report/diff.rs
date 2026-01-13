//! Diff generation for comparing expected vs actual outputs.
//!
//! Provides multiple diff formats:
//! - HTML with syntax highlighting and side-by-side view
//! - Plain text unified diff (like `git diff`)
//! - Character-level diff showing exact changes
//! - Semantic similarity scoring

use anyhow::Result;
use similar::{ChangeTag, TextDiff};
use std::fmt::Write as FmtWrite;

/// Diff generator for comparing expected vs actual outputs
pub struct DiffGenerator;

impl DiffGenerator {
    /// Generate HTML diff with side-by-side view
    ///
    /// Creates a beautiful HTML diff visualization with:
    /// - Side-by-side comparison
    /// - Syntax highlighting for additions/deletions
    /// - Line numbers
    /// - Character-level highlighting within lines
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_evals::report::diff::DiffGenerator;
    ///
    /// let expected = "The quick brown fox";
    /// let actual = "The quick red fox";
    /// let html = DiffGenerator::generate_html_diff(expected, actual).unwrap();
    /// assert!(html.contains("brown"));
    /// assert!(html.contains("red"));
    /// ```
    pub fn generate_html_diff(expected: &str, actual: &str) -> Result<String> {
        let mut html = String::new();

        // CSS styling
        writeln!(
            html,
            r"<style>
.diff-container {{
    display: flex;
    font-family: 'Courier New', monospace;
    font-size: 13px;
    border: 1px solid #ddd;
    border-radius: 4px;
    overflow: hidden;
}}
.diff-pane {{
    flex: 1;
    padding: 10px;
    overflow-x: auto;
}}
.diff-pane.expected {{
    background: #fef2f2;
    border-right: 2px solid #ddd;
}}
.diff-pane.actual {{
    background: #f0fdf4;
}}
.diff-line {{
    padding: 2px 4px;
    white-space: pre-wrap;
    word-break: break-all;
}}
.diff-line-number {{
    display: inline-block;
    width: 40px;
    color: #999;
    user-select: none;
    text-align: right;
    padding-right: 10px;
}}
.diff-removed {{
    background: #fee;
    color: #c00;
    text-decoration: line-through;
}}
.diff-added {{
    background: #dfd;
    color: #080;
    font-weight: bold;
}}
.diff-header {{
    background: #667eea;
    color: white;
    padding: 10px;
    font-weight: bold;
    text-align: center;
}}
</style>"
        )?;

        writeln!(html, r#"<div class="diff-container">"#)?;

        // Expected pane
        writeln!(html, r#"<div class="diff-pane expected">"#)?;
        writeln!(html, r#"<div class="diff-header">Expected</div>"#)?;
        for (i, line) in expected.lines().enumerate() {
            writeln!(
                html,
                r#"<div class="diff-line"><span class="diff-line-number">{}</span>{}</div>"#,
                i + 1,
                html_escape(line)
            )?;
        }
        writeln!(html, r"</div>")?;

        // Actual pane
        writeln!(html, r#"<div class="diff-pane actual">"#)?;
        writeln!(html, r#"<div class="diff-header">Actual</div>"#)?;
        for (i, line) in actual.lines().enumerate() {
            writeln!(
                html,
                r#"<div class="diff-line"><span class="diff-line-number">{}</span>{}</div>"#,
                i + 1,
                html_escape(line)
            )?;
        }
        writeln!(html, r"</div>")?;

        writeln!(html, r"</div>")?;

        // Character-level diff below
        writeln!(
            html,
            r#"<div style="margin-top: 20px; padding: 10px; background: #f8f9fa; border-radius: 4px;">"#
        )?;
        writeln!(
            html,
            r#"<h4 style="margin-top: 0;">Character-level Diff</h4>"#
        )?;
        writeln!(
            html,
            r#"<div style="font-family: monospace; white-space: pre-wrap;">"#
        )?;

        let diff = TextDiff::from_words(expected, actual);
        for change in diff.iter_all_changes() {
            let (class, text) = match change.tag() {
                ChangeTag::Delete => ("diff-removed", change.value()),
                ChangeTag::Insert => ("diff-added", change.value()),
                ChangeTag::Equal => ("", change.value()),
            };

            if class.is_empty() {
                write!(html, "{}", html_escape(text))?;
            } else {
                write!(
                    html,
                    r#"<span class="{}">{}</span>"#,
                    class,
                    html_escape(text)
                )?;
            }
        }

        writeln!(html, r"</div>")?;
        writeln!(html, r"</div>")?;

        Ok(html)
    }

    /// Generate unified diff format (like `git diff`)
    ///
    /// Creates a text-based unified diff that can be used in command-line tools.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_evals::report::diff::DiffGenerator;
    ///
    /// let expected = "line1\nline2\nline3";
    /// let actual = "line1\nmodified\nline3";
    /// let diff = DiffGenerator::generate_unified_diff(expected, actual, "expected", "actual").unwrap();
    /// assert!(diff.contains("---"));
    /// assert!(diff.contains("+++"));
    /// ```
    pub fn generate_unified_diff(
        expected: &str,
        actual: &str,
        expected_label: &str,
        actual_label: &str,
    ) -> Result<String> {
        let diff = TextDiff::from_lines(expected, actual);

        let mut output = String::new();
        writeln!(output, "--- {expected_label}")?;
        writeln!(output, "+++ {actual_label}")?;

        for hunk in diff.unified_diff().iter_hunks() {
            writeln!(output, "{hunk}")?;
        }

        Ok(output)
    }

    /// Generate inline diff showing changes within text
    ///
    /// Produces a single view with changes highlighted inline.
    pub fn generate_inline_diff(expected: &str, actual: &str) -> Result<String> {
        let mut output = String::new();

        let diff = TextDiff::from_words(expected, actual);

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => write!(output, "[-{}]", change.value())?,
                ChangeTag::Insert => write!(output, "[+{}]", change.value())?,
                ChangeTag::Equal => write!(output, "{}", change.value())?,
            }
        }

        Ok(output)
    }

    /// Calculate similarity ratio between two strings (0.0-1.0)
    ///
    /// Uses the similar crate's ratio algorithm to compute how similar two strings are.
    /// Returns 1.0 for identical strings, 0.0 for completely different.
    ///
    /// # Example
    ///
    /// ```
    /// use dashflow_evals::report::diff::DiffGenerator;
    ///
    /// let ratio = DiffGenerator::similarity_ratio("hello world", "hello world");
    /// assert_eq!(ratio, 1.0);
    ///
    /// let ratio = DiffGenerator::similarity_ratio("hello world", "hello rust");
    /// assert!(ratio > 0.5 && ratio < 1.0);
    /// ```
    #[must_use]
    pub fn similarity_ratio(expected: &str, actual: &str) -> f64 {
        f64::from(TextDiff::from_words(expected, actual).ratio())
    }

    /// Generate diff statistics
    ///
    /// Returns metrics about the diff: additions, deletions, unchanged.
    #[must_use]
    pub fn diff_stats(expected: &str, actual: &str) -> DiffStats {
        let diff = TextDiff::from_words(expected, actual);

        let mut additions = 0;
        let mut deletions = 0;
        let mut unchanged = 0;

        for change in diff.iter_all_changes() {
            match change.tag() {
                ChangeTag::Delete => deletions += 1,
                ChangeTag::Insert => additions += 1,
                ChangeTag::Equal => unchanged += 1,
            }
        }

        DiffStats {
            additions,
            deletions,
            unchanged,
            similarity: f64::from(diff.ratio()),
        }
    }

    /// Generate markdown diff for GitHub comments
    ///
    /// Creates a markdown-formatted diff suitable for GitHub PR comments.
    pub fn generate_markdown_diff(expected: &str, actual: &str) -> Result<String> {
        let mut md = String::new();

        writeln!(md, "#### Diff Analysis")?;
        writeln!(md)?;

        let stats = Self::diff_stats(expected, actual);
        writeln!(md, "**Similarity:** {:.1}%", stats.similarity * 100.0)?;
        writeln!(
            md,
            "**Changes:** {} additions, {} deletions",
            stats.additions, stats.deletions
        )?;
        writeln!(md)?;

        writeln!(md, "```diff")?;
        write!(
            md,
            "{}",
            Self::generate_unified_diff(expected, actual, "expected", "actual")?
        )?;
        writeln!(md, "```")?;

        Ok(md)
    }
}

/// Statistics about a diff
#[derive(Debug, Clone)]
pub struct DiffStats {
    /// Number of additions
    pub additions: usize,

    /// Number of deletions
    pub deletions: usize,

    /// Number of unchanged parts
    pub unchanged: usize,

    /// Overall similarity (0.0-1.0)
    pub similarity: f64,
}

/// HTML escape helper
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_diff_generation() {
        let expected = "The quick brown fox jumps over the lazy dog";
        let actual = "The quick red fox jumps over the lazy cat";

        let html = DiffGenerator::generate_html_diff(expected, actual).unwrap();

        assert!(html.contains("Expected"));
        assert!(html.contains("Actual"));
        assert!(html.contains("brown"));
        assert!(html.contains("red"));
        assert!(html.contains("dog"));
        assert!(html.contains("cat"));
    }

    #[test]
    fn test_unified_diff_generation() {
        let expected = "line1\nline2\nline3";
        let actual = "line1\nmodified\nline3";

        let diff =
            DiffGenerator::generate_unified_diff(expected, actual, "before", "after").unwrap();

        assert!(diff.contains("--- before"));
        assert!(diff.contains("+++ after"));
        assert!(diff.contains("line2") || diff.contains("modified"));
    }

    #[test]
    fn test_inline_diff_generation() {
        let expected = "hello world";
        let actual = "hello rust";

        let diff = DiffGenerator::generate_inline_diff(expected, actual).unwrap();

        assert!(diff.contains("hello"));
        assert!(diff.contains("[-") || diff.contains("[+"));
    }

    #[test]
    fn test_similarity_ratio() {
        // Identical strings
        assert_eq!(DiffGenerator::similarity_ratio("test", "test"), 1.0);

        // Completely different
        let ratio = DiffGenerator::similarity_ratio("abc", "xyz");
        assert!(ratio < 0.5);

        // Similar strings
        let ratio = DiffGenerator::similarity_ratio("hello world", "hello rust");
        assert!(ratio > 0.5 && ratio < 1.0);
    }

    #[test]
    fn test_diff_stats() {
        let expected = "one two three";
        let actual = "one two four";

        let stats = DiffGenerator::diff_stats(expected, actual);

        assert!(stats.additions > 0);
        assert!(stats.deletions > 0);
        assert!(stats.unchanged > 0);
        assert!(stats.similarity > 0.0 && stats.similarity < 1.0);
    }

    #[test]
    fn test_markdown_diff_generation() {
        let expected = "original text";
        let actual = "modified text";

        let md = DiffGenerator::generate_markdown_diff(expected, actual).unwrap();

        assert!(md.contains("Diff Analysis"));
        assert!(md.contains("Similarity"));
        assert!(md.contains("```diff"));
    }

    #[test]
    fn test_html_escape() {
        let escaped = html_escape("<script>alert('xss')</script>");
        assert!(!escaped.contains("<script>"));
        assert!(escaped.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_multiline_diff() {
        let expected = "line 1\nline 2\nline 3\nline 4";
        let actual = "line 1\nmodified line 2\nline 3\nline 4";

        let html = DiffGenerator::generate_html_diff(expected, actual).unwrap();
        assert!(html.contains("line 1"));
        assert!(html.contains("line 2") || html.contains("modified"));

        let unified = DiffGenerator::generate_unified_diff(expected, actual, "a", "b").unwrap();
        assert!(unified.contains("---"));
        assert!(unified.contains("+++"));
    }
}
