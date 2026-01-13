//! Serialization and persistence for DashFlow objects
//!
//! This module provides a serialization system for DashFlow Runnables and other
//! objects, enabling chains to be saved to and loaded from JSON/YAML files.
//!
//! # Architecture
//!
//! The serialization system uses a **constructor-based approach** matching Python `DashFlow`:
//!
//! 1. Objects implement the `Serializable` trait
//! 2. `to_json()` returns a `SerializedObject` with:
//!    - `lc`: Version number (currently 1)
//!    - `type`: "constructor" (object can be reconstructed)
//!    - `id`: Unique identifier like `["dashflow", "prompts", "PromptTemplate"]`
//!    - `kwargs`: Constructor arguments as JSON
//! 3. Secrets are excluded from serialization (replaced with env var references)
//! 4. Non-serializable objects return `SerializedNotImplemented`
//!
//! # Example
//!
//! ```rust
//! use dashflow::core::serialization::Serializable;
//! use dashflow::core::prompts::PromptTemplate;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let prompt = PromptTemplate::from_template("Hello {name}!")?;
//!
//! // Serialize to JSON
//! let json = prompt.to_json_value()?;
//! println!("{}", serde_json::to_string_pretty(&json)?);
//! // {
//! //   "lc": 1,
//! //   "type": "constructor",
//! //   "id": ["dashflow", "prompts", "PromptTemplate"],
//! //   "kwargs": {
//! //     "template": "Hello {name}!"
//! //   }
//! // }
//!
//! // Round-trip: deserialize back to config
//! // (Full deserialization requires registry - see config_loader module)
//! # Ok(())
//! # }
//! ```
//!
//! # Limitations
//!
//! The following cannot be serialized and will return `SerializedNotImplemented`:
//!
//! - **Closures and function pointers** (`RunnableLambda` with custom Rust functions)
//! - **Trait objects without type info** (type-erased Runnables)
//! - **External resources** (database connections, file handles)
//!
//! For these cases, consider using the `config_loader` system with custom
//! constructors registered at runtime.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current serialization format version
pub const SERIALIZATION_VERSION: u32 = 1;

/// A serializable DashFlow object
///
/// This trait is implemented by objects that can be serialized to JSON
/// and reconstructed from their constructor arguments.
///
/// # Design Philosophy
///
/// Unlike Rust's standard `Serialize` trait which serializes the current state,
/// `Serializable` serializes the *constructor arguments* needed to recreate the object.
/// This enables:
///
/// - **Portability**: Configs work across Rust versions
/// - **Security**: Secrets are excluded
/// - **Compatibility**: Matches Python `DashFlow`'s serialization format
///
/// # Example
///
/// ```rust
/// use dashflow::core::serialization::{Serializable, SerializedObject, SERIALIZATION_VERSION};
/// use serde_json::json;
/// use std::collections::HashMap;
///
/// struct MyRunnable {
///     name: String,
///     temperature: f32,
/// }
///
/// impl Serializable for MyRunnable {
///     fn lc_id(&self) -> Vec<String> {
///         vec!["dashflow".to_string(), "my".to_string(), "MyRunnable".to_string()]
///     }
///
///     fn is_lc_serializable(&self) -> bool {
///         true
///     }
///
///     fn to_json(&self) -> SerializedObject {
///         let mut kwargs = serde_json::Map::new();
///         kwargs.insert("name".to_string(), json!(self.name));
///         kwargs.insert("temperature".to_string(), json!(self.temperature));
///
///         SerializedObject::Constructor {
///             lc: SERIALIZATION_VERSION,
///             id: self.lc_id(),
///             kwargs: kwargs.into(),
///         }
///     }
///
///     fn lc_secrets(&self) -> HashMap<String, String> {
///         HashMap::new()
///     }
/// }
/// ```
pub trait Serializable {
    /// Get the unique identifier for this object type
    ///
    /// The identifier is a path like `["dashflow", "prompts", "PromptTemplate"]`
    /// used during deserialization to find the correct constructor.
    ///
    /// # Convention
    ///
    /// - First element: Package name ("dashflow", "`dashflow_core`", etc.)
    /// - Middle elements: Module path
    /// - Last element: Type name
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::core::serialization::Serializable;
    /// # struct PromptTemplate;
    /// # impl Serializable for PromptTemplate {
    /// fn lc_id(&self) -> Vec<String> {
    ///     vec!["dashflow_core".into(), "prompts".into(), "PromptTemplate".into()]
    /// }
    /// #   fn is_lc_serializable(&self) -> bool { true }
    /// #   fn to_json(&self) -> dashflow::core::serialization::SerializedObject {
    /// #       dashflow::core::serialization::SerializedObject::not_implemented("PromptTemplate")
    /// #   }
    /// #   fn lc_secrets(&self) -> std::collections::HashMap<String, String> { std::collections::HashMap::new() }
    /// # }
    /// ```
    fn lc_id(&self) -> Vec<String>;

    /// Is this object serializable?
    ///
    /// Even if a type implements `Serializable`, it may not be serializable
    /// in all instances (e.g., if it contains closures or external resources).
    ///
    /// Default: `false` (must opt-in to serialization)
    fn is_lc_serializable(&self) -> bool {
        false
    }

