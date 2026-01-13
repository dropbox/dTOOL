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
//! use dashflow_factories::{create_tool, ToolRequirements};
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

use dashflow::core::config_loader::env_vars::{env_is_set, env_string, TAVILY_API_KEY};
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
            #[cfg(not(feature = "duckduckgo"))]
            "duckduckgo" => {
                anyhow::bail!(
                    "DuckDuckGo support not compiled in. Enable the 'duckduckgo' feature."
                );
            }
            "calculator" => {
                return Ok(Arc::new(dashflow::core::tools::builtin::calculator_tool()));
            }
            other => {
                anyhow::bail!("Unknown tool type: {}", other);
            }
        }
    }

    // Auto-detect: try providers in priority order
    // 1. Try Tavily (AI-optimized)
    if env_is_set(TAVILY_API_KEY) {
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

    if env_is_set(TAVILY_API_KEY) {
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

    let api_key = env_string(TAVILY_API_KEY)?;

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
/// use dashflow_factories::create_tool_from_config;
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
mod tests {
    use super::*;

    // ============================================================================
    // ToolRequirements Tests
    // ============================================================================

    #[test]
    fn test_default_requirements() {
        let req = ToolRequirements::default();
        assert!(req.tool_type.is_none());
        assert!(req.max_results.is_none());
        assert!(req.search_depth.is_none());
        assert!(!req.include_answer);
    }

    #[test]
    fn test_requirements_debug_impl() {
        let req = ToolRequirements::default();
        let debug_str = format!("{:?}", req);
        assert!(debug_str.contains("ToolRequirements"));
        assert!(debug_str.contains("tool_type"));
        assert!(debug_str.contains("max_results"));
    }

    #[test]
    fn test_requirements_clone_impl() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            max_results: Some(10),
            search_depth: Some("advanced".to_string()),
            include_answer: true,
        };
        let cloned = req.clone();
        assert_eq!(cloned.tool_type, Some("tavily".to_string()));
        assert_eq!(cloned.max_results, Some(10));
        assert_eq!(cloned.search_depth, Some("advanced".to_string()));
        assert!(cloned.include_answer);
    }

    #[test]
    fn test_requirements_with_tool_type() {
        let req = ToolRequirements {
            tool_type: Some("duckduckgo".to_string()),
            ..Default::default()
        };
        assert_eq!(req.tool_type, Some("duckduckgo".to_string()));
    }

    #[test]
    fn test_requirements_with_max_results() {
        let req = ToolRequirements {
            max_results: Some(5),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(5));
    }

    #[test]
    fn test_requirements_with_search_depth_basic() {
        let req = ToolRequirements {
            search_depth: Some("basic".to_string()),
            ..Default::default()
        };
        assert_eq!(req.search_depth, Some("basic".to_string()));
    }

    #[test]
    fn test_requirements_with_search_depth_advanced() {
        let req = ToolRequirements {
            search_depth: Some("advanced".to_string()),
            ..Default::default()
        };
        assert_eq!(req.search_depth, Some("advanced".to_string()));
    }

    #[test]
    fn test_requirements_with_include_answer() {
        let req = ToolRequirements {
            include_answer: true,
            ..Default::default()
        };
        assert!(req.include_answer);
    }

    #[test]
    fn test_requirements_all_fields_set() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            max_results: Some(20),
            search_depth: Some("advanced".to_string()),
            include_answer: true,
        };
        assert_eq!(req.tool_type, Some("tavily".to_string()));
        assert_eq!(req.max_results, Some(20));
        assert_eq!(req.search_depth, Some("advanced".to_string()));
        assert!(req.include_answer);
    }

    #[test]
    fn test_requirements_max_results_zero() {
        let req = ToolRequirements {
            max_results: Some(0),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(0));
    }

    #[test]
    fn test_requirements_max_results_large() {
        let req = ToolRequirements {
            max_results: Some(100),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(100));
    }

    // ============================================================================
    // ToolProviderInfo Tests
    // ============================================================================

    #[test]
    fn test_provider_info_debug_impl() {
        let info = ToolProviderInfo {
            name: "TestProvider",
            tool_type: "test",
        };
        let debug_str = format!("{:?}", info);
        assert!(debug_str.contains("ToolProviderInfo"));
        assert!(debug_str.contains("TestProvider"));
        assert!(debug_str.contains("test"));
    }

    #[test]
    fn test_provider_info_fields() {
        let info = ToolProviderInfo {
            name: "Tavily",
            tool_type: "tavily",
        };
        assert_eq!(info.name, "Tavily");
        assert_eq!(info.tool_type, "tavily");
    }

    #[test]
    fn test_provider_info_calculator() {
        let info = ToolProviderInfo {
            name: "Calculator",
            tool_type: "calculator",
        };
        assert_eq!(info.name, "Calculator");
        assert_eq!(info.tool_type, "calculator");
    }

    #[test]
    fn test_provider_info_duckduckgo() {
        let info = ToolProviderInfo {
            name: "DuckDuckGo",
            tool_type: "duckduckgo",
        };
        assert_eq!(info.name, "DuckDuckGo");
        assert_eq!(info.tool_type, "duckduckgo");
    }

    // ============================================================================
    // detect_available_tool_providers Tests
    // ============================================================================

    #[test]
    fn test_detect_providers() {
        // This test doesn't fail - it just reports what's available
        let providers = detect_available_tool_providers();
        println!("Available tool providers: {:?}", providers);
        // Calculator is always available
        assert!(providers.iter().any(|p| p.tool_type == "calculator"));
    }

    #[test]
    fn test_detect_providers_returns_vec() {
        let providers = detect_available_tool_providers();
        // The function should return a Vec with at least Calculator
        assert!(!providers.is_empty());
        assert!(providers.len() <= 10); // Sanity check
    }

    #[test]
    fn test_detect_providers_each_has_valid_data() {
        let providers = detect_available_tool_providers();
        for provider in providers {
            assert!(!provider.name.is_empty());
            assert!(!provider.tool_type.is_empty());
        }
    }

    #[test]
    fn test_detect_providers_calculator_always_present() {
        let providers = detect_available_tool_providers();
        let has_calculator = providers.iter().any(|p| p.name == "Calculator" && p.tool_type == "calculator");
        assert!(has_calculator);
    }

    // ============================================================================
    // create_tool Tests
    // ============================================================================

    #[test]
    fn test_create_calculator() {
        let tool = create_tool(ToolRequirements {
            tool_type: Some("calculator".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(tool.name(), "calculator");
    }

    #[test]
    fn test_create_calculator_ignores_other_fields() {
        // Calculator should work regardless of other requirements
        let tool = create_tool(ToolRequirements {
            tool_type: Some("calculator".to_string()),
            max_results: Some(10), // Ignored for calculator
            search_depth: Some("advanced".to_string()), // Ignored
            include_answer: true, // Ignored
        })
        .unwrap();
        assert_eq!(tool.name(), "calculator");
    }

    #[test]
    fn test_create_tool_unknown_type_fails() {
        let result = create_tool(ToolRequirements {
            tool_type: Some("unknown_tool".to_string()),
            ..Default::default()
        });
        assert!(result.is_err());
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("Unknown tool type"));
        }
    }

    #[test]
    fn test_create_tavily_fails_without_api_key() {
        // Save and clear TAVILY_API_KEY
        let original = std::env::var("TAVILY_API_KEY").ok();
        std::env::remove_var("TAVILY_API_KEY");

        let result = create_tool(ToolRequirements {
            tool_type: Some("tavily".to_string()),
            ..Default::default()
        });

        // Restore
        if let Some(v) = original {
            std::env::set_var("TAVILY_API_KEY", v);
        }

        assert!(result.is_err());
        if let Err(err) = result {
            let err_msg = err.to_string();
            assert!(err_msg.contains("TAVILY_API_KEY"));
        }
    }

    // ============================================================================
    // Tool Type String Tests
    // ============================================================================

    #[test]
    fn test_tool_type_tavily() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            ..Default::default()
        };
        assert_eq!(req.tool_type.as_deref(), Some("tavily"));
    }

    #[test]
    fn test_tool_type_duckduckgo() {
        let req = ToolRequirements {
            tool_type: Some("duckduckgo".to_string()),
            ..Default::default()
        };
        assert_eq!(req.tool_type.as_deref(), Some("duckduckgo"));
    }

    #[test]
    fn test_tool_type_calculator() {
        let req = ToolRequirements {
            tool_type: Some("calculator".to_string()),
            ..Default::default()
        };
        assert_eq!(req.tool_type.as_deref(), Some("calculator"));
    }

    // ============================================================================
    // Requirements Combination Tests
    // ============================================================================

    #[test]
    fn test_tavily_with_max_results() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            max_results: Some(5),
            ..Default::default()
        };
        assert_eq!(req.tool_type, Some("tavily".to_string()));
        assert_eq!(req.max_results, Some(5));
    }

    #[test]
    fn test_tavily_with_search_depth() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            search_depth: Some("advanced".to_string()),
            ..Default::default()
        };
        assert_eq!(req.search_depth, Some("advanced".to_string()));
    }

    #[test]
    fn test_tavily_full_options() {
        let req = ToolRequirements {
            tool_type: Some("tavily".to_string()),
            max_results: Some(10),
            search_depth: Some("advanced".to_string()),
            include_answer: true,
        };
        assert_eq!(req.tool_type.as_deref(), Some("tavily"));
        assert_eq!(req.max_results, Some(10));
        assert_eq!(req.search_depth.as_deref(), Some("advanced"));
        assert!(req.include_answer);
    }

    #[test]
    fn test_duckduckgo_with_max_results() {
        let req = ToolRequirements {
            tool_type: Some("duckduckgo".to_string()),
            max_results: Some(20),
            ..Default::default()
        };
        assert_eq!(req.tool_type.as_deref(), Some("duckduckgo"));
        assert_eq!(req.max_results, Some(20));
    }

    // ============================================================================
    // Search Depth Value Tests
    // ============================================================================

    #[test]
    fn test_search_depth_basic_value() {
        let req = ToolRequirements {
            search_depth: Some("basic".to_string()),
            ..Default::default()
        };
        assert_eq!(req.search_depth.as_deref(), Some("basic"));
    }

    #[test]
    fn test_search_depth_advanced_value() {
        let req = ToolRequirements {
            search_depth: Some("advanced".to_string()),
            ..Default::default()
        };
        assert_eq!(req.search_depth.as_deref(), Some("advanced"));
    }

    #[test]
    fn test_search_depth_empty() {
        let req = ToolRequirements {
            search_depth: Some(String::new()),
            ..Default::default()
        };
        assert_eq!(req.search_depth, Some(String::new()));
    }

    // ============================================================================
    // Max Results Edge Cases
    // ============================================================================

    #[test]
    fn test_max_results_one() {
        let req = ToolRequirements {
            max_results: Some(1),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(1));
    }

    #[test]
    fn test_max_results_ten() {
        let req = ToolRequirements {
            max_results: Some(10),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(10));
    }

    #[test]
    fn test_max_results_max_u32() {
        let req = ToolRequirements {
            max_results: Some(u32::MAX),
            ..Default::default()
        };
        assert_eq!(req.max_results, Some(u32::MAX));
    }

    // ============================================================================
    // Auto-Detection Tests (no explicit tool_type)
    // ============================================================================

    #[test]
    fn test_auto_detect_without_credentials() {
        // Save and clear credentials
        let original_tavily = std::env::var("TAVILY_API_KEY").ok();
        std::env::remove_var("TAVILY_API_KEY");

        let result = create_tool(ToolRequirements::default());

        // Restore
        if let Some(v) = original_tavily {
            std::env::set_var("TAVILY_API_KEY", v);
        }

        // Without credentials and without duckduckgo feature, should fail
        // Unless duckduckgo feature is enabled
        #[cfg(not(feature = "duckduckgo"))]
        {
            assert!(result.is_err());
        }
        #[cfg(feature = "duckduckgo")]
        {
            // With duckduckgo feature, should succeed
            assert!(result.is_ok());
        }
    }

    // ============================================================================
    // Calculator Tool Verification
    // ============================================================================

    #[test]
    fn test_calculator_has_description() {
        let tool = create_tool(ToolRequirements {
            tool_type: Some("calculator".to_string()),
            ..Default::default()
        })
        .unwrap();
        let desc = tool.description();
        assert!(!desc.is_empty());
    }

    #[test]
    fn test_calculator_name_is_calculator() {
        let tool = create_tool(ToolRequirements {
            tool_type: Some("calculator".to_string()),
            ..Default::default()
        })
        .unwrap();
        assert_eq!(tool.name(), "calculator");
    }
}
