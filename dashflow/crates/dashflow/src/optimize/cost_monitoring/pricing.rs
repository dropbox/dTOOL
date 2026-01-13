// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Model pricing database and cost calculations

// Allow internal use of deprecated types within this deprecated module
#![allow(deprecated)]

use crate::optimize::cost_monitoring::error::{CostMonitorError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Token usage counts
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::TokenUsage` instead"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens (prompt)
    pub input_tokens: u64,
    /// Output tokens (completion)
    pub output_tokens: u64,
}

impl TokenUsage {
    /// Create new token usage
    pub fn new(input_tokens: u64, output_tokens: u64) -> Self {
        Self {
            input_tokens,
            output_tokens,
        }
    }

    /// Total tokens
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens
    }
}

/// Pricing for a specific model
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::ModelPrice` instead"
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrice {
    /// Model name
    pub name: String,
    /// Cost per 1M input tokens (USD)
    pub input_per_million: f64,
    /// Cost per 1M output tokens (USD)
    pub output_per_million: f64,
    /// Provider (e.g., "OpenAI", "Anthropic", "Google")
    pub provider: String,
}

impl ModelPrice {
    /// Calculate cost for given token usage
    pub fn calculate_cost(&self, usage: TokenUsage) -> f64 {
        let input_cost = (usage.input_tokens as f64 / 1_000_000.0) * self.input_per_million;
        let output_cost = (usage.output_tokens as f64 / 1_000_000.0) * self.output_per_million;
        input_cost + output_cost
    }
}

/// Model pricing database
#[deprecated(
    since = "1.11.3",
    note = "Use `dashflow_observability::cost::ModelPricing` instead"
)]
#[derive(Debug, Clone)]
pub struct ModelPricing {
    prices: HashMap<String, ModelPrice>,
}

impl ModelPricing {
    /// Create a new pricing database with default prices
    pub fn new() -> Self {
        let mut prices = HashMap::new();

        // OpenAI models (as of November 2024)
        prices.insert(
            "gpt-4o".to_string(),
            ModelPrice {
                name: "gpt-4o".to_string(),
                input_per_million: 2.50,
                output_per_million: 10.00,
                provider: "OpenAI".to_string(),
            },
        );

        prices.insert(
            "gpt-4o-mini".to_string(),
            ModelPrice {
                name: "gpt-4o-mini".to_string(),
                input_per_million: 0.150,
                output_per_million: 0.600,
                provider: "OpenAI".to_string(),
            },
        );

        prices.insert(
            "gpt-4-turbo".to_string(),
            ModelPrice {
                name: "gpt-4-turbo".to_string(),
                input_per_million: 10.00,
                output_per_million: 30.00,
                provider: "OpenAI".to_string(),
            },
        );

        prices.insert(
            "gpt-4".to_string(),
            ModelPrice {
                name: "gpt-4".to_string(),
                input_per_million: 30.00,
                output_per_million: 60.00,
                provider: "OpenAI".to_string(),
            },
        );

        prices.insert(
            "gpt-3.5-turbo".to_string(),
            ModelPrice {
                name: "gpt-3.5-turbo".to_string(),
                input_per_million: 0.50,
                output_per_million: 1.50,
                provider: "OpenAI".to_string(),
            },
        );

        // Anthropic models (as of November 2024)
        prices.insert(
            "claude-3-5-sonnet-20241022".to_string(),
            ModelPrice {
                name: "claude-3-5-sonnet-20241022".to_string(),
                input_per_million: 3.00,
                output_per_million: 15.00,
                provider: "Anthropic".to_string(),
            },
        );

        prices.insert(
            "claude-3-opus-20240229".to_string(),
            ModelPrice {
                name: "claude-3-opus-20240229".to_string(),
                input_per_million: 15.00,
                output_per_million: 75.00,
                provider: "Anthropic".to_string(),
            },
        );

        prices.insert(
            "claude-3-sonnet-20240229".to_string(),
            ModelPrice {
                name: "claude-3-sonnet-20240229".to_string(),
                input_per_million: 3.00,
                output_per_million: 15.00,
                provider: "Anthropic".to_string(),
            },
        );

        prices.insert(
            "claude-3-haiku-20240307".to_string(),
            ModelPrice {
                name: "claude-3-haiku-20240307".to_string(),
                input_per_million: 0.25,
                output_per_million: 1.25,
                provider: "Anthropic".to_string(),
            },
        );

        // Google models (as of November 2024)
        prices.insert(
            "gemini-1.5-pro".to_string(),
            ModelPrice {
                name: "gemini-1.5-pro".to_string(),
                input_per_million: 1.25,
                output_per_million: 5.00,
                provider: "Google".to_string(),
            },
        );

        prices.insert(
            "gemini-1.5-flash".to_string(),
            ModelPrice {
                name: "gemini-1.5-flash".to_string(),
                input_per_million: 0.075,
                output_per_million: 0.30,
                provider: "Google".to_string(),
            },
        );

        Self { prices }
    }

