//! Bloom filter for fast negative lookups.
//!
//! A bloom filter provides O(1) probabilistic set membership tests.
//! It has no false negatives but may have false positives.
//!
//! ## Design
//!
//! - Uses k=7 hash functions (optimal for 1% false positive rate)
//! - Bit array stored as `Vec<u64>` for cache efficiency
//! - FNV-1a based hash functions for speed
//!
//! ## Usage
//!
//! ```ignore
//! let mut bloom = BloomFilter::with_size(10_000);
//! bloom.insert("hello");
//! assert!(bloom.might_contain("hello")); // Definitely true
//! // might_contain("xyz") could return true (false positive)
//! // but if it returns false, "xyz" is definitely not present
//! ```

/// Number of hash functions (k=7 is optimal for 1% FPR with m/n ~= 10)
const K: usize = 7;

/// FNV-1a prime
const FNV_PRIME: u64 = 0x00000100_000001B3;

/// FNV-1a offset basis
const FNV_OFFSET: u64 = 0xcbf29ce4_84222325;

/// Bloom filter for fast set membership testing.
///
/// False positives are possible, false negatives are not.
#[derive(Debug, Clone)]
pub struct BloomFilter {
    /// Bit array stored as u64 chunks.
    bits: Vec<u64>,
    /// Total number of bits (m).
    num_bits: usize,
    /// Number of items inserted.
    count: usize,
}

impl BloomFilter {
    /// Create a new bloom filter with approximately `expected_items` capacity.
    ///
    /// The filter is sized for ~1% false positive rate at the expected capacity.
    #[must_use]
    pub fn with_capacity(expected_items: usize) -> Self {
        // For 1% FPR, we need m/n ~= 10 bits per item
        // Add some headroom
        let num_bits = (expected_items * 10).max(64);
        Self::with_size(num_bits)
    }

    /// Create a new bloom filter with the specified number of bits.
    #[must_use]
    pub fn with_size(num_bits: usize) -> Self {
        let num_bits = num_bits.max(64);
        let num_u64s = (num_bits + 63) / 64;
        Self {
            bits: vec![0u64; num_u64s],
            num_bits: num_u64s * 64,
            count: 0,
        }
    }

    /// Insert a string into the bloom filter.
    pub fn insert(&mut self, s: &str) {
        let (h1, h2) = self.hash_pair(s.as_bytes());

        for i in 0..K {
            let bit_idx = self.get_bit_index(h1, h2, i);
            self.set_bit(bit_idx);
        }
        self.count += 1;
    }

    /// Insert bytes into the bloom filter.
    pub fn insert_bytes(&mut self, bytes: &[u8]) {
        let (h1, h2) = self.hash_pair(bytes);

        for i in 0..K {
            let bit_idx = self.get_bit_index(h1, h2, i);
            self.set_bit(bit_idx);
        }
        self.count += 1;
    }

    /// Check if a string might be in the set.
    ///
    /// Returns `false` if definitely not present (no false negatives).
    /// Returns `true` if possibly present (may be false positive).
    #[must_use]
    pub fn might_contain(&self, s: &str) -> bool {
        self.might_contain_bytes(s.as_bytes())
    }

    /// Check if bytes might be in the set.
    #[must_use]
    pub fn might_contain_bytes(&self, bytes: &[u8]) -> bool {
        let (h1, h2) = self.hash_pair(bytes);

        for i in 0..K {
            let bit_idx = self.get_bit_index(h1, h2, i);
            if !self.get_bit(bit_idx) {
                return false;
            }
        }
        true
    }

    /// Get the number of items inserted.
    #[must_use]
    pub fn count(&self) -> usize {
        self.count
    }

    /// Get the number of bits in the filter.
    #[must_use]
    pub fn num_bits(&self) -> usize {
        self.num_bits
    }

    /// Estimate the false positive rate.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // FPR calculation doesn't need full precision
    pub fn estimated_fpr(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        // FPR ≈ (1 - e^(-k*n/m))^k
        let k = K as f64;
        let n = self.count as f64;
        let m = self.num_bits as f64;
        let exp = (-k * n / m).exp();
        // SAFETY: K is a const usize = 3, which fits in i32
        #[allow(clippy::cast_possible_wrap)]
        #[allow(clippy::cast_possible_truncation)] // K=3 fits in i32
        (1.0 - exp).powi(K as i32)
    }

