//! Constants for Redis vector store.

/// Required Redis modules with minimum versions.
///
/// Redis Stack 6.2+ or Redis with these modules installed is required.
pub const REDIS_REQUIRED_MODULES: &[(&str, u32)] = &[
    ("search", 20600),      // RediSearch 2.6.0+
    ("searchlight", 20600), // RedisSearch Light 2.6.0+
];

/// Supported distance metrics for vector similarity search.
pub const REDIS_DISTANCE_METRICS: &[&str] = &["COSINE", "IP", "L2"];

/// Default separator for tag fields (comma).
pub const REDIS_TAG_SEPARATOR: &str = ",";

/// Default vector field name.
pub const DEFAULT_VECTOR_KEY: &str = "content_vector";

/// Default content field name.
pub const DEFAULT_CONTENT_KEY: &str = "content";

/// Default vector datatype (FLOAT32).
pub const DEFAULT_VECTOR_DATATYPE: &str = "FLOAT32";

/// Default distance metric (COSINE).
pub const DEFAULT_DISTANCE_METRIC: &str = "COSINE";

/// Default index algorithm (FLAT for exact search).
pub const DEFAULT_ALGORITHM: &str = "FLAT";