    /// Serialize this object to a JSON-compatible representation
    ///
    /// Returns a `SerializedObject` which can be converted to JSON and
    /// used to reconstruct the object later.
    ///
    /// # Secrets
    ///
    /// Any fields listed in `lc_secrets()` must NOT be included in the
    /// serialized output. Instead, use environment variable references.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use dashflow::core::serialization::{Serializable, SerializedObject};
    /// # struct MyObject;
    /// # impl Serializable for MyObject {
    /// #   fn lc_id(&self) -> Vec<String> { vec![] }
    /// #   fn is_lc_serializable(&self) -> bool { false }
    /// fn to_json(&self) -> SerializedObject {
    ///     if !self.is_lc_serializable() {
    ///         return SerializedObject::not_implemented("MyObject");
    ///     }
    ///
    ///     SerializedObject::Constructor {
    ///         lc: 1,
    ///         id: self.lc_id(),
    ///         kwargs: serde_json::json!({"name": "example"}),
    ///     }
    /// }
    /// #   fn lc_secrets(&self) -> std::collections::HashMap<String, String> { std::collections::HashMap::new() }
    /// # }
    /// ```
    fn to_json(&self) -> SerializedObject;

    /// Map of constructor argument names to secret environment variable names
    ///
    /// Fields listed here will be excluded from serialization and replaced
    /// with `{"env": "VAR_NAME"}` references.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use std::collections::HashMap;
    /// # use dashflow::core::serialization::Serializable;
    /// # struct ChatOpenAI { api_key: String }
    /// # impl Serializable for ChatOpenAI {
    /// #   fn lc_id(&self) -> Vec<String> { vec![] }
    /// #   fn to_json(&self) -> dashflow::core::serialization::SerializedObject {
    /// #       dashflow::core::serialization::SerializedObject::not_implemented("ChatOpenAI")
    /// #   }
    /// fn lc_secrets(&self) -> HashMap<String, String> {
    ///     let mut secrets = HashMap::new();
    ///     secrets.insert("api_key".to_string(), "OPENAI_API_KEY".to_string());
    ///     secrets
    /// }
    /// #   fn is_lc_serializable(&self) -> bool { true }
    /// # }
    /// ```
    fn lc_secrets(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Convert to a `serde_json::Value` for easy serialization
    ///
    /// This is a convenience method that converts the `SerializedObject`
    /// to a JSON value that can be written to a file or sent over the network.
    fn to_json_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self.to_json())
    }

    /// Serialize to a JSON string
    ///
    /// # Arguments
    ///
    /// * `pretty` - If true, format with indentation (2 spaces)
    fn to_json_string(&self, pretty: bool) -> Result<String, serde_json::Error> {
        let value = self.to_json();
        if pretty {
            serde_json::to_string_pretty(&value)
        } else {
            serde_json::to_string(&value)
        }
    }
}

/// A serialized DashFlow object
///
/// This enum represents the different ways an object can be serialized.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SerializedObject {
    /// Object can be reconstructed from constructor arguments
    #[serde(rename = "constructor")]
    Constructor {
        /// Serialization format version
        lc: u32,

        /// Unique identifier for this object type
        ///
        /// Example: `["dashflow", "prompts", "PromptTemplate"]`
        id: Vec<String>,

        /// Constructor arguments
        ///
        /// These are the kwargs needed to reconstruct the object.
        /// Secrets should be replaced with `{"env": "VAR_NAME"}` references.
        kwargs: serde_json::Value,
    },

    /// Secret value (not serialized for security)
    #[serde(rename = "secret")]
    Secret {
        /// Serialization format version
        lc: u32,

        /// Unique identifier
        id: Vec<String>,
    },

    /// Object cannot be serialized
    #[serde(rename = "not_implemented")]
    NotImplemented {
        /// Serialization format version
        lc: u32,

        /// Unique identifier
        id: Vec<String>,

        /// Human-readable representation (for debugging)
        #[serde(skip_serializing_if = "Option::is_none")]
        repr: Option<String>,
    },
}

impl SerializedObject {
    /// Create a `NotImplemented` variant with a helpful repr
    ///
    /// Use this for objects that cannot be serialized (closures, trait objects, etc.)
    ///
    /// # Example
    ///
    /// ```rust
    /// use dashflow::core::serialization::SerializedObject;
    ///
    /// let obj = SerializedObject::not_implemented("RunnableLambda<closure>");
    /// ```
    pub fn not_implemented(repr: impl Into<String>) -> Self {
        SerializedObject::NotImplemented {
            lc: SERIALIZATION_VERSION,
            id: vec!["dashflow_core".to_string(), "not_implemented".to_string()],
            repr: Some(repr.into()),
        }
    }

    /// Get the type identifier from this serialized object
    #[must_use]
    pub fn id(&self) -> &[String] {
        match self {
            SerializedObject::Constructor { id, .. }
            | SerializedObject::Secret { id, .. }
            | SerializedObject::NotImplemented { id, .. } => id,
        }
    }

