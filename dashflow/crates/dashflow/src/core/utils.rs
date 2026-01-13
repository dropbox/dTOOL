//! Utility functions for `DashFlow` Core.
//!
//! This module provides common utility functions used across the codebase,
//! including SIMD-accelerated vector operations for embeddings and similarity search,
//! and runtime environment information for debugging and observability.

use futures::stream::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::OnceLock;

/// Calculate cosine similarity between two vectors.
///
/// # Performance
///
/// When the `simd` feature is enabled (default), this function uses SIMD-accelerated
/// operations via the `simsimd` crate, providing 10-30× speedup over naive implementations
/// on vectors of typical embedding sizes (384-1536 dimensions).
///
/// Without SIMD, falls back to a standard Rust implementation.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Cosine similarity in range [-1.0, 1.0], or 0.0 if either vector has zero norm.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::cosine_similarity;
///
/// let a = vec![1.0, 2.0, 3.0];
/// let b = vec![4.0, 5.0, 6.0];
/// let similarity = cosine_similarity(&a, &b);
/// assert!(similarity > 0.9); // Vectors point in similar direction
/// ```
#[must_use]
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(feature = "simd")]
    {
        cosine_similarity_simd(a, b)
    }
    #[cfg(not(feature = "simd"))]
    {
        cosine_similarity_fallback(a, b)
    }
}

/// SIMD-accelerated cosine similarity using simsimd.
///
/// Leverages AVX2/AVX-512 on `x86_64` and NEON on ARM for vectorized operations.
#[cfg(feature = "simd")]
#[inline]
fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;

    // simsimd expects equal-length vectors
    if a.len() != b.len() {
        return cosine_similarity_fallback(a, b);
    }

    // simsimd's cosine returns distance in [0, 2], where 0 = identical, 2 = opposite
    // Convert to similarity in [-1, 1]
    let distance = f32::cosine(a, b).unwrap_or(1.0); // 1.0 = orthogonal fallback
    let similarity = 1.0 - distance;
    similarity.clamp(-1.0, 1.0) as f32
}

/// Fallback cosine similarity implementation (no SIMD).
///
/// Used when SIMD is disabled or vectors have unequal length.
#[inline]
fn cosine_similarity_fallback(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    let dot_product: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot_product / (norm_a * norm_b)
    }
}

/// Calculate dot product between two vectors.
///
/// # Performance
///
/// SIMD-accelerated when `simd` feature is enabled.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Dot product of the two vectors.
#[must_use]
pub fn dot_product(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(feature = "simd")]
    {
        dot_product_simd(a, b)
    }
    #[cfg(not(feature = "simd"))]
    {
        dot_product_fallback(a, b)
    }
}

/// SIMD-accelerated dot product.
#[cfg(feature = "simd")]
#[inline]
fn dot_product_simd(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;

    if a.len() != b.len() {
        return dot_product_fallback(a, b);
    }

    f32::dot(a, b).unwrap_or(0.0) as f32
}

/// Fallback dot product implementation.
#[inline]
fn dot_product_fallback(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Calculate squared Euclidean distance between two vectors.
///
/// # Performance
///
/// SIMD-accelerated when `simd` feature is enabled.
///
/// # Arguments
///
/// * `a` - First vector
/// * `b` - Second vector
///
/// # Returns
///
/// Squared Euclidean distance.
#[must_use]
pub fn euclidean_distance_squared(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(feature = "simd")]
    {
        euclidean_distance_squared_simd(a, b)
    }
    #[cfg(not(feature = "simd"))]
    {
        euclidean_distance_squared_fallback(a, b)
    }
}

/// SIMD-accelerated squared Euclidean distance.
#[cfg(feature = "simd")]
#[inline]
fn euclidean_distance_squared_simd(a: &[f32], b: &[f32]) -> f32 {
    use simsimd::SpatialSimilarity;

    if a.len() != b.len() {
        return euclidean_distance_squared_fallback(a, b);
    }

    f32::sqeuclidean(a, b).unwrap_or(0.0) as f32
}

/// Fallback squared Euclidean distance implementation.
#[inline]
fn euclidean_distance_squared_fallback(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }

    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let diff = x - y;
            diff * diff
        })
        .sum()
}

// =============================================================================
// String Utilities
// =============================================================================

