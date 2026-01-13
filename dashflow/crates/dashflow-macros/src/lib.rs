// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Procedural macros for dashflow
//!
//! This crate provides derive macros for `DashFlow` state management:
//! - `GraphState`: Derive macro for state structs with field reducers
//! - `#[tool]`: Attribute macro for creating tools from functions
//!
//! # Example - GraphState
//! ```ignore
//! use dashflow_macros::GraphState;
//! use dashflow::core::messages::Message;
//!
//! #[derive(GraphState, Clone)]
//! struct AgentState {
//!     #[add_messages]
//!     messages: Vec<Message>,
//!
//!     #[reducer(concat_strings)]
//!     log: String,
//! }
//!
//! fn concat_strings(left: String, right: String) -> String {
//!     format!("{}\n{}", left, right)
//! }
//! ```
//!
//! # Example - Tool Macro
//! ```ignore
//! use dashflow_macros::tool;
//! use serde::Deserialize;
//! use schemars::JsonSchema;
//!
//! #[derive(Debug, Deserialize, JsonSchema)]
//! struct WeatherArgs {
//!     /// City name
//!     city: String,
//! }
//!
//! /// Get current weather for a city
//! #[tool]
//! async fn get_weather(args: WeatherArgs) -> Result<String, String> {
//!     Ok(format!("Weather in {}: Sunny", args.city))
//! }
//!
//! // Use it:
//! let tool = get_weather();
//! let result = tool._call(ToolInput::Structured(json!({"city": "NYC"}))).await;
//! ```

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parse,
    parse_macro_input,
    punctuated::Punctuated,
    Data,
    DeriveInput,
    Expr,
    ExprLit,
    ExprPath,
    Fields,
    ItemFn,
    Lit,
    Meta,
    Token,
};

