// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Procedural macros for DashFlow state management with compile-time safety.
//!
//! This crate provides derive macros for DashFlow state types, eliminating
//! boilerplate and ensuring compile-time correctness.
//!
//! # Available Macros
//!
//! - [`GraphState`] - Compile-time verification of required traits
//! - [`MergeableState`] - Automatic merge implementations for parallel execution
//! - [`DashFlowTool`] - Simplified tool creation with automatic schema generation
//!
//! # Example
//!
//! ```ignore
//! use dashflow::prelude::*;
//! use dashflow_derive::{GraphState, MergeableState};
//!
//! #[derive(Debug, Clone, Default, Serialize, Deserialize, GraphState, MergeableState)]
//! struct MyState {
//!     messages: Vec<String>,
//!     counter: u32,
//!     #[merge(skip)]  // Keep existing value
//!     id: String,
//! }
//! ```
//!
//! # Merge Strategies
//!
//! The `MergeableState` derive supports field-level merge strategies via the
//! `#[merge(...)]` attribute:
//!
//! - `#[merge(skip)]` - Keep self's value unchanged
//! - `#[merge(replace)]` - Replace self with other if non-empty
//! - `#[merge(take_if_empty)]` - Take other only if self is empty
//! - `#[merge(recursive)]` - Call merge() on nested MergeableState types
//!
//! Without an attribute, the default strategy is used based on the field type.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Expr, Fields, Lit, Meta, MetaNameValue,
};

/// Merge strategy for a field in MergeableState derive
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MergeStrategy {
    /// Use default type-based merge strategy
    Default,
    /// Keep self's value unchanged (no merge)
    Skip,
    /// Replace self with other if other is non-empty/non-default
    Replace,
    /// Take other only if self is empty
    TakeIfEmpty,
    /// Call merge() on nested MergeableState types
    Recursive,
}

/// Parse #[merge(...)] attribute from field attributes
fn parse_merge_attribute(attrs: &[Attribute]) -> MergeStrategy {
    for attr in attrs {
        if attr.path().is_ident("merge") {
            // Parse the attribute to determine strategy
            if let Ok(list) = attr.meta.require_list() {
                let tokens = list.tokens.to_string();
                if tokens.contains("skip") {
                    return MergeStrategy::Skip;
                } else if tokens.contains("replace") {
                    return MergeStrategy::Replace;
                } else if tokens.contains("take_if_empty") {
                    return MergeStrategy::TakeIfEmpty;
                } else if tokens.contains("recursive") {
                    return MergeStrategy::Recursive;
                }
            }
        }
    }
    MergeStrategy::Default
}

/// Derive macro for `GraphState` trait.
///
/// This macro verifies at compile time that the type implements the required traits:
/// - Clone
/// - `serde::Serialize`
/// - `serde::Deserialize`
///
/// # Example
///
/// ```rust
/// use dashflow_derive::GraphState;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Serialize, Deserialize, GraphState)]
/// struct MyState {
///     messages: Vec<String>,
/// }
/// ```
#[proc_macro_derive(GraphState)]
pub fn derive_graph_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Generate compile-time assertions to ensure required traits are implemented
    let expanded = quote! {
        // Compile-time assertion that required traits are implemented
        const _: () = {
            fn assert_graph_state<T>()
            where
                T: Clone + serde::Serialize + for<'de> serde::Deserialize<'de>
            {}

            fn assert_impl() {
                assert_graph_state::<#name>();
            }
        };
    };

    TokenStream::from(expanded)
}