/// Convert a list of items to a comma-separated string.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.strings.comma_list()`.
///
/// # Arguments
///
/// * `items` - Slice of items that implement Display
///
/// # Returns
///
/// A comma-separated string representation of the items.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::comma_list;
///
/// assert_eq!(comma_list(&[1, 2, 3]), "1, 2, 3");
/// assert_eq!(comma_list(&["a", "b", "c"]), "a, b, c");
/// assert_eq!(comma_list::<i32>(&[]), "");
/// ```
pub fn comma_list<T: std::fmt::Display>(items: &[T]) -> String {
    items
        .iter()
        .map(std::string::ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// Convert a value to a string representation, handling nested structures.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.strings.stringify_value()`.
///
/// # Arguments
///
/// * `val` - Any value that implements Display
///
/// # Returns
///
/// String representation of the value.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::stringify_value;
///
/// assert_eq!(stringify_value(&"hello"), "hello");
/// assert_eq!(stringify_value(&42), "42");
/// assert_eq!(stringify_value(&3.14), "3.14");
/// ```
pub fn stringify_value<T: std::fmt::Display>(val: &T) -> String {
    val.to_string()
}

/// Sanitize text by removing NUL bytes that are incompatible with `PostgreSQL`.
///
/// `PostgreSQL` text fields cannot contain NUL (0x00) bytes, which can cause
/// errors when inserting documents. This function removes such characters.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.strings.sanitize_for_postgres()`.
///
/// # Arguments
///
/// * `text` - The text to sanitize
///
/// # Returns
///
/// The sanitized text with NUL bytes removed.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::sanitize_for_postgres;
///
/// assert_eq!(sanitize_for_postgres("Hello\x00world"), "Helloworld");
/// assert_eq!(sanitize_for_postgres("no nulls here"), "no nulls here");
/// ```
#[must_use]
pub fn sanitize_for_postgres(text: &str) -> String {
    text.replace('\0', "")
}

/// Sanitize text by replacing NUL bytes with a replacement string.
///
/// `PostgreSQL` text fields cannot contain NUL (0x00) bytes, which can cause
/// errors when inserting documents. This function replaces such characters.
///
/// # Arguments
///
/// * `text` - The text to sanitize
/// * `replacement` - String to replace NUL bytes with
///
/// # Returns
///
/// The sanitized text with NUL bytes replaced.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::sanitize_for_postgres_with;
///
/// assert_eq!(sanitize_for_postgres_with("Hello\x00world", " "), "Hello world");
/// assert_eq!(sanitize_for_postgres_with("Hello\x00world", "-"), "Hello-world");
/// ```
#[must_use]
pub fn sanitize_for_postgres_with(text: &str, replacement: &str) -> String {
    text.replace('\0', replacement)
}

// =============================================================================
// Runtime Environment
// =============================================================================

/// Runtime environment information.
///
/// Contains information about the `DashFlow` Rust runtime environment,
/// useful for debugging, logging, and observability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeEnvironment {
    /// Library version (e.g., "1.7.0")
    pub library_version: String,
    /// Library name ("dashflow::core")
    pub library: String,
    /// Platform (e.g., "linux", "macos", "windows")
    pub platform: String,
    /// Runtime language ("rust")
    pub runtime: String,
    /// Rust compiler version (e.g., "1.81.0")
    pub runtime_version: String,
}

impl RuntimeEnvironment {
    /// Create a new `RuntimeEnvironment` with current system information.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::utils::RuntimeEnvironment;
    ///
    /// let env = RuntimeEnvironment::new();
    /// assert_eq!(env.library, "dashflow::core");
    /// assert_eq!(env.runtime, "rust");
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {
            library_version: env!("CARGO_PKG_VERSION").to_string(),
            library: "dashflow::core".to_string(),
            platform: std::env::consts::OS.to_string(),
            runtime: "rust".to_string(),
            runtime_version: get_rust_version(),
        }
    }

    /// Convert to a `HashMap` for compatibility with Python DashFlow format.
    ///
    /// # Examples
    ///
    /// ```
    /// use dashflow::core::utils::RuntimeEnvironment;
    ///
    /// let env = RuntimeEnvironment::new();
    /// let map = env.to_map();
    /// assert_eq!(map["library"], "dashflow::core");
    /// assert_eq!(map["runtime"], "rust");
    /// ```
    #[must_use]
    pub fn to_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("library_version".to_string(), self.library_version.clone());
        map.insert("library".to_string(), self.library.clone());
        map.insert("platform".to_string(), self.platform.clone());
        map.insert("runtime".to_string(), self.runtime.clone());
        map.insert("runtime_version".to_string(), self.runtime_version.clone());
        map
    }
}

impl Default for RuntimeEnvironment {
    fn default() -> Self {
        Self::new()
    }
}

/// Get the Rust compiler version used to build this binary.
///
/// Returns the version string from `rustc --version`, cached globally.
fn get_rust_version() -> String {
    static RUST_VERSION: OnceLock<String> = OnceLock::new();
    RUST_VERSION
        .get_or_init(|| {
            // Try to get rustc version at runtime
            if let Ok(output) = std::process::Command::new("rustc")
                .arg("--version")
                .output()
            {
                if let Ok(version) = String::from_utf8(output.stdout) {
                    // Extract just the version number from "rustc 1.81.0 (eeb90cda1 2024-09-04)"
                    if let Some(v) = version.split_whitespace().nth(1) {
                        return v.to_string();
                    }
                }
            }
            // Fallback to RUSTC_VERSION environment variable set at compile time
            option_env!("RUSTC_VERSION")
                .unwrap_or("unknown")
                .to_string()
        })
        .clone()
}

/// Get information about the `DashFlow` runtime environment.
///
/// Returns a cached `RuntimeEnvironment` with library version, platform,
/// and Rust compiler information. This is the Rust equivalent of Python's
/// `dashflow_core.env.get_runtime_environment()`.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::get_runtime_environment;
///
/// let env = get_runtime_environment();
/// println!("Running DashFlow v{} on {}", env.library_version, env.platform);
/// ```
pub fn get_runtime_environment() -> RuntimeEnvironment {
    static RUNTIME_ENV: OnceLock<RuntimeEnvironment> = OnceLock::new();
    RUNTIME_ENV.get_or_init(RuntimeEnvironment::new).clone()
}

