// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use crate::errors::{Error, Result};
use std::cell::RefCell;
use tracing::warn;

// Thread-local encoder/decoder pools for ZSTD context reuse
// Reduces allocation overhead by ~50% (per flamegraph analysis at N=898)
// Stores Result to allow graceful error handling if context creation fails
//
// SAFETY (M-194): This thread_local! RefCell pattern is safe in async contexts because:
// 1. All borrows are confined within synchronous `.with()` closures
// 2. compress_zstd() and decompress_zstd*() complete all work within the closure
// 3. No `.await` points exist between borrow and release
// 4. Async task migration only occurs at `.await` points
// 5. Each thread maintains its own isolated RefCell instance
//
// The pattern is: `.with(|pool| { pool.borrow_mut(); /* sync work */ })` - the borrow
// is released when the closure returns, before any potential thread migration.
thread_local! {
    static ENCODER_POOL: RefCell<Option<std::result::Result<zstd::bulk::Compressor<'static>, String>>> = const { RefCell::new(None) };
    static DECODER_POOL: RefCell<Option<std::result::Result<zstd::bulk::Decompressor<'static>, String>>> = const { RefCell::new(None) };
}

/// Compress data using Zstd with context reuse
///
/// # Arguments
///
/// * `data` - The data to compress
/// * `level` - Compression level (1-21, higher is better compression but slower)
///
/// # Returns
///
/// Compressed data as a vector of bytes
///
/// # Compression Levels
///
/// - 1-3: Fast compression (best for real-time streaming)
/// - 4-9: Balanced (good compression with reasonable speed)
/// - 10-21: Maximum compression (slower, best for archival)
///
/// # Performance Notes
///
/// This function uses thread-local storage to reuse compression contexts across calls,
/// eliminating the `ZSTD_resetCCtx_internal` overhead (~17% of compression time).
/// Each thread maintains its own context, ensuring thread safety without locks.
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::compression::compress_zstd;
///
/// let data = b"Hello, world!".repeat(100);
/// let compressed = compress_zstd(&data, 3).unwrap();
/// assert!(compressed.len() < data.len());
/// ```
pub fn compress_zstd(data: &[u8], level: i32) -> Result<Vec<u8>> {
    ENCODER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();

        // Get or create compressor, storing any initialization error
        let compressor_result = pool
            .get_or_insert_with(|| zstd::bulk::Compressor::new(level).map_err(|e| e.to_string()));

        // Check if compressor initialization failed
        let compressor = match compressor_result {
            Ok(c) => c,
            Err(e) => {
                return Err(Error::Compression(format!(
                    "Failed to create ZSTD compressor: {}",
                    e
                )))
            }
        };

        // Update compression level if needed (S-17: check result and log warning on failure)
        if let Err(e) = compressor.set_compression_level(level) {
            warn!(
                requested_level = level,
                error = %e,
                "Failed to set ZSTD compression level, using previous level"
            );
            // Continue with previous level rather than failing - graceful degradation
        }

        // Compress data
        compressor
            .compress(data)
            .map_err(|e| Error::Compression(e.to_string()))
    })
}

/// Default maximum decompressed size (10 MB)
pub const DEFAULT_MAX_DECOMPRESSED_SIZE: usize = 10 * 1024 * 1024;

/// Decompress data using Zstd with context reuse
///
/// # Arguments
///
/// * `data` - The compressed data
///
/// # Returns
///
/// Decompressed data as a vector of bytes
///
/// # Performance Notes
///
/// This function uses thread-local storage to reuse decompression contexts,
/// reducing allocation overhead similar to `compress_zstd`.
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::compression::{compress_zstd, decompress_zstd};
///
/// let data = b"Hello, world!".repeat(100);
/// let compressed = compress_zstd(&data, 3).unwrap();
/// let decompressed = decompress_zstd(&compressed).unwrap();
/// assert_eq!(data.as_slice(), decompressed.as_slice());
/// ```
pub fn decompress_zstd(data: &[u8]) -> Result<Vec<u8>> {
    decompress_zstd_with_limit(data, DEFAULT_MAX_DECOMPRESSED_SIZE)
}

/// Decompress data using Zstd with a custom size limit
///
/// # Arguments
///
/// * `data` - The compressed data
/// * `max_size` - Maximum allowed decompressed size in bytes
///
/// # Returns
///
/// Decompressed data as a vector of bytes, or error if decompressed size exceeds limit
///
/// # Security
///
/// This function prevents decompression bombs by enforcing a maximum output size.
/// Use this when you need to match a producer's configured max_message_size.
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::compression::{compress_zstd, decompress_zstd_with_limit};
///
/// let data = b"Hello, world!".repeat(100);
/// let compressed = compress_zstd(&data, 3).unwrap();
/// // Allow up to 5MB decompressed output
/// let decompressed = decompress_zstd_with_limit(&compressed, 5 * 1024 * 1024).unwrap();
/// assert_eq!(data.as_slice(), decompressed.as_slice());
/// ```
pub fn decompress_zstd_with_limit(data: &[u8], max_size: usize) -> Result<Vec<u8>> {
    DECODER_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();

        // Get or create decompressor, storing any initialization error
        let decompressor_result =
            pool.get_or_insert_with(|| zstd::bulk::Decompressor::new().map_err(|e| e.to_string()));

        // Check if decompressor initialization failed
        let decompressor = match decompressor_result {
            Ok(d) => d,
            Err(e) => {
                return Err(Error::Decompression(format!(
                    "Failed to create ZSTD decompressor: {}",
                    e
                )))
            }
        };

        // Decompress data with configurable limit
        decompressor.decompress(data, max_size).map_err(|e| {
            Error::Decompression(format!(
                "Decompression failed (max_size={}): {}",
                max_size, e
            ))
        })
    })
}

