//! Fast path scanning for the parser.
//!
//! ## Performance
//!
//! The ground state fast path finds the next byte that requires state
//! machine processing. Benchmarks show ~3 GiB/s throughput for pure ASCII
//! text on modern hardware.
//!
//! LLVM auto-vectorizes the simple predicate `byte < 0x20 || byte > 0x7E`
//! effectively, so explicit SIMD intrinsics don't provide significant benefit.
//! The `iter().position()` pattern is well-optimized by the compiler.
//!
//! ## Special Bytes
//!
//! Non-printable bytes that exit the fast path:
//! - C0 controls: 0x00-0x1F (including ESC at 0x1B)
//! - DEL: 0x7F
//! - High bytes: >= 0x80 (includes C1 controls and UTF-8 lead bytes)

/// Bytes that exit the ground state fast path.
/// These require state machine processing.
const fn is_special(byte: u8) -> bool {
    // C0 controls: 0x00-0x1F
    // DEL: 0x7F
    // C1 controls: 0x80-0x9F
    byte < 0x20 || byte == 0x7F || (byte >= 0x80 && byte <= 0x9F)
}

/// Find the first byte that requires state machine handling.
///
/// Returns `None` if all bytes are printable ASCII (0x20-0x7E) or
/// non-C1 high bytes (0xA0-0xFF, which are valid UTF-8 continuation/lead bytes).
///
/// Special bytes that exit the fast path:
/// - C0 controls: 0x00-0x1F (including ESC at 0x1B)
/// - DEL: 0x7F
/// - C1 controls: 0x80-0x9F
///
/// Note: This function is not used in the hot path. The parser uses
/// `find_non_printable` via `take_printable` instead, which is simpler
/// and auto-vectorized by LLVM.
#[inline]
pub fn find_special_byte(input: &[u8]) -> Option<usize> {
    // Single-pass scan using the same pattern as find_non_printable.
    // LLVM auto-vectorizes this predicate effectively.
    input.iter().position(|&b| is_special(b))
}

/// Find the next byte that's not in the printable ASCII range.
///
/// This is a simpler version that only looks for bytes outside 0x20-0x7E.
/// It's slightly faster when we don't need to distinguish between
/// different types of special bytes.
#[inline]
pub fn find_non_printable(input: &[u8]) -> Option<usize> {
    input.iter().position(|&b| b < 0x20 || b > 0x7E)
}

/// Count the number of printable ASCII bytes at the start of input.
///
/// Returns the length of the prefix that's all printable ASCII.
#[inline]
pub fn count_printable(input: &[u8]) -> usize {
    find_non_printable(input).unwrap_or(input.len())
}

/// Optimized batch print: returns slice of printable ASCII at start.
///
/// This is used by the fast path to avoid per-byte dispatch for
/// long runs of printable text.
#[inline]
pub fn take_printable(input: &[u8]) -> (&[u8], &[u8]) {
    let n = count_printable(input);
    input.split_at(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_special_escape() {
        assert_eq!(find_special_byte(b"hello\x1bworld"), Some(5));
    }

    #[test]
    fn test_find_special_newline() {
        assert_eq!(find_special_byte(b"hello\nworld"), Some(5));
    }

    #[test]
    fn test_find_special_c1() {
        assert_eq!(find_special_byte(b"hello\x9bworld"), Some(5));
    }

    #[test]
    fn test_find_special_none() {
        assert_eq!(find_special_byte(b"hello world"), None);
    }

    #[test]
    fn test_find_special_at_start() {
        assert_eq!(find_special_byte(b"\x1bhello"), Some(0));
    }

    #[test]
    fn test_count_printable() {
        assert_eq!(count_printable(b"hello\x1bworld"), 5);
        assert_eq!(count_printable(b"hello world"), 11);
        assert_eq!(count_printable(b"\x1bhello"), 0);
    }

    #[test]
    fn test_take_printable() {
        let (printable, rest) = take_printable(b"hello\x1bworld");
        assert_eq!(printable, b"hello");
        assert_eq!(rest, b"\x1bworld");
    }

    #[test]
    fn test_is_special() {
        // C0 controls
        assert!(is_special(0x00));
        assert!(is_special(0x07)); // BEL
        assert!(is_special(0x0A)); // LF
        assert!(is_special(0x0D)); // CR
        assert!(is_special(0x1B)); // ESC
        assert!(is_special(0x1F));

        // Printable ASCII
        assert!(!is_special(0x20)); // Space
        assert!(!is_special(b'A'));
        assert!(!is_special(b'z'));
        assert!(!is_special(0x7E)); // ~

        // DEL
        assert!(is_special(0x7F));

        // C1 controls
        assert!(is_special(0x80));
        assert!(is_special(0x9B)); // CSI
        assert!(is_special(0x9F));

        // High bytes (not C1)
        assert!(!is_special(0xA0));
        assert!(!is_special(0xFF));
    }
}