    /// Clear all bits.
    pub fn clear(&mut self) {
        self.bits.fill(0);
        self.count = 0;
    }

    /// Compute two independent hashes using FNV-1a.
    ///
    /// Uses the Kirsch-Mitzenmacher technique: h_i = h1 + i*h2
    /// Optimized to compute both hashes in a single pass.
    #[inline]
    fn hash_pair(&self, bytes: &[u8]) -> (u64, u64) {
        // Compute both hashes in a single pass.
        // h1: Standard FNV-1a
        // h2: FNV-1a with different seed and rotated intermediate values
        let mut h1 = FNV_OFFSET;
        let mut h2 = FNV_OFFSET.rotate_left(17); // Different seed

        for &b in bytes {
            let b64 = u64::from(b);
            h1 ^= b64;
            h1 = h1.wrapping_mul(FNV_PRIME);
            h2 ^= b64;
            h2 = h2.wrapping_mul(FNV_PRIME);
        }

        // Final mixing for h2 to ensure independence
        h2 = h2.rotate_left(31);

        (h1, h2)
    }

    /// Get the bit index for the i-th hash function.
    #[inline]
    fn get_bit_index(&self, h1: u64, h2: u64, i: usize) -> usize {
        let combined = h1.wrapping_add((i as u64).wrapping_mul(h2));
        // Result is immediately bounded by modulo num_bits
        #[allow(clippy::cast_possible_truncation)]
        let combined_usize = combined as usize;
        combined_usize % self.num_bits
    }

    /// Set a bit in the filter.
    #[inline]
    fn set_bit(&mut self, bit_idx: usize) {
        let word_idx = bit_idx / 64;
        let bit_pos = bit_idx % 64;
        self.bits[word_idx] |= 1u64 << bit_pos;
    }

    /// Get a bit from the filter.
    #[inline]
    fn get_bit(&self, bit_idx: usize) -> bool {
        let word_idx = bit_idx / 64;
        let bit_pos = bit_idx % 64;
        (self.bits[word_idx] & (1u64 << bit_pos)) != 0
    }
}

impl Default for BloomFilter {
    fn default() -> Self {
        Self::with_capacity(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloom_insert_and_query() {
        let mut bloom = BloomFilter::with_capacity(100);

        bloom.insert("hello");
        bloom.insert("world");

        assert!(bloom.might_contain("hello"));
        assert!(bloom.might_contain("world"));
    }

    #[test]
    fn bloom_no_false_negatives() {
        let mut bloom = BloomFilter::with_capacity(1000);

        let items: Vec<String> = (0..1000).map(|i| format!("item_{i}")).collect();
        for item in &items {
            bloom.insert(item);
        }

        // All inserted items must be found
        for item in &items {
            assert!(bloom.might_contain(item), "False negative for '{}'", item);
        }
    }

    #[test]
    fn bloom_false_positive_rate() {
        let mut bloom = BloomFilter::with_capacity(1000);

        // Insert 1000 items
        for i in 0..1000 {
            bloom.insert(&format!("inserted_{i}"));
        }

        // Test 10000 items that were NOT inserted
        let mut false_positives = 0;
        for i in 0..10000 {
            if bloom.might_contain(&format!("not_inserted_{i}")) {
                false_positives += 1;
            }
        }

        // FPR should be around 1% (allow up to 5% for statistical variance)
        let fpr = f64::from(false_positives) / 10000.0;
        assert!(fpr < 0.05, "FPR too high: {:.2}%", fpr * 100.0);
    }

    #[test]
    fn bloom_clear() {
        let mut bloom = BloomFilter::with_capacity(100);
        bloom.insert("test");
        assert!(bloom.might_contain("test"));
        assert_eq!(bloom.count(), 1);

        bloom.clear();
        assert_eq!(bloom.count(), 0);
        // After clear, might_contain should return false for most items
        // (unless false positive)
    }

    #[test]
    fn bloom_estimated_fpr() {
        let mut bloom = BloomFilter::with_capacity(1000);

        // Insert items at expected capacity
        for i in 0..1000 {
            bloom.insert(&format!("item_{i}"));
        }

        // Estimated FPR should be around 1%
        let fpr = bloom.estimated_fpr();
        assert!(fpr < 0.02, "Estimated FPR too high: {:.2}%", fpr * 100.0);
    }

    #[test]
    fn bloom_with_size() {
        let bloom = BloomFilter::with_size(1024);
        assert_eq!(bloom.num_bits(), 1024);
    }
}
