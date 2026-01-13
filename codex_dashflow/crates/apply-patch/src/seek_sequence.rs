/// Attempt to find the sequence of `pattern` lines within `lines` beginning at or after `start`.
/// Returns the starting index of the match or `None` if not found. Matches are attempted with
/// decreasing strictness: exact match, then ignoring trailing whitespace, then ignoring leading
/// and trailing whitespace. When `eof` is true, we first try starting at the end-of-file (so that
/// patterns intended to match file endings are applied at the end), and fall back to searching
/// from `start` if needed.
///
/// Special cases handled defensively:
///  - Empty `pattern` -> returns `Some(start)` (no-op match)
///  - `pattern.len() > lines.len()` -> returns `None` (cannot match, avoids
///    out-of-bounds panic that occurred pre-2025-04-12)
pub(crate) fn seek_sequence(
    lines: &[String],
    pattern: &[String],
    start: usize,
    eof: bool,
) -> Option<usize> {
    if pattern.is_empty() {
        return Some(start);
    }

    // When the pattern is longer than the available input there is no possible
    // match. Early-return to avoid the out-of-bounds slice that would occur in
    // the search loops below (previously caused a panic when
    // `pattern.len() > lines.len()`).
    if pattern.len() > lines.len() {
        return None;
    }
    let search_start = if eof && lines.len() >= pattern.len() {
        lines.len() - pattern.len()
    } else {
        start
    };
    // Exact match first.
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        if lines[i..i + pattern.len()] == *pattern {
            return Some(i);
        }
    }
    // Then rstrip match.
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let mut ok = true;
        for (p_idx, pat) in pattern.iter().enumerate() {
            if lines[i + p_idx].trim_end() != pat.trim_end() {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(i);
        }
    }
    // Finally, trim both sides to allow more lenience.
    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let mut ok = true;
        for (p_idx, pat) in pattern.iter().enumerate() {
            if lines[i + p_idx].trim() != pat.trim() {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(i);
        }
    }

    // ------------------------------------------------------------------
    // Final, most permissive pass - attempt to match after *normalising*
    // common Unicode punctuation to their ASCII equivalents so that diffs
    // authored with plain ASCII characters can still be applied to source
    // files that contain typographic dashes / quotes, etc.  This mirrors the
    // fuzzy behaviour of `git apply` which ignores minor byte-level
    // differences when locating context lines.
    // ------------------------------------------------------------------

    fn normalise(s: &str) -> String {
        s.trim()
            .chars()
            .map(|c| match c {
                // Various dash / hyphen code-points -> ASCII '-'
                '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
                | '\u{2212}' => '-',
                // Fancy single quotes -> '\''
                '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' => '\'',
                // Fancy double quotes -> '"'
                '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' => '"',
                // Non-breaking space and other odd spaces -> normal space
                '\u{00A0}' | '\u{2002}' | '\u{2003}' | '\u{2004}' | '\u{2005}' | '\u{2006}'
                | '\u{2007}' | '\u{2008}' | '\u{2009}' | '\u{200A}' | '\u{202F}' | '\u{205F}'
                | '\u{3000}' => ' ',
                other => other,
            })
            .collect::<String>()
    }

    for i in search_start..=lines.len().saturating_sub(pattern.len()) {
        let mut ok = true;
        for (p_idx, pat) in pattern.iter().enumerate() {
            if normalise(&lines[i + p_idx]) != normalise(pat) {
                ok = false;
                break;
            }
        }
        if ok {
            return Some(i);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::seek_sequence;
    use std::string::ToString;

    fn to_vec(strings: &[&str]) -> Vec<String> {
        strings.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn test_exact_match_finds_sequence() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["bar", "baz"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(1));
    }

    #[test]
    fn test_rstrip_match_ignores_trailing_whitespace() {
        let lines = to_vec(&["foo   ", "bar\t\t"]);
        // Pattern omits trailing whitespace.
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_trim_match_ignores_leading_and_trailing_whitespace() {
        let lines = to_vec(&["    foo   ", "   bar\t"]);
        // Pattern omits any additional whitespace.
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_pattern_longer_than_input_returns_none() {
        let lines = to_vec(&["just one line"]);
        let pattern = to_vec(&["too", "many", "lines"]);
        // Should not panic - must return None when pattern cannot possibly fit.
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), None);
    }

    #[test]
    fn test_empty_pattern_returns_start() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern: Vec<String> = vec![];
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
        assert_eq!(seek_sequence(&lines, &pattern, 2, false), Some(2));
    }

    #[test]
    fn test_empty_lines_empty_pattern() {
        let lines: Vec<String> = vec![];
        let pattern: Vec<String> = vec![];
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_empty_lines_nonempty_pattern() {
        let lines: Vec<String> = vec![];
        let pattern = to_vec(&["foo"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), None);
    }

    #[test]
    fn test_exact_match_at_start() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_exact_match_at_end() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["bar", "baz"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(1));
    }

    #[test]
    fn test_exact_match_single_line() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(1));
    }

    #[test]
    fn test_exact_match_entire_file() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["foo", "bar", "baz"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_no_match_found() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["qux"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), None);
    }

    #[test]
    fn test_start_offset_skips_earlier_matches() {
        let lines = to_vec(&["foo", "bar", "foo", "bar"]);
        let pattern = to_vec(&["foo"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
        assert_eq!(seek_sequence(&lines, &pattern, 1, false), Some(2));
        assert_eq!(seek_sequence(&lines, &pattern, 3, false), None);
    }

    #[test]
    fn test_start_offset_beyond_possible_match() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["foo", "bar"]);
        // Start at position 2, but pattern needs 2 lines so can't fit
        assert_eq!(seek_sequence(&lines, &pattern, 2, false), None);
    }

    #[test]
    fn test_eof_mode_prefers_end_of_file() {
        let lines = to_vec(&["foo", "bar", "foo", "bar"]);
        let pattern = to_vec(&["foo", "bar"]);
        // Without eof, should find first occurrence
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
        // With eof, should prefer last occurrence (starts search at end)
        assert_eq!(seek_sequence(&lines, &pattern, 0, true), Some(2));
    }

    #[test]
    fn test_eof_mode_single_occurrence() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["bar", "baz"]);
        // Only one occurrence at the end
        assert_eq!(seek_sequence(&lines, &pattern, 0, true), Some(1));
    }

    #[test]
    fn test_eof_mode_no_match() {
        let lines = to_vec(&["foo", "bar", "baz"]);
        let pattern = to_vec(&["qux"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, true), None);
    }

    #[test]
    fn test_trailing_whitespace_tabs() {
        let lines = to_vec(&["foo\t\t\t", "bar\t"]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_trailing_whitespace_mixed() {
        let lines = to_vec(&["foo  \t ", "bar \t\t "]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_leading_whitespace_only() {
        let lines = to_vec(&["    foo", "  bar"]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_en_dash_normalized() {
        // U+2013 EN DASH should normalize to ASCII '-'
        let lines = to_vec(&["foo\u{2013}bar"]);
        let pattern = to_vec(&["foo-bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_em_dash_normalized() {
        // U+2014 EM DASH should normalize to ASCII '-'
        let lines = to_vec(&["foo\u{2014}bar"]);
        let pattern = to_vec(&["foo-bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_curly_quotes_normalized() {
        // U+201C/U+201D curly double quotes -> ASCII '"'
        let lines = to_vec(&["\u{201C}hello\u{201D}"]);
        let pattern = to_vec(&["\"hello\""]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_single_quotes_normalized() {
        // U+2018/U+2019 curly single quotes -> ASCII '\''
        let lines = to_vec(&["\u{2018}hello\u{2019}"]);
        let pattern = to_vec(&["'hello'"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_non_breaking_space_normalized() {
        // U+00A0 non-breaking space -> regular space
        let lines = to_vec(&["foo\u{00A0}bar"]);
        let pattern = to_vec(&["foo bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_figure_dash_normalized() {
        // U+2012 figure dash -> ASCII '-'
        let lines = to_vec(&["a\u{2012}b"]);
        let pattern = to_vec(&["a-b"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_minus_sign_normalized() {
        // U+2212 minus sign -> ASCII '-'
        let lines = to_vec(&["x\u{2212}y"]);
        let pattern = to_vec(&["x-y"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_unicode_ideographic_space_normalized() {
        // U+3000 ideographic space -> regular space
        let lines = to_vec(&["foo\u{3000}bar"]);
        let pattern = to_vec(&["foo bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_exact_match_preferred_over_whitespace_match() {
        let lines = to_vec(&["foo", "foo "]);
        let pattern = to_vec(&["foo"]);
        // Exact match at index 0 should be found first
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_multiple_line_pattern_with_whitespace() {
        let lines = to_vec(&["  foo  ", "  bar  ", "  baz  "]);
        let pattern = to_vec(&["foo", "bar"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_pattern_equals_lines_length() {
        let lines = to_vec(&["a", "b", "c"]);
        let pattern = to_vec(&["a", "b", "c"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_start_at_last_position() {
        let lines = to_vec(&["a", "b", "c"]);
        let pattern = to_vec(&["c"]);
        assert_eq!(seek_sequence(&lines, &pattern, 2, false), Some(2));
    }

    #[test]
    fn test_start_beyond_lines_length() {
        let lines = to_vec(&["a", "b", "c"]);
        let pattern = to_vec(&["a"]);
        // Start position 5 is beyond lines length (3), no panic expected
        assert_eq!(seek_sequence(&lines, &pattern, 5, false), None);
    }

    #[test]
    fn test_duplicate_lines_finds_first() {
        let lines = to_vec(&["dup", "dup", "dup"]);
        let pattern = to_vec(&["dup"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_eof_with_large_start() {
        let lines = to_vec(&["a", "b", "c", "d", "e"]);
        let pattern = to_vec(&["d", "e"]);
        // In eof mode, should start search from end
        assert_eq!(seek_sequence(&lines, &pattern, 0, true), Some(3));
    }

    #[test]
    fn test_whitespace_only_lines() {
        let lines = to_vec(&["   ", "\t\t", "  \t  "]);
        let pattern = to_vec(&["", "", ""]);
        // After trimming, all become empty strings
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }

    #[test]
    fn test_mixed_unicode_normalization() {
        // Mix of various unicode chars that should all normalize
        let lines = to_vec(&["\u{201C}test\u{201D}\u{2014}value\u{00A0}here"]);
        let pattern = to_vec(&["\"test\"-value here"]);
        assert_eq!(seek_sequence(&lines, &pattern, 0, false), Some(0));
    }
}
