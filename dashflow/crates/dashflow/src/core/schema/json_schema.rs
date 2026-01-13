//! JSON Schema generation for structured outputs
//!
//! This module provides utilities for generating JSON schemas from Rust types
//! to enable structured output parsing from LLM responses.
//!
//! # Overview
//!
//! The `json_schema()` function generates JSON schemas from types that implement
//! `serde::Serialize` and `schemars::JsonSchema`. These schemas can be used to:
//!
//! - Configure LLMs to return structured JSON responses
//! - Validate LLM outputs against expected schemas
//! - Generate `OpenAI` function calling schemas
//!
//! # Example
//!
//! ```rust
//! use serde::{Serialize, Deserialize};
//! use schemars::JsonSchema;
//! use dashflow::core::schema::json_schema::json_schema;
//!
//! #[derive(Serialize, Deserialize, JsonSchema)]
//! struct GradeHallucinations {
//!     binary_score: bool,
//!     reasoning: String,
//! }
//!
//! let schema = json_schema::<GradeHallucinations>().unwrap();
//! println!("{}", serde_json::to_string_pretty(&schema).unwrap());
//! ```
//!
//! # Supported Types
//!
//! - Primitive types: `String`, `i32`, `i64`, `f32`, `f64`, `bool`
//! - Collections: `Vec<T>`, `HashMap<K, V>`
//! - Options: `Option<T>`
//! - Custom structs with `#[derive(JsonSchema)]`
//! - Enums with `#[derive(JsonSchema)]`

use schemars::{schema_for, JsonSchema};
use serde_json::Value;

use crate::core::error::Result;