// =============================================================================
// ID Generation Utilities
// =============================================================================

/// Prefix for auto-generated IDs.
///
/// Auto-generated UUIDs are prefixed with `"lc_"` for compatibility
/// with Python DashFlow's `LC_AUTO_PREFIX`.
pub const LC_AUTO_PREFIX: &str = "lc_";

/// Ensure the ID is a valid string, generating a new UUID if not provided.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.utils.ensure_id()`.
///
/// Auto-generated UUIDs are prefixed by `'lc_'` for compatibility
/// with Python DashFlow.
///
/// # Arguments
///
/// * `id_val` - Optional string ID value to validate
///
/// # Returns
///
/// A string ID, either the provided value or a newly generated UUID4 with "lc_" prefix.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::ensure_id;
///
/// // With provided ID
/// let id = ensure_id(Some("my-custom-id".to_string()));
/// assert_eq!(id, "my-custom-id");
///
/// // Without provided ID (generates UUID with lc_ prefix)
/// let id = ensure_id(None);
/// assert!(id.starts_with("lc_"));
/// assert_eq!(id.len(), 39); // "lc_" (3) + UUID (36)
/// ```
#[must_use]
pub fn ensure_id(id_val: Option<String>) -> String {
    id_val.unwrap_or_else(|| format!("{}{}", LC_AUTO_PREFIX, uuid::Uuid::new_v4()))
}

// =============================================================================
// Iterator Utilities
// =============================================================================

/// Batch an iterator into chunks of a specified size.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.iter.batch_iterate()`.
///
/// Takes an iterator and yields batches (vectors) of items. The last batch may be
/// smaller than the requested size if there aren't enough items remaining.
///
/// # Arguments
///
/// * `size` - The size of each batch
/// * `iterable` - The iterator to batch
///
/// # Returns
///
/// An iterator that yields `Vec<T>` batches of the specified size.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::batch_iterate;
///
/// let items = vec![1, 2, 3, 4, 5, 6, 7];
/// let batches: Vec<Vec<i32>> = batch_iterate(3, items).collect();
/// assert_eq!(batches, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7]]);
/// ```
///
/// ```
/// use dashflow::core::utils::batch_iterate;
///
/// // Batch strings
/// let words = vec!["hello", "world", "foo", "bar", "baz"];
/// let batches: Vec<Vec<&str>> = batch_iterate(2, words).collect();
/// assert_eq!(batches.len(), 3); // [["hello", "world"], ["foo", "bar"], ["baz"]]
/// ```
pub fn batch_iterate<T>(
    size: usize,
    iterable: impl IntoIterator<Item = T>,
) -> impl Iterator<Item = Vec<T>> {
    let mut iter = iterable.into_iter();
    std::iter::from_fn(move || {
        let mut batch = Vec::with_capacity(size);
        for _ in 0..size {
            match iter.next() {
                Some(item) => batch.push(item),
                None => break,
            }
        }
        if batch.is_empty() {
            None
        } else {
            Some(batch)
        }
    })
}

/// Batch async stream into chunks of a specified size.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.aiter.abatch_iterate()`.
///
/// This function takes an async stream and yields batches (vectors) of items
/// from that stream. Each batch contains up to `size` items. The final batch
/// may contain fewer items if the stream doesn't divide evenly.
///
/// This is commonly used for:
/// - Batching async API requests (e.g., batch embedding API calls)
/// - Processing async streams in chunks
/// - Rate limiting async operations
/// - Memory-efficient processing of large async data streams
///
/// # Arguments
///
/// * `size` - The maximum number of items in each batch
/// * `stream` - The async stream to batch (any type implementing `Stream`)
///
/// # Returns
///
/// Returns a pinned boxed stream that yields `Vec<T>` batches.
///
/// # Examples
///
/// ```rust,ignore
/// use dashflow::core::utils::abatch_iterate;
/// use futures::stream::{self, StreamExt};
///
/// # async fn example() {
/// // Batch a stream of numbers
/// let items = stream::iter(vec![1, 2, 3, 4, 5]);
/// let mut batches = abatch_iterate(2, items);
///
/// assert_eq!(batches.next().await, Some(vec![1, 2]));
/// assert_eq!(batches.next().await, Some(vec![3, 4]));
/// assert_eq!(batches.next().await, Some(vec![5]));
/// assert_eq!(batches.next().await, None);
/// # }
/// ```
///
/// ```rust,ignore
/// use dashflow::core::utils::abatch_iterate;
/// use futures::stream::{self, StreamExt};
///
/// # async fn example() {
/// // Process async stream in batches
/// let stream = stream::iter(1..=10);
/// let batches: Vec<Vec<i32>> = abatch_iterate(3, stream).collect().await;
/// assert_eq!(batches, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7, 8, 9], vec![10]]);
/// # }
/// ```
///
/// ```rust,ignore
/// use dashflow::core::utils::abatch_iterate;
/// use futures::stream::{self, StreamExt};
///
/// # async fn example() {
/// // Batch strings
/// let words = stream::iter(vec!["hello", "world", "foo", "bar", "baz"]);
/// let mut batches = abatch_iterate(2, words);
///
/// assert_eq!(batches.next().await, Some(vec!["hello", "world"]));
/// assert_eq!(batches.next().await, Some(vec!["foo", "bar"]));
/// assert_eq!(batches.next().await, Some(vec!["baz"]));
/// # }
/// ```
pub fn abatch_iterate<T, S>(size: usize, stream: S) -> Pin<Box<dyn Stream<Item = Vec<T>> + Send>>
where
    T: Send + 'static,
    S: Stream<Item = T> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        let mut batch = Vec::with_capacity(size);
        let mut stream = Box::pin(stream);

        while let Some(element) = stream.next().await {
            batch.push(element);

            if batch.len() >= size {
                yield batch;
                batch = Vec::with_capacity(size);
            }
        }

        // Yield remaining items if any
        if !batch.is_empty() {
            yield batch;
        }
    })
}

