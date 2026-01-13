// Allow clippy warnings for Redis schema
// - float_cmp: Schema config comparisons use exact float values
#![allow(clippy::float_cmp)]

//! Redis index schema types.
//!
//! This module defines types for configuring Redis vector indexes including:
//! - Distance metrics (Cosine, L2, Inner Product)
//! - Vector field configurations (FLAT, HNSW)
//! - Metadata field types (Text, Tag, Numeric)
//! - Complete index schemas

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Distance metric for vector similarity calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DistanceMetric {
    /// L2 Euclidean distance
    #[serde(rename = "L2")]
    L2,
    /// Cosine similarity (default)
    #[default]
    #[serde(rename = "COSINE")]
    Cosine,
    /// Inner product
    #[serde(rename = "IP")]
    IP,
}

impl std::fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DistanceMetric::L2 => write!(f, "L2"),
            DistanceMetric::Cosine => write!(f, "COSINE"),
            DistanceMetric::IP => write!(f, "IP"),
        }
    }
}

/// Vector data type for Redis vector fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum VectorDataType {
    /// 32-bit floating point (default)
    #[default]
    #[serde(rename = "FLOAT32")]
    Float32,
    /// 64-bit floating point
    #[serde(rename = "FLOAT64")]
    Float64,
}

impl std::fmt::Display for VectorDataType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VectorDataType::Float32 => write!(f, "FLOAT32"),
            VectorDataType::Float64 => write!(f, "FLOAT64"),
        }
    }
}

/// Schema for text fields in Redis.
///
/// Text fields support full-text search with stemming, phonetic matching, and weighting.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextFieldSchema {
    /// Field name
    pub name: String,
    /// Field weight for ranking (default: 1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,
    /// Disable stemming (default: false)
    #[serde(default)]
    pub no_stem: bool,
    /// Phonetic matcher algorithm (e.g., "dm:en")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phonetic_matcher: Option<String>,
    /// Enable suffix trie (default: false)
    #[serde(default)]
    pub withsuffixtrie: bool,
    /// Disable indexing (default: false)
    #[serde(default)]
    pub no_index: bool,
    /// Enable sorting on this field (default: false)
    #[serde(default)]
    pub sortable: bool,
}

fn default_weight() -> f64 {
    1.0
}

impl TextFieldSchema {
    /// Create a new text field schema with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            weight: 1.0,
            no_stem: false,
            phonetic_matcher: None,
            withsuffixtrie: false,
            no_index: false,
            sortable: false,
        }
    }

    /// Set the field weight.
    #[must_use]
    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }

    /// Disable stemming.
    #[must_use]
    pub fn with_no_stem(mut self, no_stem: bool) -> Self {
        self.no_stem = no_stem;
        self
    }

    /// Set phonetic matcher.
    pub fn with_phonetic_matcher(mut self, matcher: impl Into<String>) -> Self {
        self.phonetic_matcher = Some(matcher.into());
        self
    }

    /// Enable sortable.
    #[must_use]
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        let mut args = vec![self.name.clone(), "TEXT".to_string()];
        if self.weight != 1.0 {
            args.push("WEIGHT".to_string());
            args.push(self.weight.to_string());
        }
        if self.no_stem {
            args.push("NOSTEM".to_string());
        }
        if let Some(ref matcher) = self.phonetic_matcher {
            args.push("PHONETIC".to_string());
            args.push(matcher.clone());
        }
        if self.sortable {
            args.push("SORTABLE".to_string());
        }
        if self.no_index {
            args.push("NOINDEX".to_string());
        }
        args
    }
}

/// Schema for tag fields in Redis.
///
/// Tag fields support exact matching on categorical data (e.g., tags, categories).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TagFieldSchema {
    /// Field name
    pub name: String,
    /// Tag separator character (default: ",")
    #[serde(default = "default_separator")]
    pub separator: String,
    /// Case-sensitive matching (default: false)
    #[serde(default)]
    pub case_sensitive: bool,
    /// Disable indexing (default: false)
    #[serde(default)]
    pub no_index: bool,
    /// Enable sorting on this field (default: false)
    #[serde(default)]
    pub sortable: bool,
}