/// Generate a JSON schema for type `T`.
///
/// This function uses the `schemars` crate to generate a JSON Schema Draft 7
/// representation of the given Rust type. The resulting schema can be used
/// for LLM structured outputs, validation, or function calling.
///
/// # Type Parameters
///
/// * `T` - The type to generate a schema for. Must implement `JsonSchema`.
///
/// # Returns
///
/// Returns a `serde_json::Value` containing the JSON schema object, or an error
/// if schema generation fails.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use schemars::JsonSchema;
/// use dashflow::core::schema::json_schema::json_schema;
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Person {
///     name: String,
///     age: u32,
/// }
///
/// let schema = json_schema::<Person>().unwrap();
/// assert!(schema.is_object());
/// ```
pub fn json_schema<T: JsonSchema>() -> Result<Value> {
    let schema = schema_for!(T);
    Ok(serde_json::to_value(schema)?)
}

/// Generate a JSON schema for type `T` with a custom title.
///
/// This is useful when you want to override the default schema title
/// (which is typically the type name).
///
/// # Type Parameters
///
/// * `T` - The type to generate a schema for. Must implement `JsonSchema`.
///
/// # Arguments
///
/// * `title` - The title to use for the schema
///
/// # Returns
///
/// Returns a `serde_json::Value` containing the JSON schema object with the
/// custom title, or an error if schema generation fails.
///
/// # Example
///
/// ```rust
/// use serde::{Serialize, Deserialize};
/// use schemars::JsonSchema;
/// use dashflow::core::schema::json_schema::json_schema_with_title;
///
/// #[derive(Serialize, Deserialize, JsonSchema)]
/// struct Response {
///     answer: String,
/// }
///
/// let schema = json_schema_with_title::<Response>("GradeResponse").unwrap();
/// let title = schema.get("title").and_then(|v| v.as_str());
/// assert_eq!(title, Some("GradeResponse"));
/// ```
pub fn json_schema_with_title<T: JsonSchema>(title: &str) -> Result<Value> {
    let mut schema = schema_for!(T);
    // In schemars 1.x, Schema is a wrapper around serde_json::Value
    // We can modify it like a JSON object
    if let Some(obj) = schema.as_object_mut() {
        obj.insert("title".to_string(), Value::String(title.to_string()));
    }
    Ok(serde_json::to_value(schema)?)
}

#[cfg(test)]
mod tests {
    use super::{json_schema, json_schema_with_title};
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[test]
    fn test_debug_schema_structure() {
        // Debug test to see actual schema structure
        let schema = json_schema::<String>().unwrap();
        println!(
            "String schema: {}",
            serde_json::to_string_pretty(&schema).unwrap()
        );

        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Person {
            name: String,
            age: u32,
        }
        let schema = json_schema::<Person>().unwrap();
        println!(
            "Person schema: {}",
            serde_json::to_string_pretty(&schema).unwrap()
        );
    }

    #[test]
    fn test_primitive_string() {
        let schema = json_schema::<String>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let type_field = schema_obj.get("type").unwrap();

        assert_eq!(type_field, "string");
    }

    #[test]
    fn test_primitive_i32() {
        let schema = json_schema::<i32>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let type_field = schema_obj.get("type").unwrap();

        assert_eq!(type_field, "integer");
    }

    #[test]
    fn test_primitive_bool() {
        let schema = json_schema::<bool>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let type_field = schema_obj.get("type").unwrap();

        assert_eq!(type_field, "boolean");
    }

    #[test]
    fn test_primitive_f64() {
        let schema = json_schema::<f64>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let type_field = schema_obj.get("type").unwrap();

        assert_eq!(type_field, "number");
    }

    #[test]
    fn test_struct_simple() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Person {
            name: String,
            age: u32,
        }

        let schema = json_schema::<Person>().unwrap();
        assert!(schema.is_object());

        // Verify it has the expected structure
        let schema_obj = schema.as_object().unwrap();
        let properties = schema_obj.get("properties");
        assert!(properties.is_some());

        let props = properties.unwrap().as_object().unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("age"));
    }

    #[test]
    fn test_struct_with_optional() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Config {
            required_field: String,
            optional_field: Option<i32>,
        }

        let schema = json_schema::<Config>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let properties = schema_obj.get("properties").unwrap();
        let props = properties.as_object().unwrap();

        assert!(props.contains_key("required_field"));
        assert!(props.contains_key("optional_field"));
    }

    #[test]
    fn test_struct_with_vec() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Items {
            list: Vec<String>,
        }

        let schema = json_schema::<Items>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let properties = schema_obj.get("properties").unwrap();
        let props = properties.as_object().unwrap();

        assert!(props.contains_key("list"));

        // Verify list is an array type
        let list_schema = props.get("list").unwrap();
        let list_type = list_schema.get("type").unwrap();
        assert_eq!(list_type, "array");
    }

    #[test]
    fn test_enum_simple() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        enum Status {
            Active,
            Inactive,
            Pending,
        }

        let schema = json_schema::<Status>().unwrap();
        assert!(schema.is_object());

        // Verify enum is represented correctly - should have oneOf
        let schema_obj = schema.as_object().unwrap();
        assert!(schema_obj.contains_key("oneOf") || schema_obj.contains_key("enum"));
    }

    #[test]
    fn test_nested_struct() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Address {
            street: String,
            city: String,
        }

        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Person {
            name: String,
            address: Address,
        }

        let schema = json_schema::<Person>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let properties = schema_obj.get("properties").unwrap();
        let props = properties.as_object().unwrap();

        assert!(props.contains_key("name"));
        assert!(props.contains_key("address"));
    }

    #[test]
    fn test_json_schema_with_title() {
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct Response {
            answer: String,
        }

        let schema = json_schema_with_title::<Response>("CustomTitle").unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let title = schema_obj.get("title");
        assert!(title.is_some());
        assert_eq!(title.unwrap(), "CustomTitle");
    }

    #[test]
    fn test_grade_hallucinations_schema() {
        // This is the actual use case from the implementation plan
        #[derive(Serialize, Deserialize, JsonSchema)]
        struct GradeHallucinations {
            binary_score: bool,
            reasoning: String,
        }

        let schema = json_schema::<GradeHallucinations>().unwrap();
        assert!(schema.is_object());

        let schema_obj = schema.as_object().unwrap();
        let properties = schema_obj.get("properties").unwrap();
        let props = properties.as_object().unwrap();

        // Verify both fields are present
        assert!(props.contains_key("binary_score"));
        assert!(props.contains_key("reasoning"));

        // Verify field types
        let binary_score = props.get("binary_score").unwrap();
        let binary_type = binary_score.get("type").unwrap();
        assert_eq!(binary_type, "boolean");

        let reasoning = props.get("reasoning").unwrap();
        let reasoning_type = reasoning.get("type").unwrap();
        assert_eq!(reasoning_type, "string");
    }
}
