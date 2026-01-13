//! Compression support for `DashFlow` checkpointers
//!
//! This crate provides compression algorithms for reducing storage size
//! of checkpoint data. It supports:
//!
//! - **Zstd**: Best compression ratio (5-10×), moderate speed
//! - **LZ4**: Fastest compression (2-3×), lowest CPU usage
//! - **Snappy**: Balanced (3-5×), good for mixed workloads
//!
//! # Example
//!
//! ```rust
//! use dashflow_compression::{Compression, CompressionType};
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a Zstd compressor with level 3
//! let compressor = CompressionType::Zstd(3).build()?;
//!
//! let data = b"Hello, world! This is a test of compression.";
//! let compressed = compressor.compress(data)?;
//! let decompressed = compressor.decompress(&compressed)?;
//!
//! assert_eq!(data, decompressed.as_slice());
//! # Ok(())
//! # }
//! ```

use thiserror::Error;

/// Default maximum decompressed size: 100 MB
/// This prevents decompression bombs from consuming all available memory.
pub const DEFAULT_MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;

/// Compression errors
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum CompressionError {
    /// Compression failed
    #[error("Compression failed: {0}")]
    CompressionFailed(String),

    /// Decompression failed
    #[error("Decompression failed: {0}")]
    DecompressionFailed(String),

    /// Feature not enabled
    #[error("Compression type not available: {0}. Enable the corresponding feature flag.")]
    FeatureNotEnabled(String),

    /// Decompressed size exceeds maximum allowed limit (protection against decompression bombs)
    #[error("Decompressed size {actual} exceeds maximum allowed {max_allowed} bytes")]
    DecompressionSizeExceeded {
        /// Actual or estimated decompressed size
        actual: usize,
        /// Maximum allowed decompressed size
        max_allowed: usize,
    },
}

/// Compression trait for implementing different compression algorithms
pub trait Compression: Send + Sync {
    /// Compress data
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data
    ///
    /// **Warning:** This method has no size limit and could be vulnerable to
    /// decompression bombs. For untrusted input, use [`Self::decompress_with_limit`]
    /// instead.
    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError>;

    /// Decompress data with a maximum output size limit
    ///
    /// This method provides protection against decompression bombs (malicious
    /// compressed data that expands to enormous sizes). If the decompressed
    /// data would exceed `max_size`, returns [`CompressionError::DecompressionSizeExceeded`].
    ///
    /// # Arguments
    ///
    /// * `data` - Compressed data to decompress
    /// * `max_size` - Maximum allowed decompressed size in bytes
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow_compression::{Compression, CompressionType, DEFAULT_MAX_DECOMPRESSED_SIZE};
    ///
    /// let compressor = CompressionType::Zstd(3).build().unwrap();
    /// let data = b"Hello, world!";
    /// let compressed_data = compressor.compress(data).unwrap();
    /// let result = compressor.decompress_with_limit(&compressed_data, DEFAULT_MAX_DECOMPRESSED_SIZE);
    /// assert!(result.is_ok());
    /// ```
    fn decompress_with_limit(
        &self,
        data: &[u8],
        max_size: usize,
    ) -> Result<Vec<u8>, CompressionError> {
        let result = self.decompress(data)?;
        if result.len() > max_size {
            return Err(CompressionError::DecompressionSizeExceeded {
                actual: result.len(),
                max_allowed: max_size,
            });
        }
        Ok(result)
    }

    /// Decompress data with the default size limit (100 MB)
    ///
    /// This is a convenience method that calls [`Self::decompress_with_limit`] with
    /// [`DEFAULT_MAX_DECOMPRESSED_SIZE`].
    fn decompress_safe(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        self.decompress_with_limit(data, DEFAULT_MAX_DECOMPRESSED_SIZE)
    }

    /// Get the name of the compression algorithm
    fn name(&self) -> &str;
}

/// Compression type with configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressionType {
    /// Zstd compression with level (1-22, default: 3)
    /// - Best ratio: 5-10× compression
    /// - Moderate speed
    /// - Good for large states
    Zstd(i32),

    /// LZ4 compression (no level configuration)
    /// - Fastest: 2-3× compression
    /// - Lowest CPU usage
    /// - Good for frequent checkpoints
    Lz4,

    /// Snappy compression (no level configuration)
    /// - Balanced: 3-5× compression
    /// - Good for mixed workloads
    Snappy,
}