// =============================================================================
// Environment Variable Utilities
// =============================================================================

/// Check if an environment variable is set and has a truthy value.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.env.env_var_is_set()`.
///
/// An environment variable is considered "set" if it exists and its value is not
/// one of the following falsy values: "", "0", "false", "False".
///
/// # Arguments
///
/// * `env_var` - The name of the environment variable
///
/// # Returns
///
/// `true` if the environment variable is set and has a truthy value, `false` otherwise.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::env_var_is_set;
///
/// // Assuming RUST_LOG is set to "debug"
/// if env_var_is_set("RUST_LOG") {
///     println!("Logging is enabled");
/// }
/// ```
#[must_use]
pub fn env_var_is_set(env_var: &str) -> bool {
    if let Ok(value) = std::env::var(env_var) {
        !matches!(value.as_str(), "" | "0" | "false" | "False")
    } else {
        false
    }
}

/// Get a value from an environment variable with optional default.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.env.get_from_env()`.
///
/// # Arguments
///
/// * `key` - The key name (used in error messages)
/// * `env_key` - The environment variable name to look up
/// * `default` - Optional default value if the environment variable is not set
///
/// # Returns
///
/// The environment variable value, or the default if provided.
///
/// # Errors
///
/// Returns an error if the environment variable is not set and no default is provided.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::get_from_env;
///
/// // With default value
/// let api_url = get_from_env("api_url", "API_URL", Some("http://localhost:8000")).unwrap();
///
/// // Without default (will error if OPENAI_API_KEY is not set)
/// let api_key = get_from_env("openai_api_key", "OPENAI_API_KEY", None);
/// match api_key {
///     Ok(key) => println!("API key found"),
///     Err(e) => println!("Error: {}", e),
/// }
/// ```
pub fn get_from_env(key: &str, env_key: &str, default: Option<&str>) -> Result<String, String> {
    if let Ok(value) = std::env::var(env_key) {
        return Ok(value);
    }
    if let Some(default_value) = default {
        return Ok(default_value.to_string());
    }
    Err(format!(
        "Did not find {key}, please add an environment variable `{env_key}` which contains it, or pass `{key}` as a named parameter."
    ))
}

/// Get a value from a dictionary or environment variable.
///
/// This is the Rust equivalent of Python's `dashflow_core.utils.env.get_from_dict_or_env()`.
///
/// Tries to get a value from a dictionary first, then falls back to an environment variable.
/// Supports multiple keys to try in order.
///
/// # Arguments
///
/// * `data` - `HashMap` to look up the key in
/// * `keys` - One or more keys to try in order (tries each until a non-empty value is found)
/// * `env_key` - Environment variable to look up if keys are not in the dictionary
/// * `default` - Optional default value if neither dictionary nor environment has the value
///
/// # Returns
///
/// The value from the dictionary, environment variable, or default.
///
/// # Errors
///
/// Returns an error if the value is not found in the dictionary or environment and no default is provided.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::get_from_dict_or_env;
/// use std::collections::HashMap;
///
/// let mut config = HashMap::new();
/// config.insert("api_key".to_string(), "sk-12345".to_string());
///
/// // Get from dictionary
/// let key = get_from_dict_or_env(&config, &["api_key"], "OPENAI_API_KEY", None).unwrap();
/// assert_eq!(key, "sk-12345");
///
/// // Try multiple keys
/// let key = get_from_dict_or_env(&config, &["openai_api_key", "api_key"], "OPENAI_API_KEY", None).unwrap();
/// assert_eq!(key, "sk-12345");
///
/// // Fall back to environment variable
/// let config2 = HashMap::new();
/// // If OPENAI_API_KEY is set in environment, this will succeed
/// let key = get_from_dict_or_env(&config2, &["api_key"], "OPENAI_API_KEY", Some("default"));
/// ```
pub fn get_from_dict_or_env(
    data: &HashMap<String, String>,
    keys: &[&str],
    env_key: &str,
    default: Option<&str>,
) -> Result<String, String> {
    // Try each key in order
    for key in keys {
        if let Some(value) = data.get(*key) {
            if !value.is_empty() {
                return Ok(value.clone());
            }
        }
    }

    // Fall back to environment variable
    let key_for_err = keys.first().unwrap_or(&"");
    get_from_env(key_for_err, env_key, default)
}