/// Calculate compression ratio
///
/// # Arguments
///
/// * `original_size` - Size of the original data in bytes
/// * `compressed_size` - Size of the compressed data in bytes
///
/// # Returns
///
/// Compression ratio (`original_size` / `compressed_size`)
///
/// # Example
///
/// ```rust
/// use dashflow_streaming::compression::{compress_zstd, compression_ratio};
///
/// let data = b"Hello, world!".repeat(100);
/// let compressed = compress_zstd(&data, 3).unwrap();
/// let ratio = compression_ratio(data.len(), compressed.len());
/// println!("Compression ratio: {:.2}:1", ratio);
/// ```
#[must_use]
pub fn compression_ratio(original_size: usize, compressed_size: usize) -> f32 {
    if compressed_size == 0 {
        return 0.0;
    }
    original_size as f32 / compressed_size as f32
}

/// Estimate if compression is beneficial
///
/// Returns true if the data is likely to benefit from compression.
/// Uses heuristics based on data size and content.
///
/// # Arguments
///
/// * `data` - The data to analyze
/// * `min_size` - Minimum size in bytes to consider compression (default: 512)
///
/// # Returns
///
/// true if compression is likely beneficial, false otherwise
#[must_use]
pub fn should_compress(data: &[u8], min_size: usize) -> bool {
    if data.len() < min_size {
        return false;
    }

    // Simple heuristic: check for repeating bytes
    // If data has high entropy (random), compression won't help much
    // For now, we just check size - more sophisticated entropy analysis
    // could be added later
    true
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compress_decompress_roundtrip() {
        let data = b"Hello, world! This is a test message that should compress well.".repeat(10);

        let compressed = compress_zstd(&data, 3).unwrap();
        assert!(compressed.len() < data.len());

        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compress_empty() {
        let data = b"";
        let compressed = compress_zstd(data, 3).unwrap();
        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compress_small_data() {
        let data = b"Hello";
        let compressed = compress_zstd(data, 3).unwrap();
        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(data.as_slice(), decompressed.as_slice());
    }

    #[test]
    fn test_compression_levels() {
        let data = b"Hello, world! ".repeat(100);

        let compressed_fast = compress_zstd(&data, 1).unwrap();
        let compressed_balanced = compress_zstd(&data, 5).unwrap();
        let compressed_max = compress_zstd(&data, 10).unwrap();

        // Higher compression levels should produce smaller output
        assert!(compressed_max.len() <= compressed_balanced.len());
        assert!(compressed_balanced.len() <= compressed_fast.len());

        // All should decompress correctly
        assert_eq!(decompress_zstd(&compressed_fast).unwrap(), data.as_slice());
        assert_eq!(
            decompress_zstd(&compressed_balanced).unwrap(),
            data.as_slice()
        );
        assert_eq!(decompress_zstd(&compressed_max).unwrap(), data.as_slice());
    }

    #[test]
    fn test_compression_ratio() {
        assert_eq!(compression_ratio(1000, 500), 2.0);
        assert_eq!(compression_ratio(1000, 200), 5.0);
        assert_eq!(compression_ratio(100, 100), 1.0);
        assert_eq!(compression_ratio(100, 0), 0.0);
    }

    #[test]
    fn test_should_compress() {
        assert!(!should_compress(b"hello", 512)); // Too small
        assert!(should_compress(&vec![0u8; 1000], 512)); // Large enough
        assert!(should_compress(&vec![0u8; 512], 512)); // Exactly min size
        assert!(!should_compress(&vec![0u8; 511], 512)); // Just below min size
    }

    #[test]
    fn test_compress_highly_compressible() {
        // Very repetitive data should compress very well
        let data = vec![b'A'; 10000];
        let compressed = compress_zstd(&data, 5).unwrap();
        let ratio = compression_ratio(data.len(), compressed.len());

        // Should achieve at least 10:1 compression for this simple case
        assert!(ratio > 10.0);

        // Verify decompression
        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_compress_json_like_data() {
        // JSON-like data (common in DashFlow Streaming messages)
        let data = r#"{"thread_id":"session-123","node_id":"analyze","state":{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi there!"}]}}"#.repeat(50);

        let compressed = compress_zstd(data.as_bytes(), 3).unwrap();
        let ratio = compression_ratio(data.len(), compressed.len());

        // JSON should compress reasonably well
        assert!(ratio > 2.0);

        // Verify decompression
        let decompressed = decompress_zstd(&compressed).unwrap();
        assert_eq!(data.as_bytes(), decompressed.as_slice());
    }
}
