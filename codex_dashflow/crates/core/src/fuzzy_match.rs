//! Fuzzy string matching for filtering and search
//!
//! Provides a simple case-insensitive subsequence matcher for fuzzy filtering.
//! Returns matched character indices for highlighting and a score for ranking.

/// Perform case-insensitive fuzzy matching of a needle in a haystack.
///
/// Returns the indices (character positions) of matched characters in the
/// original `haystack` and a score where smaller is better.
///
/// # Scoring
/// - Contiguous matches score better than spread matches
/// - Matches at the start of the string get a -100 bonus
/// - Empty needle returns `Some(([], i32::MAX))`
///
/// # Unicode
/// Matching is performed on lowercased copies while maintaining a mapping
/// back to original character positions. This handles Unicode edge cases
/// where lowercasing can expand characters (e.g., İ → i̇).
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::fuzzy_match::fuzzy_match;
///
/// // Basic match with highlighting indices
/// let (indices, score) = fuzzy_match("hello", "hl").unwrap();
/// assert_eq!(indices, vec![0, 2]); // 'h' at 0, 'l' at 2
///
/// // Case insensitive
/// assert!(fuzzy_match("FooBar", "foO").is_some());
///
/// // No match
/// assert!(fuzzy_match("hello", "xyz").is_none());
/// ```
pub fn fuzzy_match(haystack: &str, needle: &str) -> Option<(Vec<usize>, i32)> {
    if needle.is_empty() {
        return Some((Vec::new(), i32::MAX));
    }

    // Build lowercased haystack with mapping to original indices
    let mut lowered_chars: Vec<char> = Vec::new();
    let mut lowered_to_orig_char_idx: Vec<usize> = Vec::new();
    for (orig_idx, ch) in haystack.chars().enumerate() {
        for lc in ch.to_lowercase() {
            lowered_chars.push(lc);
            lowered_to_orig_char_idx.push(orig_idx);
        }
    }

    let lowered_needle: Vec<char> = needle.to_lowercase().chars().collect();

    // Find subsequence match positions
    let mut result_orig_indices: Vec<usize> = Vec::with_capacity(lowered_needle.len());
    let mut last_lower_pos: Option<usize> = None;
    let mut cur = 0usize;

    for &nc in lowered_needle.iter() {
        let mut found_at: Option<usize> = None;
        while cur < lowered_chars.len() {
            if lowered_chars[cur] == nc {
                found_at = Some(cur);
                cur += 1;
                break;
            }
            cur += 1;
        }
        let pos = found_at?;
        result_orig_indices.push(lowered_to_orig_char_idx[pos]);
        last_lower_pos = Some(pos);
    }

    // Calculate score
    let first_lower_pos = if result_orig_indices.is_empty() {
        0usize
    } else {
        let target_orig = result_orig_indices[0];
        lowered_to_orig_char_idx
            .iter()
            .position(|&oi| oi == target_orig)
            .unwrap_or(0)
    };

    // Score = extra span between first/last hit minus needle len
    // Strongly reward prefix matches by subtracting 100 when first hit is at index 0
    let last_lower_pos = last_lower_pos.unwrap_or(first_lower_pos);
    let window =
        (last_lower_pos as i32 - first_lower_pos as i32 + 1) - (lowered_needle.len() as i32);
    let mut score = window.max(0);
    if first_lower_pos == 0 {
        score -= 100;
    }

    result_orig_indices.sort_unstable();
    result_orig_indices.dedup();
    Some((result_orig_indices, score))
}

/// Get only the matched character indices for highlighting.
///
/// Convenience wrapper around [`fuzzy_match`] that discards the score.
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::fuzzy_match::fuzzy_indices;
///
/// let indices = fuzzy_indices("hello world", "hw").unwrap();
/// assert_eq!(indices, vec![0, 6]); // 'h' at 0, 'w' at 6
/// ```
pub fn fuzzy_indices(haystack: &str, needle: &str) -> Option<Vec<usize>> {
    fuzzy_match(haystack, needle).map(|(mut idx, _)| {
        idx.sort_unstable();
        idx.dedup();
        idx
    })
}