fn default_separator() -> String {
    ",".to_string()
}

impl TagFieldSchema {
    /// Create a new tag field schema with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            separator: ",".to_string(),
            case_sensitive: false,
            no_index: false,
            sortable: false,
        }
    }

    /// Set the tag separator.
    pub fn with_separator(mut self, separator: impl Into<String>) -> Self {
        self.separator = separator.into();
        self
    }

    /// Enable case-sensitive matching.
    #[must_use]
    pub fn with_case_sensitive(mut self, case_sensitive: bool) -> Self {
        self.case_sensitive = case_sensitive;
        self
    }

    /// Enable sortable.
    #[must_use]
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        let mut args = vec![self.name.clone(), "TAG".to_string()];
        if self.separator != "," {
            args.push("SEPARATOR".to_string());
            args.push(self.separator.clone());
        }
        if self.case_sensitive {
            args.push("CASESENSITIVE".to_string());
        }
        if self.sortable {
            args.push("SORTABLE".to_string());
        }
        if self.no_index {
            args.push("NOINDEX".to_string());
        }
        args
    }
}

/// Schema for numeric fields in Redis.
///
/// Numeric fields support range queries (e.g., price < 100).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NumericFieldSchema {
    /// Field name
    pub name: String,
    /// Disable indexing (default: false)
    #[serde(default)]
    pub no_index: bool,
    /// Enable sorting on this field (default: false)
    #[serde(default)]
    pub sortable: bool,
}

impl NumericFieldSchema {
    /// Create a new numeric field schema with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            no_index: false,
            sortable: false,
        }
    }

    /// Enable sortable.
    #[must_use]
    pub fn with_sortable(mut self, sortable: bool) -> Self {
        self.sortable = sortable;
        self
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        let mut args = vec![self.name.clone(), "NUMERIC".to_string()];
        if self.sortable {
            args.push("SORTABLE".to_string());
        }
        if self.no_index {
            args.push("NOINDEX".to_string());
        }
        args
    }
}

/// Schema for FLAT vector fields in Redis.
///
/// FLAT algorithm performs exact KNN search (brute force).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlatVectorField {
    /// Field name
    pub name: String,
    /// Vector dimensions
    pub dims: usize,
    /// Vector data type (default: FLOAT32)
    #[serde(default)]
    pub datatype: VectorDataType,
    /// Distance metric (default: COSINE)
    #[serde(default)]
    pub distance_metric: DistanceMetric,
    /// Initial capacity for vectors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_cap: Option<usize>,
    /// Block size for FLAT algorithm
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_size: Option<usize>,
}

impl FlatVectorField {
    /// Create a new FLAT vector field with the given name and dimensions.
    pub fn new(name: impl Into<String>, dims: usize) -> Self {
        Self {
            name: name.into(),
            dims,
            datatype: VectorDataType::Float32,
            distance_metric: DistanceMetric::Cosine,
            initial_cap: None,
            block_size: None,
        }
    }

    /// Set the data type.
    #[must_use]
    pub fn with_datatype(mut self, datatype: VectorDataType) -> Self {
        self.datatype = datatype;
        self
    }

    /// Set the distance metric.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set the initial capacity.
    #[must_use]
    pub fn with_initial_cap(mut self, cap: usize) -> Self {
        self.initial_cap = Some(cap);
        self
    }

    /// Set the block size.
    #[must_use]
    pub fn with_block_size(mut self, size: usize) -> Self {
        self.block_size = Some(size);
        self
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        let mut args = vec![self.name.clone(), "VECTOR".to_string(), "FLAT".to_string()];

        // Count attribute arguments (key-value pairs, so multiply by 2)
        let mut attr_pairs = 3; // TYPE, DIM, DISTANCE_METRIC
        if self.initial_cap.is_some() {
            attr_pairs += 1;
        }
        if self.block_size.is_some() {
            attr_pairs += 1;
        }
        // Redis expects the total count of arguments (pairs × 2)
        args.push((attr_pairs * 2).to_string());

        args.push("TYPE".to_string());
        args.push(self.datatype.to_string());
        args.push("DIM".to_string());
        args.push(self.dims.to_string());
        args.push("DISTANCE_METRIC".to_string());
        args.push(self.distance_metric.to_string());

        if let Some(cap) = self.initial_cap {
            args.push("INITIAL_CAP".to_string());
            args.push(cap.to_string());
        }
        if let Some(size) = self.block_size {
            args.push("BLOCK_SIZE".to_string());
            args.push(size.to_string());
        }

        args
    }
}

