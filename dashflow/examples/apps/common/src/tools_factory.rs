// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Tools Factory - Provider-agnostic tool creation
//!
//! Creates tools based on environment and requirements, NOT hardcoded providers.
//!
//! # Usage
//!
//! ```rust,ignore
//! use common::tools_factory::{create_tool, ToolRequirements};
//!
//! // Basic usage - get any available search tool
//! let tool = create_tool(ToolRequirements::default())?;
//!
//! // With specific requirements
//! let tool = create_tool(ToolRequirements {
//!     tool_type: Some("tavily".to_string()),
//!     max_results: Some(10),
//!     ..Default::default()
//! })?;
//! ```
//!
//! # Provider Priority (for search tools)
//!
//! 1. **Tavily** - If TAVILY_API_KEY is set (AI-optimized search)
//! 2. **DuckDuckGo** - Free fallback (no API key required)

use dashflow::core::tools::Tool;
use std::sync::Arc;

/// Tool requirements for provider selection
#[derive(Debug, Clone, Default)]
pub struct ToolRequirements {
    /// Specific tool type to use (e.g., "tavily", "duckduckgo", "calculator")
    pub tool_type: Option<String>,
    /// Maximum number of results (for search tools)
    pub max_results: Option<u32>,
    /// Search depth (for Tavily: "basic" or "advanced")
    pub search_depth: Option<String>,
    /// Include LLM-generated answer (for Tavily)
    pub include_answer: bool,
}

/// Result of tool detection
#[derive(Debug)]
pub struct ToolProviderInfo {
    pub name: &'static str,
    pub tool_type: &'static str,
}

/// Create a search tool based on available credentials and requirements
///
/// Returns the first available provider that meets the requirements.
/// Providers are tried in this order:
/// 1. Tavily (if TAVILY_API_KEY set)
/// 2. DuckDuckGo (free fallback)
pub fn create_tool(requirements: ToolRequirements) -> anyhow::Result<Arc<dyn Tool>> {
    // If specific tool type requested, try that first
    if let Some(ref tool_type) = requirements.tool_type {
        match tool_type.as_str() {
            "tavily" => {
                if let Some(tool) = try_tavily(&requirements) {
                    return Ok(tool);
                }
                anyhow::bail!("Tavily requested but TAVILY_API_KEY not set");
            }
            #[cfg(feature = "duckduckgo")]
            "duckduckgo" => {
                if let Some(tool) = try_duckduckgo(&requirements) {
                    return Ok(tool);
                }
                anyhow::bail!("DuckDuckGo tool creation failed");
            }
            "calculator" => {
                return Ok(Arc::new(dashflow::core::tools::builtin::calculator_tool()));
            }
            other => anyhow::bail!("Unknown tool type: {}", other),
        }
    }

    // Auto-detect: try providers in priority order
    // 1. Try Tavily (AI-optimized)
    if std::env::var("TAVILY_API_KEY").is_ok() {
        if let Some(tool) = try_tavily(&requirements) {
            return Ok(tool);
        }
    }

    // 2. Try DuckDuckGo (free fallback)
    #[cfg(feature = "duckduckgo")]
    {
        if let Some(tool) = try_duckduckgo(&requirements) {
            return Ok(tool);
        }
    }

    anyhow::bail!(
        "No search tool provider available. Set TAVILY_API_KEY for Tavily search, \
         or enable the 'duckduckgo' feature for free search."
    )
}

/// Get information about available tool providers without creating a tool
pub fn detect_available_tool_providers() -> Vec<ToolProviderInfo> {
    let mut providers = Vec::new();

    if std::env::var("TAVILY_API_KEY").is_ok() {
        providers.push(ToolProviderInfo {
            name: "Tavily",
            tool_type: "tavily",
        });
    }

    #[cfg(feature = "duckduckgo")]
    {
        providers.push(ToolProviderInfo {
            name: "DuckDuckGo",
            tool_type: "duckduckgo",
        });
    }

    // Calculator is always available
    providers.push(ToolProviderInfo {
        name: "Calculator",
        tool_type: "calculator",
    });

    providers
}

/// Try to create a Tavily search tool
fn try_tavily(req: &ToolRequirements) -> Option<Arc<dyn Tool>> {
    use dashflow_tavily::TavilySearchTool;

    let api_key = std::env::var("TAVILY_API_KEY").ok()?;

    let mut builder = TavilySearchTool::builder().api_key(api_key);

    if let Some(max_results) = req.max_results {
        builder = builder.max_results(max_results);
    }

    if let Some(ref search_depth) = req.search_depth {
        builder = builder.search_depth(search_depth.clone());
    }

    if req.include_answer {
        builder = builder.include_answer(true);
    }

    builder.build().ok().map(|t| Arc::new(t) as Arc<dyn Tool>)
}

/// Try to create a DuckDuckGo search tool
#[cfg(feature = "duckduckgo")]
fn try_duckduckgo(req: &ToolRequirements) -> Option<Arc<dyn Tool>> {
    use dashflow_duckduckgo::DuckDuckGoSearchTool;

    let mut builder = DuckDuckGoSearchTool::builder();

    if let Some(max_results) = req.max_results {
        builder = builder.max_results(max_results as usize);
    }

    Some(Arc::new(builder.build()) as Arc<dyn Tool>)
}

/// Create a tool from a ToolConfig
///
/// Routes to the appropriate provider-specific `build_tool()` function
/// based on the configuration type.
///
/// # Example
///
/// ```rust,ignore
/// use dashflow::core::config_loader::ToolConfig;
/// use common::tools_factory::create_tool_from_config;
///
/// let config: ToolConfig = serde_yaml::from_str(yaml)?;
/// let tool = create_tool_from_config(&config)?;
/// ```
pub fn create_tool_from_config(
    config: &dashflow::core::config_loader::ToolConfig,
) -> anyhow::Result<Arc<dyn Tool>> {
    use dashflow::core::config_loader::ToolConfig;

    match config {
        ToolConfig::Tavily { .. } => dashflow_tavily::build_tool(config)
            .map_err(|e| anyhow::anyhow!("Tavily tool build failed: {}", e)),
    }
}

#[cfg(test)]
#[allow(deprecated, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_default_requirements() {
        let req = ToolRequirements::default();
        assert!(req.tool_type.is_none());
        assert!(req.max_results.is_none());
        assert!(!req.include_answer);
    }

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_tool_providers();
        println!("Available tool providers: {:?}", providers);
        // Calculator is always available
        assert!(providers.iter().any(|p| p.tool_type == "calculator"));
    }

    #[test]
    fn test_create_calculator() {
        let tool = create_tool(ToolRequirements {
            tool_type: Some("calculator".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(tool.name(), "calculator");
    }
}
