//! URL detection for terminal content.
//!
//! This module provides URL detection functionality for vi mode navigation
//! and hint systems. It uses regex patterns to find URLs in terminal content.

use once_cell::sync::Lazy;
use regex::Regex;

use crate::grid::Dimensions;
use crate::index::{Column, Line, Point};

/// Default URL regex pattern.
///
/// This pattern matches common URL schemes including:
/// - http://, https://
/// - ftp://, file://
/// - ssh://, git://
/// - mailto:, tel:
/// - magnet:, ipfs://, ipns://
/// - gemini://, gopher://, news:
///
/// The pattern excludes control characters and common URL-invalid characters.
const URL_PATTERN: &str = concat!(
    // URL schemes
    r"(?:",
    r"https?://|",
    r"ftp://|",
    r"file://|",
    r"ssh://|",
    r"git://|",
    r"mailto:|",
    r"tel:|",
    r"magnet:\?|",
    r"ipfs://|",
    r"ipns://|",
    r"gemini://|",
    r"gopher://|",
    r"news:",
    r")",
    // URL body: word chars, common URL punctuation, exclude spaces and delimiters
    r"[\w\-.~:/?#@!$&'()*+,;=%]+",
);

/// Compiled URL regex.
static URL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(URL_PATTERN).expect("URL regex should compile"));

/// A detected URL match with its position in the terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UrlMatch {
    /// Start position (line, column).
    pub start: Point,
    /// End position (line, column) - inclusive.
    pub end: Point,
    /// The matched URL string.
    pub url: String,
}

impl UrlMatch {
    /// Check if a point is within this URL match.
    pub fn contains(&self, point: Point) -> bool {
        if point.line < self.start.line || point.line > self.end.line {
            return false;
        }
        if point.line == self.start.line && point.column < self.start.column {
            return false;
        }
        if point.line == self.end.line && point.column > self.end.column {
            return false;
        }
        true
    }
}

/// Find all URLs in the given text content.
///
/// The `line_contents` parameter is a function that returns the text content
/// of a given line, or `None` if the line is out of bounds.
///
/// Returns URLs in order from top-left to bottom-right.
pub fn find_urls<F>(
    topmost_line: Line,
    bottommost_line: Line,
    columns: usize,
    line_contents: F,
) -> Vec<UrlMatch>
where
    F: Fn(Line) -> Option<String>,
{
    let mut matches = Vec::new();

    let mut line = topmost_line;
    while line <= bottommost_line {
        if let Some(content) = line_contents(line) {
            for m in URL_REGEX.find_iter(&content) {
                let url = post_process_url(m.as_str());
                if url.is_empty() {
                    continue;
                }

                // Calculate end column accounting for post-processing
                let start_col = m.start();
                let end_col = start_col + url.len() - 1;

                // Ensure we don't exceed column bounds
                if start_col < columns {
                    matches.push(UrlMatch {
                        start: Point::new(line, Column(start_col)),
                        end: Point::new(line, Column(end_col.min(columns - 1))),
                        url,
                    });
                }
            }
        }
        line = line + 1;
    }

    matches
}

/// Post-process a URL match to handle common edge cases.
///
/// This handles:
/// - Unbalanced parentheses/brackets (e.g., "(https://example.com)" -> "https://example.com")
/// - Trailing punctuation (e.g., "https://example.com." -> "https://example.com")
fn post_process_url(url: &str) -> String {
    let mut result = url.to_string();

    // Handle unbalanced parentheses
    result = balance_delimiters(&result, '(', ')');
    result = balance_delimiters(&result, '[', ']');

    // Remove trailing punctuation
    while let Some(last) = result.chars().last() {
        if matches!(last, '.' | ',' | ':' | ';' | '?' | '!' | '\'' | '"') {
            result.pop();
        } else {
            break;
        }
    }

    result
}

/// Balance delimiters by truncating at unmatched closing delimiter.
fn balance_delimiters(s: &str, open: char, close: char) -> String {
    let mut depth = 0i32;
    let mut end_idx = s.len();

    for (i, c) in s.char_indices() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth < 0 {
                // Unmatched closing delimiter, truncate here
                end_idx = i;
                break;
            }
        }
    }

    s[..end_idx].to_string()
}

/// Find the next URL after the given point.
///
/// Returns the URL match if found, or `None` if no URL exists after the point.
pub fn find_next_url<D, F>(dims: &D, point: Point, line_contents: F) -> Option<UrlMatch>
where
    D: Dimensions,
    F: Fn(Line) -> Option<String>,
{
    let topmost = dims.topmost_line();
    let bottommost = dims.bottommost_line();
    let columns = dims.columns();

    let urls = find_urls(topmost, bottommost, columns, &line_contents);

    // Find first URL that starts after the current point
    for url in urls {
        // URL is "after" if it starts after the current point
        if url.start.line > point.line
            || (url.start.line == point.line && url.start.column.0 > point.column.0)
        {
            return Some(url);
        }
        // Or if current point is within this URL but not at the start,
        // skip to find the next one
    }

    // Wrap around: return first URL if any
    let urls = find_urls(topmost, bottommost, columns, line_contents);
    urls.into_iter().next()
}

/// Find the previous URL before the given point.
///
/// Returns the URL match if found, or `None` if no URL exists before the point.
pub fn find_prev_url<D, F>(dims: &D, point: Point, line_contents: F) -> Option<UrlMatch>
where
    D: Dimensions,
    F: Fn(Line) -> Option<String>,
{
    let topmost = dims.topmost_line();
    let bottommost = dims.bottommost_line();
    let columns = dims.columns();

    let urls = find_urls(topmost, bottommost, columns, &line_contents);

    // Find last URL that ends before the current point
    let mut last_before = None;
    for url in &urls {
        // URL is "before" if it ends before the current point
        if url.end.line < point.line
            || (url.end.line == point.line && url.end.column.0 < point.column.0)
        {
            last_before = Some(url.clone());
        }
    }

    if last_before.is_some() {
        return last_before;
    }

    // Wrap around: return last URL if any
    urls.into_iter().last()
}