/// Schema for HNSW vector fields in Redis.
///
/// HNSW algorithm performs approximate nearest neighbor search with configurable quality.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HNSWVectorField {
    /// Field name
    pub name: String,
    /// Vector dimensions
    pub dims: usize,
    /// Vector data type (default: FLOAT32)
    #[serde(default)]
    pub datatype: VectorDataType,
    /// Distance metric (default: COSINE)
    #[serde(default)]
    pub distance_metric: DistanceMetric,
    /// Initial capacity for vectors
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_cap: Option<usize>,
    /// Number of edges per node (default: 16)
    #[serde(default = "default_m")]
    pub m: usize,
    /// Construction time candidates (default: 200)
    #[serde(default = "default_ef_construction")]
    pub ef_construction: usize,
    /// Search time candidates (default: 10)
    #[serde(default = "default_ef_runtime")]
    pub ef_runtime: usize,
    /// Epsilon for HNSW (default: 0.01)
    #[serde(default = "default_epsilon")]
    pub epsilon: f64,
}

fn default_m() -> usize {
    16
}

fn default_ef_construction() -> usize {
    200
}

fn default_ef_runtime() -> usize {
    10
}

fn default_epsilon() -> f64 {
    0.01
}

impl HNSWVectorField {
    /// Create a new HNSW vector field with the given name and dimensions.
    pub fn new(name: impl Into<String>, dims: usize) -> Self {
        Self {
            name: name.into(),
            dims,
            datatype: VectorDataType::Float32,
            distance_metric: DistanceMetric::Cosine,
            initial_cap: None,
            m: 16,
            ef_construction: 200,
            ef_runtime: 10,
            epsilon: 0.01,
        }
    }

    /// Set the data type.
    #[must_use]
    pub fn with_datatype(mut self, datatype: VectorDataType) -> Self {
        self.datatype = datatype;
        self
    }

    /// Set the distance metric.
    #[must_use]
    pub fn with_distance_metric(mut self, metric: DistanceMetric) -> Self {
        self.distance_metric = metric;
        self
    }

    /// Set the initial capacity.
    #[must_use]
    pub fn with_initial_cap(mut self, cap: usize) -> Self {
        self.initial_cap = Some(cap);
        self
    }

    /// Set the M parameter (edges per node).
    #[must_use]
    pub fn with_m(mut self, m: usize) -> Self {
        self.m = m;
        self
    }

    /// Set the `EF_CONSTRUCTION` parameter.
    #[must_use]
    pub fn with_ef_construction(mut self, ef: usize) -> Self {
        self.ef_construction = ef;
        self
    }

    /// Set the `EF_RUNTIME` parameter.
    #[must_use]
    pub fn with_ef_runtime(mut self, ef: usize) -> Self {
        self.ef_runtime = ef;
        self
    }

    /// Set the EPSILON parameter.
    #[must_use]
    pub fn with_epsilon(mut self, epsilon: f64) -> Self {
        self.epsilon = epsilon;
        self
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        let mut args = vec![self.name.clone(), "VECTOR".to_string(), "HNSW".to_string()];

        // Count attribute arguments (key-value pairs, so multiply by 2)
        // TYPE, DIM, DISTANCE_METRIC, M, EF_CONSTRUCTION, EF_RUNTIME, EPSILON = 7 pairs
        let mut attr_pairs = 7;
        if self.initial_cap.is_some() {
            attr_pairs += 1;
        }
        // Redis expects the total count of arguments (pairs × 2)
        args.push((attr_pairs * 2).to_string());

        args.push("TYPE".to_string());
        args.push(self.datatype.to_string());
        args.push("DIM".to_string());
        args.push(self.dims.to_string());
        args.push("DISTANCE_METRIC".to_string());
        args.push(self.distance_metric.to_string());

        if let Some(cap) = self.initial_cap {
            args.push("INITIAL_CAP".to_string());
            args.push(cap.to_string());
        }

        args.push("M".to_string());
        args.push(self.m.to_string());
        args.push("EF_CONSTRUCTION".to_string());
        args.push(self.ef_construction.to_string());
        args.push("EF_RUNTIME".to_string());
        args.push(self.ef_runtime.to_string());
        args.push("EPSILON".to_string());
        args.push(self.epsilon.to_string());

        args
    }
}