    /// Get the serialization version
    #[must_use]
    pub fn version(&self) -> u32 {
        match self {
            SerializedObject::Constructor { lc, .. }
            | SerializedObject::Secret { lc, .. }
            | SerializedObject::NotImplemented { lc, .. } => *lc,
        }
    }

    /// Is this a constructor-based serialization?
    #[must_use]
    pub fn is_constructor(&self) -> bool {
        matches!(self, SerializedObject::Constructor { .. })
    }

    /// Is this a not-implemented marker?
    #[must_use]
    pub fn is_not_implemented(&self) -> bool {
        matches!(self, SerializedObject::NotImplemented { .. })
    }
}

#[cfg(test)]
mod tests {
    use crate::test_prelude::*;
    use serde_json::json;

    struct TestSerializable {
        name: String,
        value: i32,
    }

    impl Serializable for TestSerializable {
        fn lc_id(&self) -> Vec<String> {
            vec![
                "dashflow_core".to_string(),
                "test".to_string(),
                "TestSerializable".to_string(),
            ]
        }

        fn is_lc_serializable(&self) -> bool {
            true
        }

        fn to_json(&self) -> SerializedObject {
            let mut kwargs = serde_json::Map::new();
            kwargs.insert("name".to_string(), json!(self.name));
            kwargs.insert("value".to_string(), json!(self.value));

            SerializedObject::Constructor {
                lc: SERIALIZATION_VERSION,
                id: self.lc_id(),
                kwargs: kwargs.into(),
            }
        }

        fn lc_secrets(&self) -> HashMap<String, String> {
            HashMap::new()
        }
    }

    #[test]
    fn test_serialize_constructor() {
        let obj = TestSerializable {
            name: "test".to_string(),
            value: 42,
        };

        let serialized = obj.to_json();

        match serialized {
            SerializedObject::Constructor { lc, id, kwargs } => {
                assert_eq!(lc, SERIALIZATION_VERSION);
                assert_eq!(id, vec!["dashflow_core", "test", "TestSerializable"]);
                assert_eq!(kwargs["name"], "test");
                assert_eq!(kwargs["value"], 42);
            }
            _ => panic!("Expected Constructor variant"),
        }
    }

    #[test]
    fn test_serialize_to_json_string() {
        let obj = TestSerializable {
            name: "test".to_string(),
            value: 42,
        };

        let json_str = obj.to_json_string(false).unwrap();
        assert!(json_str.contains("\"type\":\"constructor\""));
        assert!(json_str.contains("\"name\":\"test\""));
        assert!(json_str.contains("\"value\":42"));
    }

    #[test]
    fn test_serialize_to_json_string_pretty() {
        let obj = TestSerializable {
            name: "test".to_string(),
            value: 42,
        };

        let json_str = obj.to_json_string(true).unwrap();
        assert!(json_str.contains("  ")); // Indentation
        assert!(json_str.contains("\n")); // Newlines
    }

    #[test]
    fn test_not_implemented() {
        let obj = SerializedObject::not_implemented("RunnableLambda<closure>");

        match obj {
            SerializedObject::NotImplemented { lc, repr, .. } => {
                assert_eq!(lc, SERIALIZATION_VERSION);
                assert_eq!(repr, Some("RunnableLambda<closure>".to_string()));
            }
            _ => panic!("Expected NotImplemented variant"),
        }
    }

    #[test]
    fn test_serialized_object_methods() {
        let obj = SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: vec!["test".to_string()],
            kwargs: json!({"key": "value"}),
        };

        assert_eq!(obj.id(), &["test"]);
        assert_eq!(obj.version(), SERIALIZATION_VERSION);
        assert!(obj.is_constructor());
        assert!(!obj.is_not_implemented());

        let obj = SerializedObject::not_implemented("test");
        assert!(!obj.is_constructor());
        assert!(obj.is_not_implemented());
    }

    #[test]
    fn test_serde_roundtrip() {
        let obj = SerializedObject::Constructor {
            lc: SERIALIZATION_VERSION,
            id: vec!["dashflow".to_string(), "test".to_string()],
            kwargs: json!({"name": "test", "value": 42}),
        };

        let json_str = serde_json::to_string(&obj).unwrap();
        let deserialized: SerializedObject = serde_json::from_str(&json_str).unwrap();

        assert_eq!(obj, deserialized);
    }

    #[test]
    fn test_serde_not_implemented() {
        let obj = SerializedObject::not_implemented("closure");
        let json_str = serde_json::to_string(&obj).unwrap();
        let deserialized: SerializedObject = serde_json::from_str(&json_str).unwrap();

        assert_eq!(obj, deserialized);
    }

    #[test]
    fn test_default_is_not_serializable() {
        struct NonSerializable;

        impl Serializable for NonSerializable {
            fn lc_id(&self) -> Vec<String> {
                vec!["test".to_string()]
            }

            fn to_json(&self) -> SerializedObject {
                SerializedObject::not_implemented("NonSerializable")
            }

            fn lc_secrets(&self) -> HashMap<String, String> {
                HashMap::new()
            }
        }

        let obj = NonSerializable;
        assert!(!obj.is_lc_serializable()); // Default is false
    }
}