// =============================================================================
// Log Safety Utilities (M-235)
// =============================================================================

/// Maximum length for log-safe strings (prevents log flooding).
pub const LOG_SAFE_MAX_LENGTH: usize = 1000;

/// Sanitize a string for safe logging.
///
/// Log injection attacks occur when untrusted input containing newlines or control
/// characters is written to logs, potentially:
/// - Creating fake log entries
/// - Breaking log aggregation/parsing
/// - Hiding malicious activity
///
/// This function:
/// 1. Escapes newlines (`\n` → `\\n`, `\r` → `\\r`)
/// 2. Escapes control characters (ASCII 0-31 except tab → `\xNN`)
/// 3. Truncates to `max_len` with `...[truncated]` suffix
///
/// # Arguments
///
/// * `input` - The untrusted string to sanitize
/// * `max_len` - Maximum length before truncation (use [`LOG_SAFE_MAX_LENGTH`] as default)
///
/// # Returns
///
/// A sanitized string safe for logging.
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::{sanitize_for_log, LOG_SAFE_MAX_LENGTH};
///
/// // Escapes newlines
/// let malicious = "normal\nfake log entry\n[ERROR] attack";
/// let safe = sanitize_for_log(malicious, LOG_SAFE_MAX_LENGTH);
/// assert_eq!(safe, "normal\\nfake log entry\\n[ERROR] attack");
///
/// // Truncates long strings
/// let long = "a".repeat(2000);
/// let safe = sanitize_for_log(&long, 100);
/// assert!(safe.ends_with("...[truncated]"));
/// assert!(safe.len() <= 114); // 100 + "...[truncated]".len()
/// ```
#[must_use]
pub fn sanitize_for_log(input: &str, max_len: usize) -> String {
    // Fast path: check if sanitization is needed
    let needs_escape = input
        .bytes()
        .any(|b| b == b'\n' || b == b'\r' || (b < 32 && b != b'\t'));
    let needs_truncate = input.len() > max_len;

    if !needs_escape && !needs_truncate {
        return input.to_string();
    }

    let mut result = String::with_capacity(input.len().min(max_len + 32));
    let mut char_count = 0;

    for ch in input.chars() {
        if char_count >= max_len {
            result.push_str("...[truncated]");
            break;
        }

        match ch {
            '\n' => {
                result.push_str("\\n");
                char_count += 2;
            }
            '\r' => {
                result.push_str("\\r");
                char_count += 2;
            }
            '\t' => {
                result.push('\t'); // Tabs are safe
                char_count += 1;
            }
            c if c.is_control() => {
                // Escape other control characters as \xNN
                let escaped = format!("\\x{:02x}", c as u32);
                result.push_str(&escaped);
                char_count += escaped.len();
            }
            c => {
                result.push(c);
                char_count += 1;
            }
        }
    }

    result
}

/// Sanitize a string for logging with the default maximum length.
///
/// Convenience wrapper around [`sanitize_for_log`] using [`LOG_SAFE_MAX_LENGTH`].
///
/// # Examples
///
/// ```
/// use dashflow::core::utils::sanitize_for_log_default;
///
/// let user_input = "search query\ninjected log entry";
/// let safe = sanitize_for_log_default(user_input);
/// assert_eq!(safe, "search query\\ninjected log entry");
/// ```
#[must_use]
pub fn sanitize_for_log_default(input: &str) -> String {
    sanitize_for_log(input, LOG_SAFE_MAX_LENGTH)
}

#[cfg(test)]
mod tests {
    use super::{
        comma_list, cosine_similarity, dot_product, ensure_id, euclidean_distance_squared,
        get_runtime_environment, sanitize_for_log, sanitize_for_log_default, sanitize_for_postgres,
        sanitize_for_postgres_with, stringify_value, LOG_SAFE_MAX_LENGTH,
    };
    use crate::test_prelude::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![-1.0, -2.0, -3.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6, "sim={sim}");
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity_zero_norm() {
        let a = vec![0.0, 0.0];
        let b = vec![1.0, 2.0];
        let sim = cosine_similarity(&a, &b);
        assert_eq!(sim, 0.0);
    }