/// Vector field enum (FLAT or HNSW).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "algorithm")]
pub enum VectorField {
    /// FLAT algorithm (exact search)
    #[serde(rename = "FLAT")]
    Flat(FlatVectorField),
    /// HNSW algorithm (approximate search)
    #[serde(rename = "HNSW")]
    Hnsw(HNSWVectorField),
}

impl VectorField {
    /// Get the field name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            VectorField::Flat(f) => &f.name,
            VectorField::Hnsw(f) => &f.name,
        }
    }

    /// Convert to Redis command arguments.
    #[must_use]
    pub fn to_redis_args(&self) -> Vec<String> {
        match self {
            VectorField::Flat(f) => f.to_redis_args(),
            VectorField::Hnsw(f) => f.to_redis_args(),
        }
    }

    /// Get vector dimensions.
    #[must_use]
    pub fn dims(&self) -> usize {
        match self {
            VectorField::Flat(f) => f.dims,
            VectorField::Hnsw(f) => f.dims,
        }
    }

    /// Get data type.
    #[must_use]
    pub fn datatype(&self) -> VectorDataType {
        match self {
            VectorField::Flat(f) => f.datatype,
            VectorField::Hnsw(f) => f.datatype,
        }
    }

    /// Get distance metric.
    #[must_use]
    pub fn distance_metric(&self) -> DistanceMetric {
        match self {
            VectorField::Flat(f) => f.distance_metric,
            VectorField::Hnsw(f) => f.distance_metric,
        }
    }
}

/// Complete Redis index schema.
///
/// This defines all fields in the Redis index including text, tag, numeric, and vector fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedisIndexSchema {
    /// Text fields (always includes "content" by default)
    #[serde(default = "default_text_fields")]
    pub text: Vec<TextFieldSchema>,
    /// Tag fields (categorical data)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag: Vec<TagFieldSchema>,
    /// Numeric fields (range queries)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub numeric: Vec<NumericFieldSchema>,
    /// Vector fields (similarity search)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector: Vec<VectorField>,
    /// Key for the content field (default: "content")
    #[serde(default = "default_content_key")]
    pub content_key: String,
    /// Key for the content vector field (default: "`content_vector`")
    #[serde(default = "default_content_vector_key")]
    pub content_vector_key: String,
}

fn default_text_fields() -> Vec<TextFieldSchema> {
    vec![TextFieldSchema::new("content")]
}

fn default_content_key() -> String {
    "content".to_string()
}

fn default_content_vector_key() -> String {
    "content_vector".to_string()
}

impl Default for RedisIndexSchema {
    fn default() -> Self {
        Self {
            text: default_text_fields(),
            tag: Vec::new(),
            numeric: Vec::new(),
            vector: Vec::new(),
            content_key: "content".to_string(),
            content_vector_key: "content_vector".to_string(),
        }
    }
}

impl RedisIndexSchema {
    /// Create a new index schema.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a text field.
    #[must_use]
    pub fn with_text_field(mut self, field: TextFieldSchema) -> Self {
        self.text.push(field);
        self
    }

    /// Add a tag field.
    #[must_use]
    pub fn with_tag_field(mut self, field: TagFieldSchema) -> Self {
        self.tag.push(field);
        self
    }

    /// Add a numeric field.
    #[must_use]
    pub fn with_numeric_field(mut self, field: NumericFieldSchema) -> Self {
        self.numeric.push(field);
        self
    }