impl CompressionType {
    /// Build a compression instance from this type
    pub fn build(self) -> Result<Box<dyn Compression>, CompressionError> {
        match self {
            #[cfg(feature = "zstd")]
            CompressionType::Zstd(level) => Ok(Box::new(ZstdCompression { level })),
            #[cfg(not(feature = "zstd"))]
            CompressionType::Zstd(_) => Err(CompressionError::FeatureNotEnabled(
                "zstd (enable with feature: zstd)".to_string(),
            )),

            #[cfg(feature = "lz4")]
            CompressionType::Lz4 => Ok(Box::new(Lz4Compression)),
            #[cfg(not(feature = "lz4"))]
            CompressionType::Lz4 => Err(CompressionError::FeatureNotEnabled(
                "lz4 (enable with feature: lz4)".to_string(),
            )),

            #[cfg(feature = "snappy")]
            CompressionType::Snappy => Ok(Box::new(SnappyCompression)),
            #[cfg(not(feature = "snappy"))]
            CompressionType::Snappy => Err(CompressionError::FeatureNotEnabled(
                "snappy (enable with feature: snappy)".to_string(),
            )),
        }
    }

    /// Get the name of this compression type
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            CompressionType::Zstd(_) => "zstd",
            CompressionType::Lz4 => "lz4",
            CompressionType::Snappy => "snappy",
        }
    }
}

impl Default for CompressionType {
    /// Default compression: Zstd level 3
    fn default() -> Self {
        CompressionType::Zstd(3)
    }
}

// Zstd implementation
#[cfg(feature = "zstd")]
struct ZstdCompression {
    level: i32,
}

#[cfg(feature = "zstd")]
impl Compression for ZstdCompression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        zstd::encode_all(data, self.level).map_err(|e| {
            CompressionError::CompressionFailed(format!("Zstd compression failed: {e}"))
        })
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        zstd::decode_all(data).map_err(|e| {
            CompressionError::DecompressionFailed(format!("Zstd decompression failed: {e}"))
        })
    }

    fn name(&self) -> &'static str {
        "zstd"
    }
}

// LZ4 implementation
#[cfg(feature = "lz4")]
struct Lz4Compression;

#[cfg(feature = "lz4")]
impl Compression for Lz4Compression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        Ok(lz4_flex::compress_prepend_size(data))
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        lz4_flex::decompress_size_prepended(data).map_err(|e| {
            CompressionError::DecompressionFailed(format!("LZ4 decompression failed: {e}"))
        })
    }

    fn name(&self) -> &'static str {
        "lz4"
    }
}

// Snappy implementation
#[cfg(feature = "snappy")]
struct SnappyCompression;