/// Check if a needle fuzzy-matches a haystack.
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::fuzzy_match::fuzzy_matches;
///
/// assert!(fuzzy_matches("hello", "hl"));
/// assert!(!fuzzy_matches("hello", "xyz"));
/// ```
pub fn fuzzy_matches(haystack: &str, needle: &str) -> bool {
    fuzzy_match(haystack, needle).is_some()
}

/// Get the fuzzy match score (lower is better).
///
/// Returns `None` if there's no match.
///
/// # Examples
/// ```no_run
/// use codex_dashflow_core::fuzzy_match::fuzzy_score;
///
/// // Prefix match scores better (lower)
/// let prefix_score = fuzzy_score("file_name", "file").unwrap();
/// let mid_score = fuzzy_score("my_file_name", "file").unwrap();
/// assert!(prefix_score < mid_score);
/// ```
pub fn fuzzy_score(haystack: &str, needle: &str) -> Option<i32> {
    fuzzy_match(haystack, needle).map(|(_, score)| score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ascii_basic_indices() {
        let (idx, score) = fuzzy_match("hello", "hl").unwrap();
        assert_eq!(idx, vec![0, 2]);
        // 'h' at 0, 'l' at 2 -> window 1; start-of-string bonus applies (-100)
        assert_eq!(score, -99);
    }

    #[test]
    fn test_unicode_dotted_i() {
        let (idx, score) = fuzzy_match("İstanbul", "is").unwrap();
        assert_eq!(idx, vec![0, 1]);
        // Start-of-string bonus applies
        assert_eq!(score, -99);
    }

    #[test]
    fn test_unicode_german_sharp_s() {
        // "straße" lowercased contains "ß" not "ss", so "strasse" won't match
        assert!(fuzzy_match("straße", "strasse").is_none());
    }

    #[test]
    fn test_prefer_contiguous_match() {
        let (_, score_a) = fuzzy_match("abc", "abc").unwrap();
        let (_, score_b) = fuzzy_match("a-b-c", "abc").unwrap();
        // Contiguous window -> 0; start-of-string bonus -> -100
        assert_eq!(score_a, -100);
        // Spread over 5 chars for 3-letter needle -> window 2; with bonus -> -98
        assert_eq!(score_b, -98);
        assert!(score_a < score_b);
    }

    #[test]
    fn test_start_of_string_bonus() {
        let (_, score_a) = fuzzy_match("file_name", "file").unwrap();
        let (_, score_b) = fuzzy_match("my_file_name", "file").unwrap();
        // Start-of-string contiguous -> window 0; bonus -> -100
        assert_eq!(score_a, -100);
        // Non-prefix contiguous -> window 0; no bonus -> 0
        assert_eq!(score_b, 0);
        assert!(score_a < score_b);
    }

    #[test]
    fn test_empty_needle() {
        let (idx, score) = fuzzy_match("anything", "").unwrap();
        assert!(idx.is_empty());
        assert_eq!(score, i32::MAX);
    }

    #[test]
    fn test_case_insensitive() {
        let (idx, score) = fuzzy_match("FooBar", "foO").unwrap();
        assert_eq!(idx, vec![0, 1, 2]);
        assert_eq!(score, -100);
    }

    #[test]
    fn test_no_match() {
        assert!(fuzzy_match("hello", "xyz").is_none());
        assert!(fuzzy_match("abc", "abcd").is_none()); // needle longer than haystack match
    }

    #[test]
    fn test_fuzzy_indices() {
        let indices = fuzzy_indices("hello world", "hw").unwrap();
        assert_eq!(indices, vec![0, 6]);
    }

    #[test]
    fn test_fuzzy_matches() {
        assert!(fuzzy_matches("hello", "hl"));
        assert!(!fuzzy_matches("hello", "xyz"));
    }

    #[test]
    fn test_fuzzy_score() {
        let score = fuzzy_score("hello", "hl").unwrap();
        assert_eq!(score, -99);
        assert!(fuzzy_score("hello", "xyz").is_none());
    }

    #[test]
    fn test_multichar_lowercase_expansion() {
        let needle = "\u{0069}\u{0307}"; // "i" + combining dot above
        let (idx, score) = fuzzy_match("İ", needle).unwrap();
        assert_eq!(idx, vec![0]);
        assert_eq!(score, -100);
    }
}