    /// Add a vector field.
    #[must_use]
    pub fn with_vector_field(mut self, field: VectorField) -> Self {
        self.vector.push(field);
        self
    }

    /// Set the content key.
    pub fn with_content_key(mut self, key: impl Into<String>) -> Self {
        self.content_key = key.into();
        self
    }

    /// Set the content vector key.
    pub fn with_content_vector_key(mut self, key: impl Into<String>) -> Self {
        self.content_vector_key = key.into();
        self
    }

    /// Ensure the content field exists in the text fields.
    pub fn ensure_content_field(&mut self) {
        if !self.text.iter().any(|f| f.name == self.content_key) {
            self.text.push(TextFieldSchema::new(&self.content_key));
        }
    }

    /// Add a vector field from a dict-like structure.
    pub fn add_vector_field_from_dict(
        &mut self,
        dict: HashMap<String, serde_json::Value>,
    ) -> Result<(), String> {
        let algorithm = dict
            .get("algorithm")
            .and_then(|v| v.as_str())
            .ok_or("algorithm field required")?;

        match algorithm {
            "FLAT" => {
                let field: FlatVectorField =
                    serde_json::from_value(serde_json::Value::Object(dict.into_iter().collect()))
                        .map_err(|e| e.to_string())?;
                self.vector.push(VectorField::Flat(field));
            }
            "HNSW" => {
                let field: HNSWVectorField =
                    serde_json::from_value(serde_json::Value::Object(dict.into_iter().collect()))
                        .map_err(|e| e.to_string())?;
                self.vector.push(VectorField::Hnsw(field));
            }
            _ => return Err(format!("algorithm must be FLAT or HNSW, got {algorithm}")),
        }
        Ok(())
    }

    /// Get the content vector field.
    pub fn content_vector(&self) -> Result<&VectorField, String> {
        self.vector
            .iter()
            .find(|f| f.name() == self.content_vector_key)
            .ok_or_else(|| {
                format!(
                    "No content_vector field found with name '{}'",
                    self.content_vector_key
                )
            })
    }