/// Derive macro for `MergeableState` trait.
///
/// This macro automatically generates a merge implementation based on field types:
/// - `Vec<T>`: Extends the vector with elements from other
/// - `VecDeque<T>`: Extends the deque with elements from other
/// - `HashSet<T>`: Extends the set with elements from other
/// - `BTreeSet<T>`: Extends the set with elements from other
/// - `HashMap<K, V>`: Extends the map with entries from other (overwrites on key collision)
/// - `BTreeMap<K, V>`: Extends the map with entries from other (overwrites on key collision)
/// - `Option<T>`: Takes other's value if self is None
/// - Numeric types (i32, u32, i64, u64, f32, f64, usize, isize, i8, u8, i16, u16): Takes max value
/// - String: Concatenates with newline separator if both non-empty
/// - bool: Logical OR (true if either is true)
/// - Other types: Keeps self's value (no merge)
///
/// # Field Attributes
///
/// Override default merge behavior per-field using `#[merge(...)]`:
///
/// - `#[merge(skip)]` - Keep self's value unchanged (no merge)
/// - `#[merge(replace)]` - Replace self with other if other is non-empty/non-default
/// - `#[merge(take_if_empty)]` - Take other only if self is empty (for Vec/String)
/// - `#[merge(recursive)]` - Call merge() on nested MergeableState types
///
/// # Example
///
/// ```rust
/// use dashflow_derive::MergeableState;
/// use serde::{Deserialize, Serialize};
/// use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
///
/// #[derive(Clone, Serialize, Deserialize, MergeableState)]
/// struct MyState {
///     findings: Vec<String>,
///     queue: VecDeque<String>,
///     tags: HashSet<String>,
///     ordered_tags: BTreeSet<String>,
///     metadata: HashMap<String, String>,
///     ordered_metadata: BTreeMap<String, String>,
///     count: usize,
///     description: String,
///     enabled: bool,
///     #[merge(skip)]
///     immutable_id: String,
///     #[merge(replace)]
///     status: String,
///     #[merge(take_if_empty)]
///     initial_data: Vec<String>,
/// }
/// ```
#[proc_macro_derive(MergeableState, attributes(merge))]
pub fn derive_mergeable_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    name,
                    "MergeableState can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(name, "MergeableState can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // Generate merge logic for each field based on its type and attributes
    let merge_fields = fields.iter().map(|field| {
        // SAFETY: Named struct fields always have identifiers (we validated Fields::Named above)
        #[allow(clippy::unwrap_used)]
        let field_name = field.ident.as_ref().unwrap();
        let field_type = &field.ty;

        // Parse #[merge(...)] attributes
        let merge_attr = parse_merge_attribute(&field.attrs);

        // Convert type to string for pattern matching
        let type_str = quote!(#field_type).to_string();

        // Handle explicit merge attributes first
        match merge_attr {
            MergeStrategy::Skip => {
                return quote! {
                    // Keep self.#field_name unchanged (skip)
                };
            }
            MergeStrategy::Replace => {
                // Replace self with other if other is non-empty/non-default
                // String and Vec both support is_empty()
                let is_string = type_str == "String" || type_str == "std :: string :: String";
                let is_vec = type_str.starts_with("Vec <") || type_str.contains("::Vec<");
                let is_option = type_str.starts_with("Option <") || type_str.contains("::Option<");

                if is_string || is_vec {
                    return quote! {
                        if !other.#field_name.is_empty() {
                            self.#field_name = other.#field_name.clone();
                        }
                    };
                } else if is_option {
                    return quote! {
                        if other.#field_name.is_some() {
                            self.#field_name = other.#field_name.clone();
                        }
                    };
                } else {
                    // For other types, just replace
                    return quote! {
                        self.#field_name = other.#field_name.clone();
                    };
                }
            }
            MergeStrategy::TakeIfEmpty => {
                // Take other only if self is empty
                // String and Vec both support is_empty()
                let is_string = type_str == "String" || type_str == "std :: string :: String";
                let is_vec = type_str.starts_with("Vec <") || type_str.contains("::Vec<");
                let is_option = type_str.starts_with("Option <") || type_str.contains("::Option<");

                if is_string || is_vec {
                    return quote! {
                        if self.#field_name.is_empty() && !other.#field_name.is_empty() {
                            self.#field_name = other.#field_name.clone();
                        }
                    };
                } else if is_option {
                    return quote! {
                        if self.#field_name.is_none() {
                            self.#field_name = other.#field_name.clone();
                        }
                    };
                } else {
                    // Fallback: keep self
                    return quote! {
                        // Keep self.#field_name unchanged (take_if_empty not applicable)
                    };
                }
            }
            MergeStrategy::Recursive => {
                // Call merge() on nested MergeableState types
                return quote! {
                    self.#field_name.merge(&other.#field_name);
                };
            }
            MergeStrategy::Default => {
                // Fall through to type-based inference
            }
        }

        // Determine merge strategy based on type (default behavior)
        // Check if type is a collection that supports extend()
        let is_extendable = type_str.starts_with("Vec <")
            || type_str.contains("::Vec<")
            || type_str.starts_with("VecDeque <")
            || type_str.contains("::VecDeque<")
            || type_str.starts_with("HashSet <")
            || type_str.contains("::HashSet<")
            || type_str.starts_with("BTreeSet <")
            || type_str.contains("::BTreeSet<")
            || type_str.starts_with("HashMap <")
            || type_str.contains("::HashMap<")
            || type_str.starts_with("BTreeMap <")
            || type_str.contains("::BTreeMap<");

        if is_extendable {
            // Collection types: extend with other's elements/entries
            quote! {
                self.#field_name.extend(other.#field_name.clone());
            }
        } else if type_str.starts_with("Option <") || type_str.contains("::Option<") {
            // Option<T>: take other if self is None
            quote! {
                if self.#field_name.is_none() {
                    self.#field_name = other.#field_name.clone();
                }
            }
        } else if type_str == "String" || type_str == "std :: string :: String" {
            // String: concatenate with newline if both non-empty
            quote! {
                if !other.#field_name.is_empty() {
                    if !self.#field_name.is_empty() {
                        self.#field_name.push('\n');
                    }
                    self.#field_name.push_str(&other.#field_name);
                }
            }
        } else if [
            "i32", "u32", "i64", "u64", "f32", "f64", "usize", "isize", "i8", "u8", "i16", "u16",
        ]
        .iter()
        .any(|&t| type_str == t)
        {
            // Numeric types: take max
            quote! {
                self.#field_name = self.#field_name.max(other.#field_name);
            }
        } else if type_str == "bool" {
            // bool: logical OR (true if either is true)
            quote! {
                self.#field_name = self.#field_name || other.#field_name;
            }
        } else {
            // Other types: keep self (no merge)
            // This is safe default behavior - preserves existing data
            quote! {
                // Keep self.#field_name unchanged (type doesn't support automatic merging)
            }
        }
    });

    let expanded = quote! {
        impl dashflow::state::MergeableState for #name {
            fn merge(&mut self, other: &Self) {
                #( #merge_fields )*
            }
        }
    };

    TokenStream::from(expanded)
}