    /// Get price for a model
    pub fn get_price(&self, model: &str) -> Result<&ModelPrice> {
        self.prices
            .get(model)
            .ok_or_else(|| CostMonitorError::ModelNotFound(model.to_string()))
    }

    /// Calculate cost for model and token usage
    pub fn calculate_cost(&self, model: &str, usage: TokenUsage) -> Result<f64> {
        let price = self.get_price(model)?;
        Ok(price.calculate_cost(usage))
    }

    /// Add or update a model price
    pub fn set_price(&mut self, model: &str, price: ModelPrice) {
        self.prices.insert(model.to_string(), price);
    }

    /// List all available models
    pub fn list_models(&self) -> Vec<&str> {
        self.prices.keys().map(|s| s.as_str()).collect()
    }

    /// Get all prices grouped by provider
    pub fn by_provider(&self) -> HashMap<String, Vec<&ModelPrice>> {
        let mut by_provider: HashMap<String, Vec<&ModelPrice>> = HashMap::new();
        for price in self.prices.values() {
            by_provider
                .entry(price.provider.clone())
                .or_default()
                .push(price);
        }
        by_provider
    }
}

impl Default for ModelPricing {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_usage() {
        let usage = TokenUsage::new(1000, 500);
        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 500);
        assert_eq!(usage.total(), 1500);
    }

    #[test]
    fn test_model_price_calculation() {
        let price = ModelPrice {
            name: "test-model".to_string(),
            input_per_million: 1.0,
            output_per_million: 2.0,
            provider: "Test".to_string(),
        };

        let usage = TokenUsage::new(1_000_000, 500_000);
        let cost = price.calculate_cost(usage);

        // 1M input @ $1/M = $1.00
        // 500k output @ $2/M = $1.00
        // Total = $2.00
        assert!((cost - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_pricing_database() {
        let pricing = ModelPricing::new();

        // Test OpenAI models exist
        assert!(pricing.get_price("gpt-4o").is_ok());
        assert!(pricing.get_price("gpt-4o-mini").is_ok());
        assert!(pricing.get_price("gpt-3.5-turbo").is_ok());

        // Test Anthropic models exist
        assert!(pricing.get_price("claude-3-5-sonnet-20241022").is_ok());
        assert!(pricing.get_price("claude-3-haiku-20240307").is_ok());

        // Test unknown model
        assert!(pricing.get_price("unknown-model").is_err());
    }

    #[test]
    fn test_calculate_cost() {
        let pricing = ModelPricing::new();
        let usage = TokenUsage::new(1000, 500);

        // Should successfully calculate for known model
        let cost = pricing.calculate_cost("gpt-4o-mini", usage);
        assert!(cost.is_ok());

        // Should fail for unknown model
        let cost = pricing.calculate_cost("unknown", usage);
        assert!(cost.is_err());
    }

    #[test]
    fn test_gpt4o_mini_cost() {
        let pricing = ModelPricing::new();
        // gpt-4o-mini: $0.150/M input, $0.600/M output
        let usage = TokenUsage::new(10_000, 5_000);
        let cost = pricing.calculate_cost("gpt-4o-mini", usage).unwrap();

        // 10k input @ $0.150/M = $0.0015
        // 5k output @ $0.600/M = $0.0030
        // Total = $0.0045
        assert!((cost - 0.0045).abs() < 0.0001);
    }

    #[test]
    fn test_custom_price() {
        let mut pricing = ModelPricing::new();

        let custom = ModelPrice {
            name: "custom-model".to_string(),
            input_per_million: 5.0,
            output_per_million: 10.0,
            provider: "Custom".to_string(),
        };

        pricing.set_price("custom-model", custom);

        assert!(pricing.get_price("custom-model").is_ok());
        let usage = TokenUsage::new(1_000_000, 1_000_000);
        let cost = pricing.calculate_cost("custom-model", usage).unwrap();
        assert!((cost - 15.0).abs() < 0.001);
    }

    #[test]
    fn test_list_models() {
        let pricing = ModelPricing::new();
        let models = pricing.list_models();

        assert!(models.contains(&"gpt-4o"));
        assert!(models.contains(&"gpt-4o-mini"));
        assert!(models.contains(&"claude-3-haiku-20240307"));
        assert!(models.len() >= 10);
    }

    #[test]
    fn test_by_provider() {
        let pricing = ModelPricing::new();
        let by_provider = pricing.by_provider();

        assert!(by_provider.contains_key("OpenAI"));
        assert!(by_provider.contains_key("Anthropic"));
        assert!(by_provider.contains_key("Google"));

        let openai_models = &by_provider["OpenAI"];
        assert!(openai_models.len() >= 3);
    }
}