/// Derive macro for graph state with field reducers
///
/// This macro generates implementations for state merging based on field attributes:
/// - `#[add_messages]`: Use the `add_messages` reducer for `Vec<Message>` fields
/// - `#[reducer(fn_name)]`: Use a custom reducer function
///
/// # Example
/// ```ignore
/// #[derive(GraphState, Clone)]
/// struct AgentState {
///     #[add_messages]
///     messages: Vec<Message>,
/// }
/// ```
// Proc-macro: unwrap() is acceptable since panics become compile errors for invalid input.
// - field.ident.as_ref().unwrap(): Named struct fields always have identifiers (validated by match)
#[allow(clippy::unwrap_used)]
#[proc_macro_derive(GraphState, attributes(add_messages, reducer))]
pub fn derive_graph_state(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let name = &input.ident;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Parse struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input.ident,
                    "GraphState can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into()
            }
        },
        _ => {
            return syn::Error::new_spanned(
                &input.ident,
                "GraphState can only be derived for structs",
            )
            .to_compile_error()
            .into()
        }
    };

    // Generate merge implementations for each field
    let merge_fields = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();

        // Check for #[add_messages] attribute
        let has_add_messages = field
            .attrs
            .iter()
            .any(|attr| attr.path().get_ident().is_some_and(|i| i == "add_messages"));

        // Check for #[reducer(fn_name)] attribute
        let custom_reducer = field.attrs.iter().find_map(|attr| {
            if attr.path().get_ident().is_some_and(|i| i == "reducer") {
                if let Meta::List(meta_list) = &attr.meta {
                    // Parse the function name from the tokens
                    let tokens = &meta_list.tokens;
                    return Some(quote! { #tokens });
                }
            }
            None
        });

        if has_add_messages {
            // Use add_messages reducer via the __private module
            // This works both for internal tests and external usage
            quote! {
                #field_name: ::dashflow::__private::reducer::add_messages(
                    self.#field_name.clone(),
                    partial.#field_name.clone()
                )
            }
        } else if let Some(reducer_fn) = custom_reducer {
            // Use custom reducer function
            quote! {
                #field_name: #reducer_fn(
                    self.#field_name.clone(),
                    partial.#field_name.clone()
                )
            }
        } else {
            // Default: use value from partial (right side wins)
            quote! {
                #field_name: partial.#field_name.clone()
            }
        }
    });

    // Generate the merge_partial method
    let expanded = quote! {
        impl #impl_generics #name #ty_generics #where_clause {
            /// Merge a partial state update into this state
            ///
            /// This method is automatically generated by the `GraphState` derive macro.
            /// It applies field-specific reducers based on the attributes:
            /// - `#[add_messages]`: Merges message lists with ID-based deduplication
            /// - `#[reducer(fn)]`: Applies custom reducer function
            /// - No attribute: Uses value from partial (right side wins)
            pub fn merge_partial(&self, partial: &Self) -> Self {
                Self {
                    #(#merge_fields),*
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Attribute macro for creating tools from functions
///
/// This macro transforms a function into a tool factory that returns a `StructuredTool`.
/// The function's doc comment becomes the tool description, and the argument type
/// is used to generate the JSON Schema for tool parameters.
///
/// # Requirements
///
/// - The function must take exactly one argument that implements `Deserialize` and `JsonSchema`
/// - The function must return `Result<String, String>` (sync) or `impl Future<Output = Result<String, String>>` (async)
/// - The argument type must derive both `serde::Deserialize` and `schemars::JsonSchema`
///
/// # Example
///
/// ```ignore
/// use dashflow_macros::tool;
/// use serde::Deserialize;
/// use schemars::JsonSchema;
///
/// #[derive(Debug, Deserialize, JsonSchema)]
/// struct CalculatorArgs {
///     /// First number
///     a: f64,
///     /// Second number
///     b: f64,
///     /// Operation: add, subtract, multiply, divide
///     operation: String,
/// }
///
/// /// Performs basic arithmetic operations
/// #[tool]
/// fn calculator(args: CalculatorArgs) -> Result<String, String> {
///     let result = match args.operation.as_str() {
///         "add" => args.a + args.b,
///         "subtract" => args.a - args.b,
///         "multiply" => args.a * args.b,
///         "divide" => args.a / args.b,
///         _ => return Err(format!("Unknown operation: {}", args.operation)),
///     };
///     Ok(result.to_string())
/// }
///
/// // The macro generates a factory function:
/// let tool = calculator();  // Returns impl Tool
/// assert_eq!(tool.name(), "calculator");
/// ```
#[proc_macro_attribute]
pub fn tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Extract function components
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let vis = &input_fn.vis;
    let attrs = &input_fn.attrs;
    let is_async = input_fn.sig.asyncness.is_some();
    let fn_block = &input_fn.block;

    // Extract doc comment for the description
    let description = attrs
        .iter()
        .filter_map(|attr| {
            if attr.path().is_ident("doc") {
                if let Meta::NameValue(meta) = &attr.meta {
                    if let syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(s),
                        ..
                    }) = &meta.value
                    {
                        return Some(s.value().trim().to_string());
                    }
                }
            }
            None
        })
        .collect::<Vec<_>>()
        .join(" ");

    let description = if description.is_empty() {
        format!("Tool function: {}", fn_name_str)
    } else {
        description
    };

    // Extract the argument type
    let inputs = &input_fn.sig.inputs;
    if inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "#[tool] function must have exactly one argument (the args struct)",
        )
        .to_compile_error()
        .into();
    }

    // SAFETY: length validated to be exactly 1 above
    #[allow(clippy::indexing_slicing)] // length checked above
    let arg = &inputs[0];
    let (arg_name, arg_type) = match arg {
        syn::FnArg::Typed(pat_type) => {
            let name = &pat_type.pat;
            let ty = &pat_type.ty;
            (name, ty)
        }
        _ => {
            return syn::Error::new_spanned(arg, "#[tool] function cannot use self parameter")
                .to_compile_error()
                .into();
        }
    };

    // Generate the tool factory function
    // For async functions, we need to handle them differently
    //
    // Note: We use schemars::schema_for! macro to generate JSON Schema.
    // This is the schemars 1.x API. The generated schema is then converted
    // to serde_json::Value.
    let generated = if is_async {
        quote! {
            #(#attrs)*
            #vis fn #fn_name() -> impl ::dashflow::core::tools::Tool {
                // Inner async function that does the actual work
                async fn __inner_impl(#arg_name: #arg_type) -> ::std::result::Result<String, String>
                    #fn_block

                // Generate schema using schemars 1.x API
                let schema = ::schemars::schema_for!(#arg_type);
                let schema_value: ::serde_json::Value = ::serde_json::to_value(schema)
                    .unwrap_or_else(|_| ::serde_json::json!({"type": "object"}));

                ::dashflow::core::tools::StructuredTool::new(
                    #fn_name_str,
                    #description,
                    schema_value,
                    |#arg_name: #arg_type| __inner_impl(#arg_name)
                )
            }
        }
    } else {
        quote! {
            #(#attrs)*
            #vis fn #fn_name() -> impl ::dashflow::core::tools::Tool {
                // Inner sync function that does the actual work
                fn __inner_impl(#arg_name: #arg_type) -> ::std::result::Result<String, String>
                    #fn_block

                // Generate schema using schemars 1.x API
                let schema = ::schemars::schema_for!(#arg_type);
                let schema_value: ::serde_json::Value = ::serde_json::to_value(schema)
                    .unwrap_or_else(|_| ::serde_json::json!({"type": "object"}));

                ::dashflow::core::tools::sync_structured_tool(
                    #fn_name_str,
                    #description,
                    schema_value,
                    |#arg_name: #arg_type| -> ::std::result::Result<String, String> {
                        __inner_impl(#arg_name)
                    }
                )
            }
        }
    };

    TokenStream::from(generated)
}

#[derive(Debug)]
struct CapabilityArgs {
    tags: Vec<String>,
}

impl Parse for CapabilityArgs {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let args = Punctuated::<Expr, Token![,]>::parse_terminated(input)?;

        let mut tags = Vec::new();
        for expr in args {
            let tag = match expr {
                Expr::Lit(ExprLit {
                    lit: Lit::Str(s), ..
                }) => s.value(),
                Expr::Path(ExprPath { path, .. }) if path.segments.len() == 1 => path
                    .segments
                    .first()
                    .ok_or_else(|| {
                        syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "expected a single-segment identifier",
                        )
                    })?
                    .ident
                    .to_string(),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "expected a string literal (\"tag\") or an identifier (tag)",
                    ));
                }
            };

            let tag = tag.trim().to_string();
            if tag.is_empty() {
                return Err(syn::Error::new(proc_macro2::Span::call_site(), "tag cannot be empty"));
            }
            if tag.chars().any(char::is_whitespace) {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "tag cannot contain whitespace",
                ));
            }

            tags.push(tag);
        }

        if tags.is_empty() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[capability(...)] requires at least one tag",
            ));
        }

        Ok(Self { tags })
    }
}