/// Derive macro for `Tool` trait implementation.
///
/// This macro simplifies tool creation by generating the `Tool` trait implementation
/// from a struct definition with attributes. The struct fields become the tool's
/// input parameters, and doc comments become parameter descriptions.
///
/// # Attributes
///
/// - `#[tool(name = "...")]` - The tool's name (optional, defaults to struct name in snake_case)
/// - `#[tool(description = "...")]` - The tool's description (required)
///
/// # Field Attributes
///
/// - `#[arg(default = ...)]` - Default value for optional parameters
/// - Doc comments (`///`) - Used as parameter descriptions
///
/// # Example
///
/// ```rust,ignore
/// use dashflow_derive::DashFlowTool;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Clone, Debug, Serialize, Deserialize, DashFlowTool)]
/// #[tool(name = "read_file", description = "Read contents of a file")]
/// struct ReadFile {
///     /// Path to the file to read
///     path: String,
///
///     /// Maximum lines to return
///     #[arg(default = 1000)]
///     max_lines: Option<u32>,
/// }
/// ```
///
/// This generates:
/// - `name()` returns "read_file"
/// - `description()` returns "Read contents of a file"
/// - `args_schema()` returns JSON schema with path and max_lines parameters
#[proc_macro_derive(DashFlowTool, attributes(tool, arg))]
pub fn derive_dashflow_tool(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let struct_name = &input.ident;

    // Extract tool attributes
    let mut tool_name: Option<String> = None;
    let mut tool_description: Option<String> = None;

    for attr in &input.attrs {
        if attr.path().is_ident("tool") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    tool_name = Some(value.value());
                } else if meta.path.is_ident("description") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    tool_description = Some(value.value());
                }
                Ok(())
            });
        }
    }

    // Default tool name to snake_case of struct name
    let tool_name = tool_name.unwrap_or_else(|| to_snake_case(&struct_name.to_string()));

    // Description is required
    let tool_description = match tool_description {
        Some(desc) => desc,
        None => {
            return syn::Error::new_spanned(
                struct_name,
                "DashFlowTool requires #[tool(description = \"...\")] attribute",
            )
            .to_compile_error()
            .into();
        }
    };

    // Extract struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    struct_name,
                    "DashFlowTool can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(
                struct_name,
                "DashFlowTool can only be derived for structs",
            )
            .to_compile_error()
            .into();
        }
    };

    // Generate JSON schema properties and required fields
    let mut properties = Vec::new();
    let mut required_fields = Vec::new();

    for field in fields {
        // SAFETY: Named struct fields always have identifiers (we validated Fields::Named above)
        #[allow(clippy::unwrap_used)]
        let field_name = field.ident.as_ref().unwrap();
        let field_name_str = field_name.to_string();
        let field_type = &field.ty;
        let type_str = quote!(#field_type).to_string();

        // Extract doc comment for description
        let mut field_description = String::new();
        let mut has_default = false;
        let mut default_value: Option<String> = None;

        for attr in &field.attrs {
            if attr.path().is_ident("doc") {
                if let Meta::NameValue(MetaNameValue {
                    value: Expr::Lit(expr_lit),
                    ..
                }) = &attr.meta
                {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let doc = lit.value().trim().to_string();
                        if !field_description.is_empty() {
                            field_description.push(' ');
                        }
                        field_description.push_str(&doc);
                    }
                }
            } else if attr.path().is_ident("arg") {
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("default") {
                        has_default = true;
                        let value: syn::Lit = meta.value()?.parse()?;
                        match value {
                            Lit::Str(lit) => default_value = Some(format!("\"{}\"", lit.value())),
                            Lit::Int(lit) => default_value = Some(lit.to_string()),
                            Lit::Float(lit) => default_value = Some(lit.to_string()),
                            Lit::Bool(lit) => default_value = Some(lit.value.to_string()),
                            _ => {}
                        }
                    }
                    Ok(())
                });
            }
        }

        // Determine JSON schema type
        let (json_type, is_optional) = get_json_type(&type_str);

        // Build property JSON
        let mut prop = format!(r#""{}": {{"type": "{}""#, field_name_str, json_type);

        if !field_description.is_empty() {
            prop.push_str(&format!(
                r#", "description": "{}""#,
                escape_json(&field_description)
            ));
        }

        if let Some(default) = default_value {
            prop.push_str(&format!(r#", "default": {}"#, default));
        }

        prop.push('}');
        properties.push(prop);

        // Add to required if not optional and no default
        if !is_optional && !has_default {
            required_fields.push(format!(r#""{}""#, field_name_str));
        }
    }

    let properties_json = properties.join(", ");
    let required_json = required_fields.join(", ");

    // Generate the Tool implementation
    let expanded = quote! {
        #[async_trait::async_trait]
        impl dashflow::tools::Tool for #struct_name {
            fn name(&self) -> &str {
                #tool_name
            }

            fn description(&self) -> &str {
                #tool_description
            }

            fn args_schema(&self) -> serde_json::Value {
                serde_json::json!({
                    "type": "object",
                    "properties": serde_json::from_str::<serde_json::Value>(
                        &format!(r#"{{{}}}"#, #properties_json)
                    ).unwrap_or_default(),
                    "required": serde_json::from_str::<serde_json::Value>(
                        &format!(r#"[{}]"#, #required_json)
                    ).unwrap_or_default()
                })
            }

            async fn call(&self, input: dashflow::tools::ToolInput) -> dashflow::Result<String> {
                // Default implementation - users should implement their own call method
                // by overriding this in their impl block
                Err(dashflow::Error::Tool {
                    name: self.name().to_string(),
                    message: "Tool call not implemented. Override the call method in your impl block.".to_string(),
                })
            }
        }

        impl #struct_name {
            /// Parse tool input into this struct
            pub fn from_input(input: &dashflow::tools::ToolInput) -> dashflow::Result<Self>
            where
                Self: serde::de::DeserializeOwned,
            {
                match input {
                    dashflow::tools::ToolInput::Structured(map) => {
                        serde_json::from_value(serde_json::Value::Object(map.clone()))
                            .map_err(|e| dashflow::Error::Tool {
                                name: #tool_name.to_string(),
                                message: format!("Failed to parse input: {}", e),
                            })
                    }
                    dashflow::tools::ToolInput::String(s) => {
                        serde_json::from_str(s)
                            .map_err(|e| dashflow::Error::Tool {
                                name: #tool_name.to_string(),
                                message: format!("Failed to parse string input as JSON: {}", e),
                            })
                    }
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert PascalCase to snake_case
fn to_snake_case(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                result.push('_');
            }
            // SAFETY: char::to_lowercase() always produces at least one character
            #[allow(clippy::unwrap_used)]
            result.push(c.to_lowercase().next().unwrap());
        } else {
            result.push(c);
        }
    }
    result
}

/// Get JSON schema type from Rust type string
fn get_json_type(type_str: &str) -> (&'static str, bool) {
    let type_str = type_str.replace(' ', "");

    // Check for Option<T>
    if type_str.starts_with("Option<") {
        let inner = &type_str[7..type_str.len() - 1];
        let (inner_type, _) = get_json_type(inner);
        return (inner_type, true);
    }

    // Map Rust types to JSON schema types
    match type_str.as_str() {
        "String" | "&str" | "std::string::String" => ("string", false),
        "i8" | "i16" | "i32" | "i64" | "isize" | "u8" | "u16" | "u32" | "u64" | "usize" => {
            ("integer", false)
        }
        "f32" | "f64" => ("number", false),
        "bool" => ("boolean", false),
        _ if type_str.starts_with("Vec<") => ("array", false),
        _ if type_str.starts_with("HashMap<") || type_str.starts_with("BTreeMap<") => {
            ("object", false)
        }
        _ => ("string", false), // Default to string
    }
}

/// Escape string for JSON
fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // to_snake_case tests
    // ========================================================================

    #[test]
    fn test_to_snake_case_simple() {
        assert_eq!(to_snake_case("MyStruct"), "my_struct");
    }

    #[test]
    fn test_to_snake_case_single_word() {
        assert_eq!(to_snake_case("Hello"), "hello");
    }

    #[test]
    fn test_to_snake_case_multiple_capitals() {
        assert_eq!(to_snake_case("ReadFile"), "read_file");
        assert_eq!(to_snake_case("HTTPRequest"), "h_t_t_p_request");
    }

    #[test]
    fn test_to_snake_case_already_lowercase() {
        assert_eq!(to_snake_case("already"), "already");
    }

    #[test]
    fn test_to_snake_case_empty() {
        assert_eq!(to_snake_case(""), "");
    }

    #[test]
    fn test_to_snake_case_complex() {
        assert_eq!(to_snake_case("MyBigStructName"), "my_big_struct_name");
    }

    // ========================================================================
    // get_json_type tests
    // ========================================================================

    #[test]
    fn test_get_json_type_string() {
        assert_eq!(get_json_type("String"), ("string", false));
        assert_eq!(get_json_type("&str"), ("string", false));
        assert_eq!(get_json_type("std::string::String"), ("string", false));
    }

    #[test]
    fn test_get_json_type_integers() {
        assert_eq!(get_json_type("i32"), ("integer", false));
        assert_eq!(get_json_type("u64"), ("integer", false));
        assert_eq!(get_json_type("usize"), ("integer", false));
        assert_eq!(get_json_type("i8"), ("integer", false));
    }

    #[test]
    fn test_get_json_type_floats() {
        assert_eq!(get_json_type("f32"), ("number", false));
        assert_eq!(get_json_type("f64"), ("number", false));
    }

    #[test]
    fn test_get_json_type_bool() {
        assert_eq!(get_json_type("bool"), ("boolean", false));
    }

    #[test]
    fn test_get_json_type_vec() {
        assert_eq!(get_json_type("Vec<String>"), ("array", false));
        assert_eq!(get_json_type("Vec<i32>"), ("array", false));
    }

    #[test]
    fn test_get_json_type_maps() {
        assert_eq!(get_json_type("HashMap<String, i32>"), ("object", false));
        assert_eq!(get_json_type("BTreeMap<String, String>"), ("object", false));
    }

    #[test]
    fn test_get_json_type_option() {
        assert_eq!(get_json_type("Option<String>"), ("string", true));
        assert_eq!(get_json_type("Option<i32>"), ("integer", true));
        assert_eq!(get_json_type("Option<Vec<String>>"), ("array", true));
    }

    #[test]
    fn test_get_json_type_unknown() {
        // Unknown types default to string
        assert_eq!(get_json_type("MyCustomType"), ("string", false));
    }

    #[test]
    fn test_get_json_type_with_spaces() {
        // Function handles spaces in type strings
        assert_eq!(get_json_type("Option < String >"), ("string", true));
    }

    // ========================================================================
    // escape_json tests
    // ========================================================================

    #[test]
    fn test_escape_json_no_escaping() {
        assert_eq!(escape_json("hello world"), "hello world");
    }

    #[test]
    fn test_escape_json_quotes() {
        assert_eq!(escape_json(r#"say "hello""#), r#"say \"hello\""#);
    }

    #[test]
    fn test_escape_json_backslash() {
        assert_eq!(escape_json(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn test_escape_json_newline() {
        assert_eq!(escape_json("line1\nline2"), "line1\\nline2");
    }

    #[test]
    fn test_escape_json_carriage_return() {
        assert_eq!(escape_json("line1\rline2"), "line1\\rline2");
    }

    #[test]
    fn test_escape_json_tab() {
        assert_eq!(escape_json("col1\tcol2"), "col1\\tcol2");
    }

    #[test]
    fn test_escape_json_combined() {
        assert_eq!(
            escape_json("Say \"hello\"\nand\tbye\\"),
            "Say \\\"hello\\\"\\nand\\tbye\\\\"
        );
    }

    #[test]
    fn test_escape_json_empty() {
        assert_eq!(escape_json(""), "");
    }

    // ========================================================================
    // MergeStrategy tests
    // ========================================================================

    #[test]
    fn test_merge_strategy_eq() {
        assert_eq!(MergeStrategy::Default, MergeStrategy::Default);
        assert_eq!(MergeStrategy::Skip, MergeStrategy::Skip);
        assert_eq!(MergeStrategy::Replace, MergeStrategy::Replace);
        assert_eq!(MergeStrategy::TakeIfEmpty, MergeStrategy::TakeIfEmpty);
        assert_eq!(MergeStrategy::Recursive, MergeStrategy::Recursive);
    }

    #[test]
    fn test_merge_strategy_ne() {
        assert_ne!(MergeStrategy::Default, MergeStrategy::Skip);
        assert_ne!(MergeStrategy::Replace, MergeStrategy::TakeIfEmpty);
    }

    #[test]
    fn test_merge_strategy_clone() {
        let strategy = MergeStrategy::Recursive;
        let cloned = strategy;
        assert_eq!(strategy, cloned);
    }

    #[test]
    fn test_merge_strategy_debug() {
        let debug_str = format!("{:?}", MergeStrategy::Default);
        assert!(debug_str.contains("Default"));
    }
}