#[cfg(feature = "snappy")]
impl Compression for SnappyCompression {
    fn compress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut encoder = snap::raw::Encoder::new();
        encoder.compress_vec(data).map_err(|e| {
            CompressionError::CompressionFailed(format!("Snappy compression failed: {e}"))
        })
    }

    fn decompress(&self, data: &[u8]) -> Result<Vec<u8>, CompressionError> {
        let mut decoder = snap::raw::Decoder::new();
        decoder.decompress_vec(data).map_err(|e| {
            CompressionError::DecompressionFailed(format!("Snappy decompression failed: {e}"))
        })
    }

    fn name(&self) -> &'static str {
        "snappy"
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    const TEST_DATA: &[u8] = b"Hello, world! This is a test of compression. \
                                We need some repetitive data to test compression ratios. \
                                Repetitive data compresses well. Repetitive data is good. \
                                More repetitive data here. Even more repetitive data.";

    // ============================================================
    // CompressionType Tests
    // ============================================================

    #[test]
    fn test_compression_type_name() {
        assert_eq!(CompressionType::Zstd(3).name(), "zstd");
        assert_eq!(CompressionType::Lz4.name(), "lz4");
        assert_eq!(CompressionType::Snappy.name(), "snappy");
    }

    #[test]
    fn test_default_compression() {
        let default = CompressionType::default();
        assert_eq!(default, CompressionType::Zstd(3));
    }

    #[test]
    fn test_compression_type_clone() {
        let original = CompressionType::Zstd(5);
        let cloned = original.clone();
        assert_eq!(original, cloned);

        let lz4 = CompressionType::Lz4;
        let lz4_cloned = lz4.clone();
        assert_eq!(lz4, lz4_cloned);

        let snappy = CompressionType::Snappy;
        let snappy_cloned = snappy.clone();
        assert_eq!(snappy, snappy_cloned);
    }

    #[test]
    fn test_compression_type_copy() {
        let original = CompressionType::Zstd(7);
        let copied = original; // Copy, not move
        assert_eq!(original, copied);

        // original is still usable (Copy trait)
        assert_eq!(original.name(), "zstd");
    }

    #[test]
    fn test_compression_type_debug() {
        let zstd = CompressionType::Zstd(3);
        let debug_str = format!("{:?}", zstd);
        assert!(debug_str.contains("Zstd"));
        assert!(debug_str.contains("3"));

        let lz4 = CompressionType::Lz4;
        let debug_str = format!("{:?}", lz4);
        assert!(debug_str.contains("Lz4"));

        let snappy = CompressionType::Snappy;
        let debug_str = format!("{:?}", snappy);
        assert!(debug_str.contains("Snappy"));
    }

    #[test]
    fn test_compression_type_eq() {
        // Same type and level
        assert_eq!(CompressionType::Zstd(3), CompressionType::Zstd(3));

        // Different levels
        assert_ne!(CompressionType::Zstd(3), CompressionType::Zstd(5));

        // Different types
        assert_ne!(CompressionType::Zstd(3), CompressionType::Lz4);
        assert_ne!(CompressionType::Lz4, CompressionType::Snappy);
        assert_ne!(CompressionType::Zstd(1), CompressionType::Snappy);
    }

    #[test]
    fn test_compression_type_name_zstd_various_levels() {
        // Name should be "zstd" regardless of level
        assert_eq!(CompressionType::Zstd(1).name(), "zstd");
        assert_eq!(CompressionType::Zstd(10).name(), "zstd");
        assert_eq!(CompressionType::Zstd(22).name(), "zstd");
        assert_eq!(CompressionType::Zstd(0).name(), "zstd");
        assert_eq!(CompressionType::Zstd(-1).name(), "zstd");
    }

    // ============================================================
    // CompressionError Tests
    // ============================================================

    #[test]
    fn test_compression_error_display_compression_failed() {
        let err = CompressionError::CompressionFailed("test error message".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Compression failed"));
        assert!(display.contains("test error message"));
    }

    #[test]
    fn test_compression_error_display_decompression_failed() {
        let err = CompressionError::DecompressionFailed("invalid data".to_string());
        let display = format!("{}", err);
        assert!(display.contains("Decompression failed"));
        assert!(display.contains("invalid data"));
    }

    #[test]
    fn test_compression_error_display_feature_not_enabled() {
        let err = CompressionError::FeatureNotEnabled("zstd".to_string());
        let display = format!("{}", err);
        assert!(display.contains("not available"));
        assert!(display.contains("zstd"));
    }

    #[test]
    fn test_compression_error_display_size_exceeded() {
        let err = CompressionError::DecompressionSizeExceeded {
            actual: 1000,
            max_allowed: 100,
        };
        let display = format!("{}", err);
        assert!(display.contains("1000"));
        assert!(display.contains("100"));
        assert!(display.contains("exceeds"));
    }

    #[test]
    fn test_compression_error_debug() {
        let err = CompressionError::CompressionFailed("debug test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("CompressionFailed"));
        assert!(debug_str.contains("debug test"));
    }

    #[test]
    fn test_compression_error_is_error_trait() {
        // Verify that CompressionError implements std::error::Error
        fn assert_error<T: std::error::Error>(_: &T) {}
        let err = CompressionError::CompressionFailed("test".to_string());
        assert_error(&err);
    }

    // ============================================================
    // DEFAULT_MAX_DECOMPRESSED_SIZE Tests
    // ============================================================

    #[test]
    fn test_default_max_decompressed_size_value() {
        // 100 MB = 100 * 1024 * 1024
        assert_eq!(DEFAULT_MAX_DECOMPRESSED_SIZE, 104_857_600);
    }

    #[test]
    fn test_default_max_decompressed_size_is_100mb() {
        // Verify it's exactly 100 MB
        assert_eq!(DEFAULT_MAX_DECOMPRESSED_SIZE, 100 * 1024 * 1024);
    }

    // ============================================================
    // Zstd Tests
    // ============================================================

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_roundtrip() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(TEST_DATA, decompressed.as_slice());
        // Verify compression actually happened
        assert!(compressed.len() < TEST_DATA.len());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_level_1_fastest() {
        let compressor = CompressionType::Zstd(1).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(TEST_DATA, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_level_22_maximum() {
        let compressor = CompressionType::Zstd(22).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(TEST_DATA, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_name_via_trait() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        assert_eq!(compressor.name(), "zstd");
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_empty_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let empty: &[u8] = &[];
        let compressed = compressor.compress(empty).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_single_byte() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let single_byte: &[u8] = &[0x42];
        let compressed = compressor.compress(single_byte).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(single_byte, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_highly_compressible_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        // All zeros - highly compressible
        let zeros = vec![0u8; 10000];
        let compressed = compressor.compress(&zeros).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(zeros, decompressed);
        // Should compress very well
        assert!(compressed.len() < zeros.len() / 10); // Better than 10:1
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_binary_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        // Binary data with all byte values
        let binary: Vec<u8> = (0..=255).collect();
        let compressed = compressor.compress(&binary).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(binary, decompressed);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_unicode_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let unicode = "こんにちは世界🌍🦀 Привет мир مرحبا".as_bytes();
        let compressed = compressor.compress(unicode).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(unicode, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_decompress_invalid_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let invalid_data = b"this is not valid zstd compressed data";
        let result = compressor.decompress(invalid_data);
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::DecompressionFailed(msg) => {
                assert!(msg.contains("Zstd"));
            }
            other => panic!("Expected DecompressionFailed, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_decompress_with_limit_within_limit() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Limit larger than data - should succeed
        let result = compressor.decompress_with_limit(&compressed, TEST_DATA.len() + 100);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TEST_DATA);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_decompress_with_limit_exceeds_limit() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Limit smaller than data - should fail
        let result = compressor.decompress_with_limit(&compressed, 10);
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::DecompressionSizeExceeded { actual, max_allowed } => {
                assert_eq!(actual, TEST_DATA.len());
                assert_eq!(max_allowed, 10);
            }
            other => panic!("Expected DecompressionSizeExceeded, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_decompress_with_limit_exact_size() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Limit exactly equals data size - should succeed
        let result = compressor.decompress_with_limit(&compressed, TEST_DATA.len());
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TEST_DATA);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_decompress_safe() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // decompress_safe uses DEFAULT_MAX_DECOMPRESSED_SIZE (100 MB)
        // Our test data is small, so this should succeed
        let result = compressor.decompress_safe(&compressed);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TEST_DATA);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_different_levels_produce_valid_output() {
        // All levels should produce valid decompressible output
        for level in [1, 3, 5, 10, 15, 19, 22] {
            let compressor = CompressionType::Zstd(level).build().unwrap();
            let compressed = compressor.compress(TEST_DATA).unwrap();
            let decompressed = compressor.decompress(&compressed).unwrap();
            assert_eq!(TEST_DATA, decompressed.as_slice(), "Failed at level {}", level);
        }
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_higher_level_better_compression() {
        let low_compressor = CompressionType::Zstd(1).build().unwrap();
        let high_compressor = CompressionType::Zstd(19).build().unwrap();

        // Use highly compressible data
        let data = "a".repeat(10000);

        let low_compressed = low_compressor.compress(data.as_bytes()).unwrap();
        let high_compressed = high_compressor.compress(data.as_bytes()).unwrap();

        // Higher level should generally produce smaller output
        assert!(high_compressed.len() <= low_compressed.len());
    }

    // ============================================================
    // LZ4 Tests
    // ============================================================

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_roundtrip() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(TEST_DATA, decompressed.as_slice());
        // Verify compression actually happened
        assert!(compressed.len() < TEST_DATA.len());
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_name_via_trait() {
        let compressor = CompressionType::Lz4.build().unwrap();
        assert_eq!(compressor.name(), "lz4");
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_empty_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let empty: &[u8] = &[];
        let compressed = compressor.compress(empty).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_single_byte() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let single_byte: &[u8] = &[0x42];
        let compressed = compressor.compress(single_byte).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(single_byte, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_highly_compressible_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let zeros = vec![0u8; 10000];
        let compressed = compressor.compress(&zeros).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(zeros, decompressed);
        // LZ4 should still achieve decent compression on zeros
        assert!(compressed.len() < zeros.len() / 5);
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_binary_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let binary: Vec<u8> = (0..=255).collect();
        let compressed = compressor.compress(&binary).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(binary, decompressed);
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_unicode_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let unicode = "こんにちは世界🌍🦀 Привет мир مرحبا".as_bytes();
        let compressed = compressor.compress(unicode).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(unicode, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_decompress_invalid_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let invalid_data = b"this is not valid lz4 compressed data at all";
        let result = compressor.decompress(invalid_data);
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::DecompressionFailed(msg) => {
                assert!(msg.contains("LZ4"));
            }
            other => panic!("Expected DecompressionFailed, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_decompress_with_limit() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Within limit - success
        let result = compressor.decompress_with_limit(&compressed, TEST_DATA.len() + 100);
        assert!(result.is_ok());

        // Exceeds limit - error
        let result = compressor.decompress_with_limit(&compressed, 10);
        assert!(matches!(
            result,
            Err(CompressionError::DecompressionSizeExceeded { .. })
        ));
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_decompress_safe() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let result = compressor.decompress_safe(&compressed);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TEST_DATA);
    }

    // ============================================================
    // Snappy Tests
    // ============================================================

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_roundtrip() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(TEST_DATA, decompressed.as_slice());
        // Verify compression actually happened
        assert!(compressed.len() < TEST_DATA.len());
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_name_via_trait() {
        let compressor = CompressionType::Snappy.build().unwrap();
        assert_eq!(compressor.name(), "snappy");
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_empty_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let empty: &[u8] = &[];
        let compressed = compressor.compress(empty).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert!(decompressed.is_empty());
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_single_byte() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let single_byte: &[u8] = &[0x42];
        let compressed = compressor.compress(single_byte).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(single_byte, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_highly_compressible_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let zeros = vec![0u8; 10000];
        let compressed = compressor.compress(&zeros).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(zeros, decompressed);
        // Snappy should achieve decent compression
        assert!(compressed.len() < zeros.len() / 5);
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_binary_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let binary: Vec<u8> = (0..=255).collect();
        let compressed = compressor.compress(&binary).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(binary, decompressed);
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_unicode_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let unicode = "こんにちは世界🌍🦀 Привет мир مرحبا".as_bytes();
        let compressed = compressor.compress(unicode).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(unicode, decompressed.as_slice());
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_decompress_invalid_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let invalid_data = b"this is not valid snappy compressed data";
        let result = compressor.decompress(invalid_data);
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::DecompressionFailed(msg) => {
                assert!(msg.contains("Snappy"));
            }
            other => panic!("Expected DecompressionFailed, got {:?}", other),
        }
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_decompress_with_limit() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Within limit - success
        let result = compressor.decompress_with_limit(&compressed, TEST_DATA.len() + 100);
        assert!(result.is_ok());

        // Exceeds limit - error
        let result = compressor.decompress_with_limit(&compressed, 10);
        assert!(matches!(
            result,
            Err(CompressionError::DecompressionSizeExceeded { .. })
        ));
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_decompress_safe() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();
        let result = compressor.decompress_safe(&compressed);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), TEST_DATA);
    }

    // ============================================================
    // Feature-Independent Build Tests
    // ============================================================

    #[test]
    #[cfg(not(feature = "zstd"))]
    fn test_zstd_feature_not_enabled() {
        let result = CompressionType::Zstd(3).build();
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::FeatureNotEnabled(msg) => {
                assert!(msg.contains("zstd"));
            }
            other => panic!("Expected FeatureNotEnabled, got {:?}", other),
        }
    }

    #[test]
    #[cfg(not(feature = "lz4"))]
    fn test_lz4_feature_not_enabled() {
        let result = CompressionType::Lz4.build();
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::FeatureNotEnabled(msg) => {
                assert!(msg.contains("lz4"));
            }
            other => panic!("Expected FeatureNotEnabled, got {:?}", other),
        }
    }

    #[test]
    #[cfg(not(feature = "snappy"))]
    fn test_snappy_feature_not_enabled() {
        let result = CompressionType::Snappy.build();
        assert!(result.is_err());
        match result.unwrap_err() {
            CompressionError::FeatureNotEnabled(msg) => {
                assert!(msg.contains("snappy"));
            }
            other => panic!("Expected FeatureNotEnabled, got {:?}", other),
        }
    }

    // ============================================================
    // Cross-Algorithm Tests
    // ============================================================

    #[test]
    #[cfg(all(feature = "zstd", feature = "lz4"))]
    fn test_zstd_lz4_different_formats() {
        let zstd_compressor = CompressionType::Zstd(3).build().unwrap();
        let lz4_compressor = CompressionType::Lz4.build().unwrap();

        let zstd_compressed = zstd_compressor.compress(TEST_DATA).unwrap();
        let lz4_compressed = lz4_compressor.compress(TEST_DATA).unwrap();

        // Different formats should produce different output
        assert_ne!(zstd_compressed, lz4_compressed);

        // Each should only decompress its own format
        assert!(lz4_compressor.decompress(&zstd_compressed).is_err());
        assert!(zstd_compressor.decompress(&lz4_compressed).is_err());
    }

    #[test]
    #[cfg(all(feature = "zstd", feature = "snappy"))]
    fn test_zstd_snappy_different_formats() {
        let zstd_compressor = CompressionType::Zstd(3).build().unwrap();
        let snappy_compressor = CompressionType::Snappy.build().unwrap();

        let zstd_compressed = zstd_compressor.compress(TEST_DATA).unwrap();
        let snappy_compressed = snappy_compressor.compress(TEST_DATA).unwrap();

        // Different formats should produce different output
        assert_ne!(zstd_compressed, snappy_compressed);
    }

    #[test]
    #[cfg(all(feature = "lz4", feature = "snappy"))]
    fn test_lz4_snappy_different_formats() {
        let lz4_compressor = CompressionType::Lz4.build().unwrap();
        let snappy_compressor = CompressionType::Snappy.build().unwrap();

        let lz4_compressed = lz4_compressor.compress(TEST_DATA).unwrap();
        let snappy_compressed = snappy_compressor.compress(TEST_DATA).unwrap();

        // Different formats should produce different output
        assert_ne!(lz4_compressed, snappy_compressed);
    }

    // ============================================================
    // Large Data Tests
    // ============================================================

    #[test]
    #[cfg(feature = "zstd")]
    fn test_zstd_large_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        // 1 MB of data
        let large_data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
        let compressed = compressor.compress(&large_data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(large_data, decompressed);
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_large_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        // 1 MB of data
        let large_data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
        let compressed = compressor.compress(&large_data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(large_data, decompressed);
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_large_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        // 1 MB of data
        let large_data: Vec<u8> = (0..1_000_000).map(|i| (i % 256) as u8).collect();
        let compressed = compressor.compress(&large_data).unwrap();
        let decompressed = compressor.decompress(&compressed).unwrap();
        assert_eq!(large_data, decompressed);
    }

    // ============================================================
    // Edge Cases
    // ============================================================

    #[test]
    #[cfg(feature = "zstd")]
    fn test_compress_decompress_limit_zero() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let data = b"test";
        let compressed = compressor.compress(data).unwrap();

        // Zero limit should always fail if there's any data
        let result = compressor.decompress_with_limit(&compressed, 0);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_decompress_truncated_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Truncate the compressed data
        let truncated = &compressed[..compressed.len() / 2];
        let result = compressor.decompress(truncated);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "lz4")]
    fn test_lz4_decompress_truncated_data() {
        let compressor = CompressionType::Lz4.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Truncate the compressed data
        let truncated = &compressed[..compressed.len() / 2];
        let result = compressor.decompress(truncated);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "snappy")]
    fn test_snappy_decompress_truncated_data() {
        let compressor = CompressionType::Snappy.build().unwrap();
        let compressed = compressor.compress(TEST_DATA).unwrap();

        // Truncate the compressed data
        let truncated = &compressed[..compressed.len() / 2];
        let result = compressor.decompress(truncated);
        assert!(result.is_err());
    }

    #[test]
    fn test_compression_type_with_same_values_are_equal() {
        let a = CompressionType::Zstd(5);
        let b = CompressionType::Zstd(5);
        assert!(a == b);
        assert!(a.eq(&b));
    }

    #[test]
    fn test_compression_type_reflexive_equality() {
        let ct = CompressionType::Lz4;
        assert_eq!(ct, ct);
    }

    #[test]
    fn test_compression_type_symmetric_equality() {
        let a = CompressionType::Snappy;
        let b = CompressionType::Snappy;
        assert_eq!(a, b);
        assert_eq!(b, a);
    }

    #[test]
    #[cfg(feature = "zstd")]
    fn test_multiple_compress_same_data() {
        let compressor = CompressionType::Zstd(3).build().unwrap();

        // Compressing the same data multiple times should produce consistent results
        let compressed1 = compressor.compress(TEST_DATA).unwrap();
        let compressed2 = compressor.compress(TEST_DATA).unwrap();

        // Both should decompress to the same data
        let decompressed1 = compressor.decompress(&compressed1).unwrap();
        let decompressed2 = compressor.decompress(&compressed2).unwrap();
        assert_eq!(decompressed1, decompressed2);
        assert_eq!(decompressed1, TEST_DATA);
    }
}