/// Attribute macro for annotating platform types with capability tags.
///
/// This is a no-op macro used for static analysis and source discovery (e.g., module discovery and
/// linting), allowing types to declare what they provide.
///
/// Accepted forms:
/// - `#[dashflow::capability(\"bm25\", \"retriever\")]`
/// - `#[dashflow::capability(bm25, retriever)]`
#[proc_macro_attribute]
pub fn capability(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as CapabilityArgs);
    let _tag_count = args.tags.len();
    item
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::quote;

    // Helper to parse CapabilityArgs from tokens
    fn parse_capability_args(tokens: TokenStream2) -> syn::Result<CapabilityArgs> {
        syn::parse2::<CapabilityArgs>(tokens)
    }

    // ========================================================================
    // CapabilityArgs parsing tests
    // ========================================================================

    mod capability_args_parsing {
        use super::*;

        #[test]
        fn test_single_string_tag() {
            let args = parse_capability_args(quote! { "retriever" }).unwrap();
            assert_eq!(args.tags.len(), 1);
            assert_eq!(args.tags[0], "retriever");
        }

        #[test]
        fn test_multiple_string_tags() {
            let args = parse_capability_args(quote! { "bm25", "retriever", "search" }).unwrap();
            assert_eq!(args.tags.len(), 3);
            assert_eq!(args.tags[0], "bm25");
            assert_eq!(args.tags[1], "retriever");
            assert_eq!(args.tags[2], "search");
        }

        #[test]
        fn test_single_identifier_tag() {
            let args = parse_capability_args(quote! { retriever }).unwrap();
            assert_eq!(args.tags.len(), 1);
            assert_eq!(args.tags[0], "retriever");
        }

        #[test]
        fn test_multiple_identifier_tags() {
            let args = parse_capability_args(quote! { bm25, retriever, search }).unwrap();
            assert_eq!(args.tags.len(), 3);
            assert_eq!(args.tags[0], "bm25");
            assert_eq!(args.tags[1], "retriever");
            assert_eq!(args.tags[2], "search");
        }

        #[test]
        fn test_mixed_string_and_identifier_tags() {
            let args = parse_capability_args(quote! { "bm25", retriever, "search" }).unwrap();
            assert_eq!(args.tags.len(), 3);
            assert_eq!(args.tags[0], "bm25");
            assert_eq!(args.tags[1], "retriever");
            assert_eq!(args.tags[2], "search");
        }

        #[test]
        fn test_string_tag_with_whitespace_trimmed() {
            let args = parse_capability_args(quote! { "  retriever  " }).unwrap();
            assert_eq!(args.tags[0], "retriever");
        }

        #[test]
        fn test_empty_args_error() {
            let result = parse_capability_args(quote! {});
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("requires at least one tag"));
        }

        #[test]
        fn test_empty_string_tag_error() {
            let result = parse_capability_args(quote! { "" });
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("tag cannot be empty"));
        }

        #[test]
        fn test_whitespace_only_string_tag_error() {
            let result = parse_capability_args(quote! { "   " });
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("tag cannot be empty"));
        }

        #[test]
        fn test_tag_with_internal_whitespace_error() {
            let result = parse_capability_args(quote! { "hello world" });
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(err.to_string().contains("tag cannot contain whitespace"));
        }

        #[test]
        fn test_multiple_tags_with_one_invalid_error() {
            let result = parse_capability_args(quote! { "valid", "also valid with spaces" });
            assert!(result.is_err());
            // The second tag has spaces, so it should error
            let err = result.unwrap_err();
            assert!(err.to_string().contains("tag cannot contain whitespace"));
        }

        #[test]
        fn test_underscore_in_tag_ok() {
            let args = parse_capability_args(quote! { "vector_store" }).unwrap();
            assert_eq!(args.tags[0], "vector_store");
        }

        #[test]
        fn test_hyphen_in_tag_ok() {
            let args = parse_capability_args(quote! { "bm25-retriever" }).unwrap();
            assert_eq!(args.tags[0], "bm25-retriever");
        }

        #[test]
        fn test_numbers_in_tag_ok() {
            let args = parse_capability_args(quote! { "version2" }).unwrap();
            assert_eq!(args.tags[0], "version2");
        }

        #[test]
        fn test_all_numeric_tag_ok() {
            let args = parse_capability_args(quote! { "12345" }).unwrap();
            assert_eq!(args.tags[0], "12345");
        }

        #[test]
        fn test_special_characters_in_tag_ok() {
            let args = parse_capability_args(quote! { "tag:with:colons" }).unwrap();
            assert_eq!(args.tags[0], "tag:with:colons");
        }

        #[test]
        fn test_path_expression_error() {
            // std::vec should fail (multi-segment path)
            let result = parse_capability_args(quote! { std::vec });
            assert!(result.is_err());
        }

        #[test]
        fn test_many_tags() {
            let args = parse_capability_args(
                quote! { "a", "b", "c", "d", "e", "f", "g", "h", "i", "j" }
            ).unwrap();
            assert_eq!(args.tags.len(), 10);
        }

        #[test]
        fn test_duplicate_tags_allowed() {
            // The macro doesn't deduplicate - that's caller's responsibility
            let args = parse_capability_args(quote! { "dupe", "dupe", "dupe" }).unwrap();
            assert_eq!(args.tags.len(), 3);
            assert!(args.tags.iter().all(|t| t == "dupe"));
        }

        #[test]
        fn test_unicode_tag_ok() {
            let args = parse_capability_args(quote! { "日本語" }).unwrap();
            assert_eq!(args.tags[0], "日本語");
        }

        #[test]
        fn test_emoji_tag_ok() {
            let args = parse_capability_args(quote! { "rocket🚀" }).unwrap();
            assert_eq!(args.tags[0], "rocket🚀");
        }
    }

    // ========================================================================
    // doc comment extraction tests (testing internal logic patterns)
    // ========================================================================

    mod doc_comment_extraction {
        use super::*;

        // Helper to simulate doc comment extraction logic from the tool macro
        fn extract_description(attrs: &[syn::Attribute]) -> String {
            attrs
                .iter()
                .filter_map(|attr| {
                    if attr.path().is_ident("doc") {
                        if let Meta::NameValue(meta) = &attr.meta {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(s),
                                ..
                            }) = &meta.value
                            {
                                return Some(s.value().trim().to_string());
                            }
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
                .join(" ")
        }

        #[test]
        fn test_single_doc_comment() {
            let item: syn::ItemFn = syn::parse2(quote! {
                /// This is a description
                fn my_tool() {}
            }).unwrap();
            let desc = extract_description(&item.attrs);
            assert_eq!(desc, "This is a description");
        }

        #[test]
        fn test_multi_line_doc_comment() {
            let item: syn::ItemFn = syn::parse2(quote! {
                /// First line
                /// Second line
                /// Third line
                fn my_tool() {}
            }).unwrap();
            let desc = extract_description(&item.attrs);
            assert_eq!(desc, "First line Second line Third line");
        }

        #[test]
        fn test_no_doc_comment() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool() {}
            }).unwrap();
            let desc = extract_description(&item.attrs);
            assert_eq!(desc, "");
        }

        #[test]
        fn test_doc_comment_with_leading_trailing_whitespace() {
            let item: syn::ItemFn = syn::parse2(quote! {
                ///   Trimmed description
                fn my_tool() {}
            }).unwrap();
            let desc = extract_description(&item.attrs);
            assert_eq!(desc, "Trimmed description");
        }

        #[test]
        fn test_other_attributes_ignored() {
            let item: syn::ItemFn = syn::parse2(quote! {
                #[inline]
                #[allow(unused)]
                /// The actual description
                fn my_tool() {}
            }).unwrap();
            let desc = extract_description(&item.attrs);
            assert_eq!(desc, "The actual description");
        }
    }

    // ========================================================================
    // function signature validation tests (testing patterns used in macros)
    // ========================================================================

    mod function_signature_validation {
        use super::*;

        fn is_async_fn(item: &syn::ItemFn) -> bool {
            item.sig.asyncness.is_some()
        }

        fn has_single_typed_arg(item: &syn::ItemFn) -> bool {
            if item.sig.inputs.len() != 1 {
                return false;
            }
            matches!(&item.sig.inputs[0], syn::FnArg::Typed(_))
        }

        fn has_self_param(item: &syn::ItemFn) -> bool {
            item.sig.inputs.iter().any(|arg| matches!(arg, syn::FnArg::Receiver(_)))
        }

        #[test]
        fn test_sync_function_detection() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(!is_async_fn(&item));
        }

        #[test]
        fn test_async_function_detection() {
            let item: syn::ItemFn = syn::parse2(quote! {
                async fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(is_async_fn(&item));
        }

        #[test]
        fn test_single_typed_arg() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(has_single_typed_arg(&item));
        }

        #[test]
        fn test_no_args() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool() -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(!has_single_typed_arg(&item));
        }

        #[test]
        fn test_multiple_args() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool(a: A, b: B) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(!has_single_typed_arg(&item));
        }

        #[test]
        fn test_self_param_detection() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_method(&self, args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(has_self_param(&item));
        }

        #[test]
        fn test_no_self_param() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(!has_self_param(&item));
        }
    }

    // ========================================================================
    // struct field attribute detection tests (for GraphState derive)
    // ========================================================================

    mod field_attribute_detection {
        use super::*;

        fn has_add_messages_attr(attrs: &[syn::Attribute]) -> bool {
            attrs.iter().any(|attr| attr.path().get_ident().is_some_and(|i| i == "add_messages"))
        }

        fn get_reducer_fn(attrs: &[syn::Attribute]) -> Option<String> {
            attrs.iter().find_map(|attr| {
                if attr.path().get_ident().is_some_and(|i| i == "reducer") {
                    if let Meta::List(meta_list) = &attr.meta {
                        return Some(meta_list.tokens.to_string());
                    }
                }
                None
            })
        }

        // Helper to extract first field from a struct
        fn get_first_field(input: &syn::DeriveInput) -> Option<&syn::Field> {
            if let Data::Struct(data) = &input.data {
                if let Fields::Named(fields) = &data.fields {
                    return fields.named.first();
                }
            }
            None
        }

        #[test]
        fn test_add_messages_attribute_detection() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    #[add_messages]
                    messages: Vec<Message>,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            assert!(has_add_messages_attr(&field.attrs));
        }

        #[test]
        fn test_no_add_messages_attribute() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    messages: Vec<Message>,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            assert!(!has_add_messages_attr(&field.attrs));
        }

        #[test]
        fn test_reducer_attribute_extraction() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    #[reducer(concat_strings)]
                    log: String,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            let reducer = get_reducer_fn(&field.attrs);
            assert_eq!(reducer, Some("concat_strings".to_string()));
        }

        #[test]
        fn test_reducer_with_path() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    #[reducer(my_module::concat)]
                    log: String,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            let reducer = get_reducer_fn(&field.attrs);
            assert_eq!(reducer, Some("my_module :: concat".to_string()));
        }

        #[test]
        fn test_no_reducer_attribute() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    count: i32,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            let reducer = get_reducer_fn(&field.attrs);
            assert!(reducer.is_none());
        }

        #[test]
        fn test_multiple_attributes_on_field() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct Test {
                    #[serde(skip)]
                    #[add_messages]
                    #[doc = "Messages"]
                    messages: Vec<Message>,
                }
            }).unwrap();
            let field = get_first_field(&input).unwrap();
            assert!(has_add_messages_attr(&field.attrs));
        }
    }

    // ========================================================================
    // struct validation tests (for GraphState derive)
    // ========================================================================

    mod struct_validation {
        use super::*;

        fn is_named_struct(input: &syn::DeriveInput) -> bool {
            matches!(&input.data, Data::Struct(data) if matches!(&data.fields, Fields::Named(_)))
        }

        fn is_struct(input: &syn::DeriveInput) -> bool {
            matches!(&input.data, Data::Struct(_))
        }

        #[test]
        fn test_named_struct_valid() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State {
                    field: i32,
                }
            }).unwrap();
            assert!(is_struct(&input));
            assert!(is_named_struct(&input));
        }

        #[test]
        fn test_tuple_struct_invalid() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State(i32, String);
            }).unwrap();
            assert!(is_struct(&input));
            assert!(!is_named_struct(&input));
        }

        #[test]
        fn test_unit_struct_invalid() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State;
            }).unwrap();
            assert!(is_struct(&input));
            assert!(!is_named_struct(&input));
        }

        #[test]
        fn test_enum_invalid() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                enum State {
                    A,
                    B,
                }
            }).unwrap();
            assert!(!is_struct(&input));
        }

        #[test]
        fn test_struct_with_generics() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State<T> {
                    field: T,
                }
            }).unwrap();
            assert!(is_named_struct(&input));
            assert!(!input.generics.params.is_empty());
        }

        #[test]
        fn test_struct_with_lifetime() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State<'a> {
                    field: &'a str,
                }
            }).unwrap();
            assert!(is_named_struct(&input));
        }

        #[test]
        fn test_struct_with_where_clause() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct State<T> where T: Clone {
                    field: T,
                }
            }).unwrap();
            assert!(is_named_struct(&input));
            assert!(input.generics.where_clause.is_some());
        }
    }

    // ========================================================================
    // generated code pattern tests
    // ========================================================================

    mod code_generation_patterns {
        use super::*;

        #[test]
        fn test_quote_merge_field_default() {
            // Test that the default merge pattern (right side wins) generates correctly
            let field_name = quote::format_ident!("count");
            let generated = quote! {
                #field_name: partial.#field_name.clone()
            };
            let expected = "count : partial . count . clone ()";
            assert_eq!(generated.to_string(), expected);
        }

        #[test]
        fn test_quote_merge_field_with_reducer() {
            let field_name = quote::format_ident!("messages");
            let reducer_fn = quote! { add_messages };
            let generated = quote! {
                #field_name: #reducer_fn(
                    self.#field_name.clone(),
                    partial.#field_name.clone()
                )
            };
            assert!(generated.to_string().contains("add_messages"));
            assert!(generated.to_string().contains("self . messages . clone ()"));
            assert!(generated.to_string().contains("partial . messages . clone ()"));
        }

        #[test]
        fn test_quote_tool_name_extraction() {
            let fn_name = quote::format_ident!("get_weather");
            let fn_name_str = fn_name.to_string();
            assert_eq!(fn_name_str, "get_weather");
        }

        #[test]
        fn test_quote_empty_description_fallback() {
            let fn_name_str = "my_tool";
            let description = String::new();
            let final_desc = if description.is_empty() {
                format!("Tool function: {}", fn_name_str)
            } else {
                description
            };
            assert_eq!(final_desc, "Tool function: my_tool");
        }
    }

    // ========================================================================
    // edge cases and error handling
    // ========================================================================

    mod edge_cases {
        use super::*;

        #[test]
        fn test_empty_struct() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct EmptyState {}
            }).unwrap();
            if let Data::Struct(data) = &input.data {
                if let Fields::Named(fields) = &data.fields {
                    assert!(fields.named.is_empty());
                }
            }
        }

        #[test]
        fn test_struct_with_many_fields() {
            let input: syn::DeriveInput = syn::parse2(quote! {
                struct LargeState {
                    field1: i32,
                    field2: i32,
                    field3: i32,
                    field4: i32,
                    field5: i32,
                    field6: i32,
                    field7: i32,
                    field8: i32,
                    field9: i32,
                    field10: i32,
                }
            }).unwrap();
            if let Data::Struct(data) = &input.data {
                if let Fields::Named(fields) = &data.fields {
                    assert_eq!(fields.named.len(), 10);
                }
            }
        }

        #[test]
        fn test_visibility_preserved() {
            let item: syn::ItemFn = syn::parse2(quote! {
                pub fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(matches!(item.vis, syn::Visibility::Public(_)));
        }

        #[test]
        fn test_private_visibility() {
            let item: syn::ItemFn = syn::parse2(quote! {
                fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(matches!(item.vis, syn::Visibility::Inherited));
        }

        #[test]
        fn test_crate_visibility() {
            let item: syn::ItemFn = syn::parse2(quote! {
                pub(crate) fn my_tool(args: Args) -> Result<String, String> { Ok("".into()) }
            }).unwrap();
            assert!(matches!(item.vis, syn::Visibility::Restricted(_)));
        }
    }

    // ========================================================================
    // selector escaping tests (for reference - similar pattern used elsewhere)
    // ========================================================================

    mod string_escaping {
        #[test]
        fn test_single_quote_escaping() {
            let selector = "button[data-attr='value']";
            let escaped = selector.replace('\'', "\\'");
            assert_eq!(escaped, "button[data-attr=\\'value\\']");
        }

        #[test]
        fn test_no_escaping_needed() {
            let selector = "button.class";
            let escaped = selector.replace('\'', "\\'");
            assert_eq!(escaped, "button.class");
        }

        #[test]
        fn test_multiple_quotes() {
            let selector = "'first' and 'second'";
            let escaped = selector.replace('\'', "\\'");
            assert_eq!(escaped, "\\'first\\' and \\'second\\'");
        }
    }
}