    /// Check if the schema is empty (no fields defined).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
            && self.tag.is_empty()
            && self.numeric.is_empty()
            && self.vector.is_empty()
    }

    /// Get all metadata keys (excluding content and `content_vector`).
    #[must_use]
    pub fn metadata_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();

        for field in &self.text {
            if field.name != self.content_key && field.name != self.content_vector_key {
                keys.push(field.name.clone());
            }
        }

        for field in &self.tag {
            if field.name != self.content_key && field.name != self.content_vector_key {
                keys.push(field.name.clone());
            }
        }

        for field in &self.numeric {
            if field.name != self.content_key && field.name != self.content_vector_key {
                keys.push(field.name.clone());
            }
        }

        keys
    }

    /// Convert to Redis FT.CREATE command arguments.
    #[must_use]
    pub fn to_redis_schema_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        for field in &self.text {
            args.extend(field.to_redis_args());
        }

        for field in &self.tag {
            args.extend(field.to_redis_args());
        }

        for field in &self.numeric {
            args.extend(field.to_redis_args());
        }

        for field in &self.vector {
            args.extend(field.to_redis_args());
        }

        args
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_metric_display() {
        assert_eq!(DistanceMetric::L2.to_string(), "L2");
        assert_eq!(DistanceMetric::Cosine.to_string(), "COSINE");
        assert_eq!(DistanceMetric::IP.to_string(), "IP");
    }

    #[test]
    fn test_distance_metric_default() {
        assert_eq!(DistanceMetric::default(), DistanceMetric::Cosine);
    }

    #[test]
    fn test_vector_datatype_display() {
        assert_eq!(VectorDataType::Float32.to_string(), "FLOAT32");
        assert_eq!(VectorDataType::Float64.to_string(), "FLOAT64");
    }

    #[test]
    fn test_vector_datatype_default() {
        assert_eq!(VectorDataType::default(), VectorDataType::Float32);
    }

    #[test]
    fn test_text_field_schema_new() {
        let field = TextFieldSchema::new("test_field");
        assert_eq!(field.name, "test_field");
        assert_eq!(field.weight, 1.0);
        assert!(!field.no_stem);
        assert!(field.phonetic_matcher.is_none());
        assert!(!field.sortable);
    }

    #[test]
    fn test_text_field_schema_builder() {
        let field = TextFieldSchema::new("test")
            .with_weight(2.0)
            .with_no_stem(true)
            .with_phonetic_matcher("dm:en")
            .with_sortable(true);

        assert_eq!(field.name, "test");
        assert_eq!(field.weight, 2.0);
        assert!(field.no_stem);
        assert_eq!(field.phonetic_matcher, Some("dm:en".to_string()));
        assert!(field.sortable);
    }

    #[test]
    fn test_text_field_to_redis_args() {
        let field = TextFieldSchema::new("content");
        let args = field.to_redis_args();
        assert_eq!(args, vec!["content", "TEXT"]);

        let field = TextFieldSchema::new("content")
            .with_weight(2.0)
            .with_no_stem(true)
            .with_sortable(true);
        let args = field.to_redis_args();
        assert_eq!(
            args,
            vec!["content", "TEXT", "WEIGHT", "2", "NOSTEM", "SORTABLE"]
        );
    }

    #[test]
    fn test_tag_field_schema_new() {
        let field = TagFieldSchema::new("category");
        assert_eq!(field.name, "category");
        assert_eq!(field.separator, ",");
        assert!(!field.case_sensitive);
        assert!(!field.sortable);
    }

    #[test]
    fn test_tag_field_schema_builder() {
        let field = TagFieldSchema::new("tags")
            .with_separator("|")
            .with_case_sensitive(true)
            .with_sortable(true);

        assert_eq!(field.name, "tags");
        assert_eq!(field.separator, "|");
        assert!(field.case_sensitive);
        assert!(field.sortable);
    }

    #[test]
    fn test_tag_field_to_redis_args() {
        let field = TagFieldSchema::new("category");
        let args = field.to_redis_args();
        assert_eq!(args, vec!["category", "TAG"]);

        let field = TagFieldSchema::new("tags")
            .with_separator("|")
            .with_case_sensitive(true)
            .with_sortable(true);
        let args = field.to_redis_args();
        assert_eq!(
            args,
            vec!["tags", "TAG", "SEPARATOR", "|", "CASESENSITIVE", "SORTABLE"]
        );
    }

    #[test]
    fn test_numeric_field_schema_new() {
        let field = NumericFieldSchema::new("price");
        assert_eq!(field.name, "price");
        assert!(!field.sortable);
    }

    #[test]
    fn test_numeric_field_to_redis_args() {
        let field = NumericFieldSchema::new("price");
        let args = field.to_redis_args();
        assert_eq!(args, vec!["price", "NUMERIC"]);

        let field = NumericFieldSchema::new("age").with_sortable(true);
        let args = field.to_redis_args();
        assert_eq!(args, vec!["age", "NUMERIC", "SORTABLE"]);
    }

    #[test]
    fn test_flat_vector_field_new() {
        let field = FlatVectorField::new("embedding", 384);
        assert_eq!(field.name, "embedding");
        assert_eq!(field.dims, 384);
        assert_eq!(field.datatype, VectorDataType::Float32);
        assert_eq!(field.distance_metric, DistanceMetric::Cosine);
        assert!(field.initial_cap.is_none());
        assert!(field.block_size.is_none());
    }

    #[test]
    fn test_flat_vector_field_builder() {
        let field = FlatVectorField::new("vec", 128)
            .with_datatype(VectorDataType::Float64)
            .with_distance_metric(DistanceMetric::L2)
            .with_initial_cap(1000)
            .with_block_size(100);

        assert_eq!(field.dims, 128);
        assert_eq!(field.datatype, VectorDataType::Float64);
        assert_eq!(field.distance_metric, DistanceMetric::L2);
        assert_eq!(field.initial_cap, Some(1000));
        assert_eq!(field.block_size, Some(100));
    }

    #[test]
    fn test_flat_vector_field_to_redis_args() {
        let field = FlatVectorField::new("embedding", 384);
        let args = field.to_redis_args();
        assert_eq!(
            args,
            vec![
                "embedding",
                "VECTOR",
                "FLAT",
                "6", // 3 pairs × 2 = 6 arguments
                "TYPE",
                "FLOAT32",
                "DIM",
                "384",
                "DISTANCE_METRIC",
                "COSINE"
            ]
        );

        let field = FlatVectorField::new("embedding", 384)
            .with_initial_cap(1000)
            .with_block_size(100);
        let args = field.to_redis_args();
        assert_eq!(
            args,
            vec![
                "embedding",
                "VECTOR",
                "FLAT",
                "10", // 5 pairs × 2 = 10 arguments
                "TYPE",
                "FLOAT32",
                "DIM",
                "384",
                "DISTANCE_METRIC",
                "COSINE",
                "INITIAL_CAP",
                "1000",
                "BLOCK_SIZE",
                "100"
            ]
        );
    }

    #[test]
    fn test_hnsw_vector_field_new() {
        let field = HNSWVectorField::new("embedding", 768);
        assert_eq!(field.name, "embedding");
        assert_eq!(field.dims, 768);
        assert_eq!(field.datatype, VectorDataType::Float32);
        assert_eq!(field.distance_metric, DistanceMetric::Cosine);
        assert!(field.initial_cap.is_none());
        assert_eq!(field.m, 16);
        assert_eq!(field.ef_construction, 200);
        assert_eq!(field.ef_runtime, 10);
        assert_eq!(field.epsilon, 0.01);
    }

    #[test]
    fn test_hnsw_vector_field_builder() {
        let field = HNSWVectorField::new("vec", 256)
            .with_datatype(VectorDataType::Float64)
            .with_distance_metric(DistanceMetric::IP)
            .with_initial_cap(5000)
            .with_m(32)
            .with_ef_construction(400)
            .with_ef_runtime(20)
            .with_epsilon(0.02);

        assert_eq!(field.dims, 256);
        assert_eq!(field.datatype, VectorDataType::Float64);
        assert_eq!(field.distance_metric, DistanceMetric::IP);
        assert_eq!(field.initial_cap, Some(5000));
        assert_eq!(field.m, 32);
        assert_eq!(field.ef_construction, 400);
        assert_eq!(field.ef_runtime, 20);
        assert_eq!(field.epsilon, 0.02);
    }

    #[test]
    fn test_hnsw_vector_field_to_redis_args() {
        let field = HNSWVectorField::new("embedding", 1536);
        let args = field.to_redis_args();
        assert_eq!(
            args,
            vec![
                "embedding",
                "VECTOR",
                "HNSW",
                "14",
                "TYPE",
                "FLOAT32",
                "DIM",
                "1536",
                "DISTANCE_METRIC",
                "COSINE",
                "M",
                "16",
                "EF_CONSTRUCTION",
                "200",
                "EF_RUNTIME",
                "10",
                "EPSILON",
                "0.01"
            ]
        );
    }

    #[test]
    fn test_vector_field_enum() {
        let flat = VectorField::Flat(FlatVectorField::new("vec", 128));
        assert_eq!(flat.name(), "vec");
        assert_eq!(flat.dims(), 128);
        assert_eq!(flat.datatype(), VectorDataType::Float32);

        let hnsw = VectorField::Hnsw(HNSWVectorField::new("vec", 256));
        assert_eq!(hnsw.name(), "vec");
        assert_eq!(hnsw.dims(), 256);
        assert_eq!(hnsw.datatype(), VectorDataType::Float32);
    }

    #[test]
    fn test_redis_index_schema_default() {
        let schema = RedisIndexSchema::default();
        assert_eq!(schema.text.len(), 1);
        assert_eq!(schema.text[0].name, "content");
        assert_eq!(schema.tag.len(), 0);
        assert_eq!(schema.numeric.len(), 0);
        assert_eq!(schema.vector.len(), 0);
        assert_eq!(schema.content_key, "content");
        assert_eq!(schema.content_vector_key, "content_vector");
    }

    #[test]
    fn test_redis_index_schema_builder() {
        let schema = RedisIndexSchema::new()
            .with_text_field(TextFieldSchema::new("description"))
            .with_tag_field(TagFieldSchema::new("category"))
            .with_numeric_field(NumericFieldSchema::new("price"))
            .with_vector_field(VectorField::Hnsw(HNSWVectorField::new(
                "content_vector",
                384,
            )))
            .with_content_key("text")
            .with_content_vector_key("vec");

        assert_eq!(schema.text.len(), 2); // "content" (default) + "description"
        assert_eq!(schema.tag.len(), 1);
        assert_eq!(schema.numeric.len(), 1);
        assert_eq!(schema.vector.len(), 1);
        assert_eq!(schema.content_key, "text");
        assert_eq!(schema.content_vector_key, "vec");
    }

    #[test]
    fn test_redis_index_schema_ensure_content_field() {
        let mut schema = RedisIndexSchema {
            text: vec![],
            tag: vec![],
            numeric: vec![],
            vector: vec![],
            content_key: "content".to_string(),
            content_vector_key: "content_vector".to_string(),
        };

        schema.ensure_content_field();
        assert_eq!(schema.text.len(), 1);
        assert_eq!(schema.text[0].name, "content");

        // Should not add duplicate
        schema.ensure_content_field();
        assert_eq!(schema.text.len(), 1);
    }

    #[test]
    fn test_redis_index_schema_content_vector() {
        let mut schema = RedisIndexSchema::new();

        // Should error when no vector field
        assert!(schema.content_vector().is_err());

        // Should find the content_vector field
        schema.vector.push(VectorField::Hnsw(HNSWVectorField::new(
            "content_vector",
            384,
        )));
        let vec_field = schema.content_vector().unwrap();
        assert_eq!(vec_field.name(), "content_vector");
        assert_eq!(vec_field.dims(), 384);
    }

    #[test]
    fn test_redis_index_schema_metadata_keys() {
        let schema = RedisIndexSchema::new()
            .with_text_field(TextFieldSchema::new("description"))
            .with_tag_field(TagFieldSchema::new("category"))
            .with_numeric_field(NumericFieldSchema::new("price"));

        let keys = schema.metadata_keys();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&"description".to_string()));
        assert!(keys.contains(&"category".to_string()));
        assert!(keys.contains(&"price".to_string()));
        // Should not include "content" (content_key)
        assert!(!keys.contains(&"content".to_string()));
    }

    #[test]
    fn test_redis_index_schema_to_redis_schema_args() {
        let schema = RedisIndexSchema::new().with_vector_field(VectorField::Flat(
            FlatVectorField::new("content_vector", 384),
        ));

        let args = schema.to_redis_schema_args();

        // Should include content field (default text field)
        assert!(args.contains(&"content".to_string()));
        assert!(args.contains(&"TEXT".to_string()));

        // Should include vector field
        assert!(args.contains(&"content_vector".to_string()));
        assert!(args.contains(&"VECTOR".to_string()));
        assert!(args.contains(&"FLAT".to_string()));
        assert!(args.contains(&"384".to_string()));
    }

    #[test]
    fn test_redis_index_schema_is_empty() {
        let mut schema = RedisIndexSchema {
            text: vec![],
            tag: vec![],
            numeric: vec![],
            vector: vec![],
            content_key: "content".to_string(),
            content_vector_key: "content_vector".to_string(),
        };

        assert!(schema.is_empty());

        schema.text.push(TextFieldSchema::new("content"));
        assert!(!schema.is_empty());
    }

    #[test]
    fn test_schema_serialization() {
        let schema = RedisIndexSchema::new()
            .with_tag_field(TagFieldSchema::new("category"))
            .with_numeric_field(NumericFieldSchema::new("price"))
            .with_vector_field(VectorField::Hnsw(HNSWVectorField::new(
                "content_vector",
                384,
            )));

        // Test JSON serialization
        let json = serde_json::to_string(&schema).unwrap();
        let deserialized: RedisIndexSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(schema, deserialized);
    }
}