    #[test]
    fn test_dot_product() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 5.0, 6.0];
        let dot = dot_product(&a, &b);
        assert!((dot - 32.0).abs() < 1e-6);
    }

    #[test]
    fn test_euclidean_distance_squared() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![4.0, 6.0, 8.0];
        let dist_sq = euclidean_distance_squared(&a, &b);
        // (4-1)^2 + (6-2)^2 + (8-3)^2 = 9 + 16 + 25 = 50
        assert!((dist_sq - 50.0).abs() < 1e-6);
    }

    #[test]
    fn test_large_vectors() {
        // Test with BERT-like embedding dimension (768)
        let dim = 768;
        let a: Vec<f32> = (0..dim).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32).cos()).collect();

        let sim = cosine_similarity(&a, &b);
        assert!((-1.0..=1.0).contains(&sim));

        let dot = dot_product(&a, &b);
        assert!(dot.is_finite());

        let dist_sq = euclidean_distance_squared(&a, &b);
        assert!(dist_sq >= 0.0);
    }

    // String utilities tests
    #[test]
    fn test_comma_list() {
        assert_eq!(comma_list(&[1, 2, 3]), "1, 2, 3");
        assert_eq!(comma_list(&["a", "b", "c"]), "a, b, c");
        let empty: &[i32] = &[];
        assert_eq!(comma_list(empty), "");
        assert_eq!(comma_list(&["single"]), "single");
    }

    #[test]
    fn test_stringify_value() {
        assert_eq!(stringify_value(&"hello"), "hello");
        assert_eq!(stringify_value(&42), "42");
        assert_eq!(stringify_value(&true), "true");
        assert_eq!(stringify_value(&3.5), "3.5");
    }

    #[test]
    fn test_sanitize_for_postgres() {
        assert_eq!(sanitize_for_postgres("Hello\x00world"), "Helloworld");
        assert_eq!(
            sanitize_for_postgres_with("Hello\x00world", " "),
            "Hello world"
        );
        assert_eq!(sanitize_for_postgres("no nulls here"), "no nulls here");
    }

    // Log sanitization tests (M-235)

    #[test]
    fn test_sanitize_for_log_newlines() {
        // Newlines can create fake log entries
        let malicious = "normal\nfake log entry\n[ERROR] attack";
        let safe = sanitize_for_log(malicious, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, "normal\\nfake log entry\\n[ERROR] attack");
        assert!(!safe.contains('\n'));
    }

    #[test]
    fn test_sanitize_for_log_carriage_return() {
        let input = "line1\r\nline2";
        let safe = sanitize_for_log(input, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, "line1\\r\\nline2");
        assert!(!safe.contains('\r'));
        assert!(!safe.contains('\n'));
    }

    #[test]
    fn test_sanitize_for_log_control_chars() {
        // Control characters (except tab) should be escaped
        let input = "before\x00null\x07bell\x1bescape";
        let safe = sanitize_for_log(input, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, "before\\x00null\\x07bell\\x1bescape");
        assert!(!safe.chars().any(|c| c.is_control() && c != '\t'));
    }

    #[test]
    fn test_sanitize_for_log_preserves_tab() {
        // Tabs are safe and commonly used in logs
        let input = "col1\tcol2\tcol3";
        let safe = sanitize_for_log(input, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, "col1\tcol2\tcol3");
    }

    #[test]
    fn test_sanitize_for_log_truncation() {
        let long = "a".repeat(2000);
        let safe = sanitize_for_log(&long, 100);
        assert!(safe.ends_with("...[truncated]"));
        assert!(safe.len() <= 114); // 100 chars + "...[truncated]"
    }

    #[test]
    fn test_sanitize_for_log_no_truncation_at_limit() {
        let exact = "a".repeat(100);
        let safe = sanitize_for_log(&exact, 100);
        assert_eq!(safe, exact);
        assert!(!safe.contains("truncated"));
    }

    #[test]
    fn test_sanitize_for_log_fast_path_no_alloc() {
        // Clean input should return without escaping
        let clean = "this is a clean log message with spaces and punctuation!";
        let safe = sanitize_for_log(clean, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, clean);
    }

    #[test]
    fn test_sanitize_for_log_default() {
        let input = "query\ninjected";
        let safe = sanitize_for_log_default(input);
        assert_eq!(safe, "query\\ninjected");
    }

    #[test]
    fn test_sanitize_for_log_unicode() {
        // Unicode should pass through
        let input = "日本語 🚀 émojis";
        let safe = sanitize_for_log(input, LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, input);
    }

    #[test]
    fn test_sanitize_for_log_empty() {
        let safe = sanitize_for_log("", LOG_SAFE_MAX_LENGTH);
        assert_eq!(safe, "");
    }

    #[test]
    fn test_runtime_environment_creation() {
        let env = RuntimeEnvironment::new();
        assert_eq!(env.library, "dashflow::core");
        assert_eq!(env.runtime, "rust");
        assert!(!env.library_version.is_empty());
        assert!(!env.platform.is_empty());
        assert!(!env.runtime_version.is_empty());
    }

    #[test]
    fn test_runtime_environment_to_map() {
        let env = RuntimeEnvironment::new();
        let map = env.to_map();
        assert_eq!(map["library"], "dashflow::core");
        assert_eq!(map["runtime"], "rust");
        assert!(map.contains_key("library_version"));
        assert!(map.contains_key("platform"));
        assert!(map.contains_key("runtime_version"));
    }

    #[test]
    fn test_get_runtime_environment_cached() {
        let env1 = get_runtime_environment();
        let env2 = get_runtime_environment();
        assert_eq!(env1, env2);
    }

    #[test]
    fn test_runtime_environment_default() {
        let env = RuntimeEnvironment::default();
        assert_eq!(env.library, "dashflow::core");
        assert_eq!(env.runtime, "rust");
    }

    #[test]
    fn test_runtime_environment_platform() {
        let env = RuntimeEnvironment::new();
        // Platform should be one of: linux, macos, windows, etc.
        let valid_platforms = ["linux", "macos", "windows", "freebsd", "netbsd", "openbsd"];
        assert!(
            valid_platforms.contains(&env.platform.as_str()),
            "Unexpected platform: {}",
            env.platform
        );
    }

    #[test]
    fn test_runtime_environment_serialization() {
        let env = RuntimeEnvironment::new();
        let json = serde_json::to_string(&env).unwrap();
        let deserialized: RuntimeEnvironment = serde_json::from_str(&json).unwrap();
        assert_eq!(env, deserialized);
    }

    // Iterator utilities tests

    #[test]
    fn test_batch_iterate_even_batches() {
        let items = vec![1, 2, 3, 4, 5, 6];
        let batches: Vec<Vec<i32>> = batch_iterate(2, items).collect();
        assert_eq!(batches, vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
    }

    #[test]
    fn test_batch_iterate_uneven_batches() {
        let items = vec![1, 2, 3, 4, 5, 6, 7];
        let batches: Vec<Vec<i32>> = batch_iterate(3, items).collect();
        assert_eq!(batches, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7]]);
    }

    #[test]
    fn test_batch_iterate_empty() {
        let items: Vec<i32> = vec![];
        let batches: Vec<Vec<i32>> = batch_iterate(3, items).collect();
        assert_eq!(batches, Vec::<Vec<i32>>::new());
    }

    #[test]
    fn test_batch_iterate_single_batch() {
        let items = vec![1, 2, 3];
        let batches: Vec<Vec<i32>> = batch_iterate(5, items).collect();
        assert_eq!(batches, vec![vec![1, 2, 3]]);
    }

    #[test]
    fn test_batch_iterate_size_one() {
        let items = vec![1, 2, 3];
        let batches: Vec<Vec<i32>> = batch_iterate(1, items).collect();
        assert_eq!(batches, vec![vec![1], vec![2], vec![3]]);
    }

    #[test]
    fn test_batch_iterate_strings() {
        let words = vec!["hello", "world", "foo", "bar", "baz"];
        let batches: Vec<Vec<&str>> = batch_iterate(2, words).collect();
        assert_eq!(
            batches,
            vec![vec!["hello", "world"], vec!["foo", "bar"], vec!["baz"]]
        );
    }

    // Async iterator utilities tests

    #[tokio::test]
    async fn test_abatch_iterate_even_batches() {
        use futures::stream::{self, StreamExt};

        let items = stream::iter(vec![1, 2, 3, 4, 5, 6]);
        let batches: Vec<Vec<i32>> = abatch_iterate(2, items).collect().await;
        assert_eq!(batches, vec![vec![1, 2], vec![3, 4], vec![5, 6]]);
    }

    #[tokio::test]
    async fn test_abatch_iterate_uneven_batches() {
        use futures::stream::{self, StreamExt};

        let items = stream::iter(vec![1, 2, 3, 4, 5, 6, 7]);
        let batches: Vec<Vec<i32>> = abatch_iterate(3, items).collect().await;
        assert_eq!(batches, vec![vec![1, 2, 3], vec![4, 5, 6], vec![7]]);
    }

    #[tokio::test]
    async fn test_abatch_iterate_empty() {
        use futures::stream::{self, StreamExt};

        let items: Vec<i32> = vec![];
        let batches: Vec<Vec<i32>> = abatch_iterate(3, stream::iter(items)).collect().await;
        assert_eq!(batches, Vec::<Vec<i32>>::new());
    }

    #[tokio::test]
    async fn test_abatch_iterate_single_batch() {
        use futures::stream::{self, StreamExt};

        let items = stream::iter(vec![1, 2, 3]);
        let batches: Vec<Vec<i32>> = abatch_iterate(5, items).collect().await;
        assert_eq!(batches, vec![vec![1, 2, 3]]);
    }

    #[tokio::test]
    async fn test_abatch_iterate_size_one() {
        use futures::stream::{self, StreamExt};

        let items = stream::iter(vec![1, 2, 3]);
        let batches: Vec<Vec<i32>> = abatch_iterate(1, items).collect().await;
        assert_eq!(batches, vec![vec![1], vec![2], vec![3]]);
    }

    #[tokio::test]
    async fn test_abatch_iterate_strings() {
        use futures::stream::{self, StreamExt};

        let words = stream::iter(vec!["hello", "world", "foo", "bar", "baz"]);
        let batches: Vec<Vec<&str>> = abatch_iterate(2, words).collect().await;
        assert_eq!(
            batches,
            vec![vec!["hello", "world"], vec!["foo", "bar"], vec!["baz"]]
        );
    }

    #[tokio::test]
    async fn test_abatch_iterate_large_stream() {
        use futures::stream::{self, StreamExt};

        // Test with larger stream to verify efficiency
        let items = stream::iter(1..=100);
        let batches: Vec<Vec<i32>> = abatch_iterate(10, items).collect().await;
        assert_eq!(batches.len(), 10);
        assert_eq!(batches[0], vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
        assert_eq!(batches[9], vec![91, 92, 93, 94, 95, 96, 97, 98, 99, 100]);
    }

    // ID generation utilities tests

    #[test]
    fn test_ensure_id_with_provided_id() {
        let id = ensure_id(Some("my-custom-id".to_string()));
        assert_eq!(id, "my-custom-id");
    }

    #[test]
    fn test_ensure_id_without_id_generates_uuid() {
        let id = ensure_id(None);
        assert!(id.starts_with("lc_"));
        assert_eq!(id.len(), 39); // "lc_" (3 chars) + UUID (36 chars including hyphens)
    }

    #[test]
    fn test_ensure_id_generates_unique_ids() {
        let id1 = ensure_id(None);
        let id2 = ensure_id(None);
        assert_ne!(id1, id2); // Should generate different UUIDs
        assert!(id1.starts_with("lc_"));
        assert!(id2.starts_with("lc_"));
    }

    #[test]
    fn test_lc_auto_prefix_constant() {
        assert_eq!(LC_AUTO_PREFIX, "lc_");
    }

    // Environment variable utilities tests

    #[test]
    fn test_env_var_is_set_true() {
        std::env::set_var("TEST_VAR_TRUE", "1");
        assert!(env_var_is_set("TEST_VAR_TRUE"));
        std::env::remove_var("TEST_VAR_TRUE");
    }

    #[test]
    fn test_env_var_is_set_false() {
        std::env::set_var("TEST_VAR_FALSE", "0");
        assert!(!env_var_is_set("TEST_VAR_FALSE"));
        std::env::remove_var("TEST_VAR_FALSE");

        std::env::set_var("TEST_VAR_FALSE2", "false");
        assert!(!env_var_is_set("TEST_VAR_FALSE2"));
        std::env::remove_var("TEST_VAR_FALSE2");

        std::env::set_var("TEST_VAR_FALSE3", "False");
        assert!(!env_var_is_set("TEST_VAR_FALSE3"));
        std::env::remove_var("TEST_VAR_FALSE3");

        std::env::set_var("TEST_VAR_EMPTY", "");
        assert!(!env_var_is_set("TEST_VAR_EMPTY"));
        std::env::remove_var("TEST_VAR_EMPTY");
    }

    #[test]
    fn test_env_var_is_set_not_set() {
        std::env::remove_var("TEST_VAR_NOT_SET");
        assert!(!env_var_is_set("TEST_VAR_NOT_SET"));
    }

    #[test]
    fn test_get_from_env_exists() {
        std::env::set_var("TEST_ENV_KEY", "test_value");
        let result = get_from_env("test_key", "TEST_ENV_KEY", None);
        assert_eq!(result.unwrap(), "test_value");
        std::env::remove_var("TEST_ENV_KEY");
    }

    #[test]
    fn test_get_from_env_with_default() {
        std::env::remove_var("TEST_ENV_KEY_MISSING");
        let result = get_from_env("test_key", "TEST_ENV_KEY_MISSING", Some("default_value"));
        assert_eq!(result.unwrap(), "default_value");
    }

    #[test]
    fn test_get_from_env_missing_no_default() {
        std::env::remove_var("TEST_ENV_KEY_MISSING2");
        let result = get_from_env("test_key", "TEST_ENV_KEY_MISSING2", None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Did not find test_key"));
        assert!(err.contains("TEST_ENV_KEY_MISSING2"));
    }

    #[test]
    fn test_get_from_dict_or_env_from_dict() {
        let mut data = HashMap::new();
        data.insert("api_key".to_string(), "dict_value".to_string());

        std::env::remove_var("TEST_API_KEY");
        let result = get_from_dict_or_env(&data, &["api_key"], "TEST_API_KEY", None);
        assert_eq!(result.unwrap(), "dict_value");
    }

    #[test]
    fn test_get_from_dict_or_env_from_env() {
        let data = HashMap::new();
        std::env::set_var("TEST_API_KEY2", "env_value");
        let result = get_from_dict_or_env(&data, &["api_key"], "TEST_API_KEY2", None);
        assert_eq!(result.unwrap(), "env_value");
        std::env::remove_var("TEST_API_KEY2");
    }

    #[test]
    fn test_get_from_dict_or_env_multiple_keys() {
        let mut data = HashMap::new();
        data.insert("second_key".to_string(), "found_value".to_string());

        std::env::remove_var("TEST_API_KEY3");
        let result =
            get_from_dict_or_env(&data, &["first_key", "second_key"], "TEST_API_KEY3", None);
        assert_eq!(result.unwrap(), "found_value");
    }

    #[test]
    fn test_get_from_dict_or_env_with_default() {
        let data = HashMap::new();
        std::env::remove_var("TEST_API_KEY4");
        let result = get_from_dict_or_env(&data, &["api_key"], "TEST_API_KEY4", Some("default"));
        assert_eq!(result.unwrap(), "default");
    }

    #[test]
    fn test_get_from_dict_or_env_missing_no_default() {
        let data = HashMap::new();
        std::env::remove_var("TEST_API_KEY5");
        let result = get_from_dict_or_env(&data, &["api_key"], "TEST_API_KEY5", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_from_dict_or_env_empty_value_skipped() {
        let mut data = HashMap::new();
        data.insert("api_key".to_string(), "".to_string()); // Empty value should be skipped

        std::env::set_var("TEST_API_KEY6", "env_value");
        let result = get_from_dict_or_env(&data, &["api_key"], "TEST_API_KEY6", None);
        assert_eq!(result.unwrap(), "env_value"); // Should fall back to env
        std::env::remove_var("TEST_API_KEY6");
    }
}
