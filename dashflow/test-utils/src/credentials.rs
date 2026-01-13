//! Credential loading and validation for integration tests

use std::collections::HashMap;
use std::env;

use crate::{Result, TestError};

/// Test credentials container
#[derive(Debug, Clone, Default)]
pub struct Credentials {
    /// All environment variables
    env: HashMap<String, String>,
}

impl Credentials {
    /// Get a required credential
    pub fn get_required(&self, key: &str) -> Result<String> {
        self.env
            .get(key)
            .cloned()
            .ok_or_else(|| TestError::MissingCredential(key.to_string()))
    }

    /// Get an optional credential
    #[must_use]
    pub fn get_optional(&self, key: &str) -> Option<String> {
        self.env.get(key).cloned()
    }

    /// Check if a credential exists
    #[must_use]
    pub fn has(&self, key: &str) -> bool {
        self.env.contains_key(key)
    }

    /// Get all credentials
    #[must_use]
    pub fn all(&self) -> &HashMap<String, String> {
        &self.env
    }
}

/// Credential loader and validator
pub struct CredentialsLoader {
    required_keys: Vec<String>,
    optional_keys: Vec<String>,
}

impl CredentialsLoader {
    /// Create a new credential loader
    #[must_use]
    pub fn new() -> Self {
        Self {
            required_keys: Vec::new(),
            optional_keys: Vec::new(),
        }
    }

    /// Add a required credential key
    pub fn require(mut self, key: impl Into<String>) -> Self {
        self.required_keys.push(key.into());
        self
    }

    /// Add an optional credential key
    pub fn optional(mut self, key: impl Into<String>) -> Self {
        self.optional_keys.push(key.into());
        self
    }

    /// Load and validate credentials
    pub fn load(self) -> Result<Credentials> {
        let mut env = HashMap::new();

        // Load required keys
        for key in &self.required_keys {
            let value = env::var(key).map_err(|_| TestError::MissingCredential(key.clone()))?;
            env.insert(key.clone(), value);
        }

        // Load optional keys
        for key in &self.optional_keys {
            if let Ok(value) = env::var(key) {
                env.insert(key.clone(), value);
            }
        }

        Ok(Credentials { env })
    }

    /// Load credentials or return None if any required key is missing
    #[must_use]
    pub fn load_optional(self) -> Option<Credentials> {
        self.load().ok()
    }
}

impl Default for CredentialsLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Predefined credential loaders for common services
///
/// `OpenAI` credentials
pub fn openai_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("OPENAI_API_KEY").load()
}

/// Anthropic credentials
pub fn anthropic_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("ANTHROPIC_API_KEY").load()
}

/// Groq credentials
pub fn groq_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("GROQ_API_KEY").load()
}

/// Cohere credentials
pub fn cohere_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("COHERE_API_KEY").load()
}

/// `HuggingFace` credentials
pub fn huggingface_credentials() -> Result<Credentials> {
    CredentialsLoader::new()
        .require("HUGGINGFACE_API_KEY")
        .optional("HF_TOKEN")
        .load()
}

/// Pinecone credentials
pub fn pinecone_credentials() -> Result<Credentials> {
    CredentialsLoader::new()
        .require("PINECONE_API_KEY")
        .require("PINECONE_ENVIRONMENT")
        .load()
}

/// `MongoDB` credentials (docker)
pub fn mongodb_credentials() -> Result<Credentials> {
    CredentialsLoader::new()
        .optional("MONGODB_URI")
        .optional("MONGODB_DATABASE")
        .load()
}

/// `PostgreSQL` credentials (docker)
pub fn postgres_credentials() -> Result<Credentials> {
    CredentialsLoader::new()
        .optional("DATABASE_URL")
        .optional("POSTGRES_HOST")
        .optional("POSTGRES_PORT")
        .optional("POSTGRES_USER")
        .optional("POSTGRES_PASSWORD")
        .optional("POSTGRES_DB")
        .load()
}

/// Redis credentials (docker)
pub fn redis_credentials() -> Result<Credentials> {
    CredentialsLoader::new().optional("REDIS_URL").load()
}

/// Chroma credentials (docker)
pub fn chroma_credentials() -> Result<Credentials> {
    CredentialsLoader::new().optional("CHROMA_URL").load()
}

/// Qdrant credentials (docker)
pub fn qdrant_credentials() -> Result<Credentials> {
    CredentialsLoader::new().optional("QDRANT_URL").load()
}

/// Weaviate credentials (docker)
pub fn weaviate_credentials() -> Result<Credentials> {
    CredentialsLoader::new().optional("WEAVIATE_URL").load()
}

/// Mistral credentials
pub fn mistral_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("MISTRAL_API_KEY").load()
}

/// Fireworks credentials
pub fn fireworks_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("FIREWORKS_API_KEY").load()
}

/// Ollama credentials (local, usually no auth needed)
pub fn ollama_credentials() -> Result<Credentials> {
    CredentialsLoader::new().optional("OLLAMA_BASE_URL").load()
}

/// xAI credentials
pub fn xai_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("XAI_API_KEY").load()
}

/// Nomic credentials
pub fn nomic_credentials() -> Result<Credentials> {
    CredentialsLoader::new().require("NOMIC_API_KEY").load()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credentials_loader() {
        env::set_var("TEST_KEY_1", "value1");
        env::set_var("TEST_KEY_2", "value2");

        let creds = CredentialsLoader::new()
            .require("TEST_KEY_1")
            .optional("TEST_KEY_2")
            .optional("TEST_KEY_MISSING")
            .load()
            .unwrap();

        assert_eq!(creds.get_required("TEST_KEY_1").unwrap(), "value1");
        assert_eq!(creds.get_optional("TEST_KEY_2").unwrap(), "value2");
        assert!(creds.get_optional("TEST_KEY_MISSING").is_none());
        assert!(creds.has("TEST_KEY_1"));
        assert!(!creds.has("TEST_KEY_MISSING"));

        env::remove_var("TEST_KEY_1");
        env::remove_var("TEST_KEY_2");
    }

    #[test]
    fn test_missing_required_credential() {
        let result = CredentialsLoader::new().require("MISSING_KEY").load();

        assert!(result.is_err());
        match result {
            Err(TestError::MissingCredential(key)) => {
                assert_eq!(key, "MISSING_KEY");
            }
            _ => panic!("Expected MissingCredential error"),
        }
    }
}