/// Get the URL at the given point, if any.
pub fn url_at_point<D, F>(dims: &D, point: Point, line_contents: F) -> Option<UrlMatch>
where
    D: Dimensions,
    F: Fn(Line) -> Option<String>,
{
    let topmost = dims.topmost_line();
    let bottommost = dims.bottommost_line();
    let columns = dims.columns();

    let urls = find_urls(topmost, bottommost, columns, line_contents);

    urls.into_iter().find(|url| url.contains(point))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_dims(lines: usize, cols: usize) -> (usize, usize) {
        (lines, cols)
    }

    #[test]
    fn test_url_pattern_http() {
        let content = "Check out https://example.com for more info";
        let matches: Vec<_> = URL_REGEX.find_iter(content).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "https://example.com");
    }

    #[test]
    fn test_url_pattern_with_path() {
        let content = "Visit https://example.com/path/to/page?query=1&foo=bar";
        let matches: Vec<_> = URL_REGEX.find_iter(content).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(
            matches[0].as_str(),
            "https://example.com/path/to/page?query=1&foo=bar"
        );
    }

    #[test]
    fn test_url_pattern_multiple() {
        let content = "http://foo.com and https://bar.com and ftp://baz.com";
        let matches: Vec<_> = URL_REGEX.find_iter(content).collect();
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_url_pattern_mailto() {
        let content = "Email me at mailto:user@example.com";
        let matches: Vec<_> = URL_REGEX.find_iter(content).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "mailto:user@example.com");
    }

    #[test]
    fn test_url_pattern_ssh() {
        let content = "Clone with ssh://git@github.com/user/repo.git";
        let matches: Vec<_> = URL_REGEX.find_iter(content).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "ssh://git@github.com/user/repo.git");
    }

    #[test]
    fn test_post_process_trailing_period() {
        let url = "https://example.com.";
        assert_eq!(post_process_url(url), "https://example.com");
    }

    #[test]
    fn test_post_process_trailing_comma() {
        let url = "https://example.com,";
        assert_eq!(post_process_url(url), "https://example.com");
    }

    #[test]
    fn test_post_process_unbalanced_paren() {
        let url = "https://example.com)";
        assert_eq!(post_process_url(url), "https://example.com");
    }

    #[test]
    fn test_post_process_balanced_paren() {
        let url = "https://example.com/wiki/(disambiguation)";
        assert_eq!(
            post_process_url(url),
            "https://example.com/wiki/(disambiguation)"
        );
    }

    #[test]
    fn test_find_urls_single_line() {
        let lines = [
            "Hello world".to_string(),
            "Check https://example.com here".to_string(),
            "Goodbye".to_string(),
        ];

        let urls = find_urls(Line(0), Line(2), 80, |line| {
            lines.get(line.0 as usize).cloned()
        });

        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].url, "https://example.com");
        assert_eq!(urls[0].start.line, Line(1));
        assert_eq!(urls[0].start.column, Column(6));
    }

    #[test]
    fn test_find_next_url() {
        let dims = make_dims(5, 80);
        let lines = [
            "Line 0".to_string(),
            "URL1: https://first.com here".to_string(),
            "Line 2".to_string(),
            "URL2: https://second.com here".to_string(),
            "Line 4".to_string(),
        ];

        let get_line = |line: Line| lines.get(line.0 as usize).cloned();

        // From start, should find first URL
        let url = find_next_url(&dims, Point::new(Line(0), Column(0)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://first.com");

        // After first URL, should find second
        let url = find_next_url(&dims, Point::new(Line(1), Column(25)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://second.com");

        // After all URLs, should wrap to first
        let url = find_next_url(&dims, Point::new(Line(4), Column(0)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://first.com");
    }

    #[test]
    fn test_find_prev_url() {
        let dims = make_dims(5, 80);
        let lines = [
            "Line 0".to_string(),
            "URL1: https://first.com here".to_string(),
            "Line 2".to_string(),
            "URL2: https://second.com here".to_string(),
            "Line 4".to_string(),
        ];

        let get_line = |line: Line| lines.get(line.0 as usize).cloned();

        // From end, should find second URL
        let url = find_prev_url(&dims, Point::new(Line(4), Column(0)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://second.com");

        // Before second URL, should find first
        let url = find_prev_url(&dims, Point::new(Line(3), Column(0)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://first.com");

        // Before all URLs, should wrap to last
        let url = find_prev_url(&dims, Point::new(Line(0), Column(0)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://second.com");
    }

    #[test]
    fn test_url_at_point() {
        let dims = make_dims(3, 80);
        let lines = [
            "Hello world".to_string(),
            "Check https://example.com here".to_string(),
            "Goodbye".to_string(),
        ];

        let get_line = |line: Line| lines.get(line.0 as usize).cloned();

        // Point within URL
        let url = url_at_point(&dims, Point::new(Line(1), Column(10)), get_line);
        assert!(url.is_some());
        assert_eq!(url.unwrap().url, "https://example.com");

        // Point outside URL
        let url = url_at_point(&dims, Point::new(Line(1), Column(0)), get_line);
        assert!(url.is_none());
    }

    #[test]
    fn test_no_urls() {
        let lines = [
            "Hello world".to_string(),
            "No URLs here".to_string(),
            "Goodbye".to_string(),
        ];

        let urls = find_urls(Line(0), Line(2), 80, |line| {
            lines.get(line.0 as usize).cloned()
        });

        assert!(urls.is_empty());
    }
}
