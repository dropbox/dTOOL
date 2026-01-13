//! Browser automation tools for DashFlow using Playwright.
//!
//! This crate provides a collection of tools for browser automation using Playwright,
//! enabling AI agents to interact with web pages.
//!
//! # Features
//!
//! - **NavigateTool**: Navigate to a URL
//! - **NavigateBackTool**: Navigate back in browser history
//! - **ExtractTextTool**: Extract text content from the current page
//! - **ExtractHyperlinksTool**: Extract all hyperlinks from the current page
//! - **GetElementsTool**: Get elements by CSS selector
//! - **ClickTool**: Click on an element
//! - **CurrentWebPageTool**: Get current page URL and title
//!
//! # Example
//!
//! ```no_run
//! use dashflow_playwright::NavigateTool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let tool = NavigateTool::new().await?;
//! let result = tool._call_str("https://www.example.com".to_string()).await?;
//! println!("Navigated: {}", result);
//! # Ok(())
//! # }
//! ```

use dashflow::core::error::Result;
use dashflow::core::tools::{Tool, ToolInput};
use playwright::api::{BrowserContext, Page, Playwright};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Shared browser state across all Playwright tools.
#[derive(Clone)]
pub struct BrowserState {
    /// Browser context - kept alive to maintain page lifetime.
    /// In Playwright, the context owns the page, so we must hold a reference
    /// to prevent the page from being dropped.
    ///
    /// Lifetime management field: CRITICAL for Playwright architecture.
    /// BrowserContext MUST be held to keep Page valid (Playwright ownership model)
    #[allow(dead_code)] // Architectural: Context held alive to prevent Page use-after-free
    context: Arc<Mutex<BrowserContext>>,
    page: Arc<Mutex<Page>>,
}

impl BrowserState {
    /// Initialize a new browser state with Chromium.
    pub async fn new() -> Result<Self> {
        let playwright = Playwright::initialize().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to initialize Playwright: {}", e))
        })?;
        playwright.install_chromium().map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to install Chromium: {}", e))
        })?;
        let chromium = playwright.chromium();
        let browser = chromium
            .launcher()
            .headless(true)
            .launch()
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to launch browser: {}", e))
            })?;
        let context = browser.context_builder().build().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to create browser context: {}", e))
        })?;
        let page = context.new_page().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to create new page: {}", e))
        })?;

        Ok(Self {
            context: Arc::new(Mutex::new(context)),
            page: Arc::new(Mutex::new(page)),
        })
    }

    /// Get a reference to the current page.
    pub async fn page(&self) -> tokio::sync::MutexGuard<'_, Page> {
        self.page.lock().await
    }
}

/// Tool for navigating to a URL.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::NavigateTool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let tool = NavigateTool::new().await?;
/// let result = tool._call_str("https://www.rust-lang.org".to_string()).await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct NavigateTool {
    state: BrowserState,
}

impl NavigateTool {
    /// Create a new NavigateTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for NavigateTool {
    fn name(&self) -> &str {
        "navigate"
    }

    fn description(&self) -> &str {
        "Navigate to a URL. Input should be a valid URL string."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let url = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error("Expected 'input' field with URL string")
                })?
                .to_string(),
        };

        let page = self.state.page().await;
        page.goto_builder(&url).goto().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to navigate to URL: {}", e))
        })?;

        let current_url = page.url().map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to get current URL: {}", e))
        })?;
        Ok(format!("Navigated to: {}", current_url))
    }
}

/// Tool for navigating back in browser history.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, NavigateBackTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.rust-lang.org".to_string()).await?;
/// nav_tool._call_str("https://crates.io".to_string()).await?;
///
/// let back_tool = NavigateBackTool::with_state(nav_tool.state().clone());
/// let result = back_tool._call_str("".to_string()).await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct NavigateBackTool {
    state: BrowserState,
}

impl NavigateBackTool {
    /// Create a new NavigateBackTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for NavigateBackTool {
    fn name(&self) -> &str {
        "navigate_back"
    }

    fn description(&self) -> &str {
        "Navigate back to the previous page in the browser history."
    }

    async fn call(&self, _input: ToolInput) -> Result<String> {
        let page = self.state.page().await;
        page.go_back_builder().go_back().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to navigate back: {}", e))
        })?;

        let url = page.url().map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to get current URL: {}", e))
        })?;
        Ok(format!("Navigated back to: {}", url))
    }
}

/// Tool for extracting text content from the current page.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, ExtractTextTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.example.com".to_string()).await?;
///
/// let extract_tool = ExtractTextTool::with_state(nav_tool.state().clone());
/// let text = extract_tool._call_str("".to_string()).await?;
/// println!("Page text: {}", text);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ExtractTextTool {
    state: BrowserState,
}

impl ExtractTextTool {
    /// Create a new ExtractTextTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for ExtractTextTool {
    fn name(&self) -> &str {
        "extract_text"
    }

    fn description(&self) -> &str {
        "Extract all visible text from the current web page."
    }

    async fn call(&self, _input: ToolInput) -> Result<String> {
        let page = self.state.page().await;
        let text = page
            .eval::<String>("() => document.body.innerText")
            .await
            .map_err(|e| {
                dashflow::core::Error::tool_error(format!(
                    "Failed to extract text from page: {}",
                    e
                ))
            })?;

        Ok(text)
    }
}

/// Tool for extracting hyperlinks from the current page.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, ExtractHyperlinksTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.example.com".to_string()).await?;
///
/// let extract_tool = ExtractHyperlinksTool::with_state(nav_tool.state().clone());
/// let links = extract_tool._call_str("".to_string()).await?;
/// println!("Links: {}", links);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ExtractHyperlinksTool {
    state: BrowserState,
}

impl ExtractHyperlinksTool {
    /// Create a new ExtractHyperlinksTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Link {
    text: String,
    href: String,
}

#[async_trait::async_trait]
impl Tool for ExtractHyperlinksTool {
    fn name(&self) -> &str {
        "extract_hyperlinks"
    }

    fn description(&self) -> &str {
        "Extract all hyperlinks from the current web page. Returns JSON array of {text, href} objects."
    }

    async fn call(&self, _input: ToolInput) -> Result<String> {
        let page = self.state.page().await;

        let script = r#"
            () => {
                const links = Array.from(document.querySelectorAll('a'));
                return links.map(a => ({
                    text: a.innerText.trim(),
                    href: a.href
                })).filter(link => link.href);
            }
        "#;

        let links: Vec<Link> = page.eval(script).await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to extract hyperlinks: {}", e))
        })?;

        serde_json::to_string_pretty(&links).map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to serialize links: {}", e))
        })
    }
}

/// Tool for getting elements by CSS selector.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, GetElementsTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.example.com".to_string()).await?;
///
/// let get_tool = GetElementsTool::with_state(nav_tool.state().clone());
/// let elements = get_tool._call_str("h1, h2".to_string()).await?;
/// println!("Elements: {}", elements);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct GetElementsTool {
    state: BrowserState,
}

impl GetElementsTool {
    /// Create a new GetElementsTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Element {
    tag: String,
    text: String,
    attributes: serde_json::Value,
}

#[async_trait::async_trait]
impl Tool for GetElementsTool {
    fn name(&self) -> &str {
        "get_elements"
    }

    fn description(&self) -> &str {
        "Get elements by CSS selector. Input should be a valid CSS selector string. Returns JSON array of elements."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let selector = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error(
                        "Expected 'input' field with CSS selector string",
                    )
                })?
                .to_string(),
        };

        let page = self.state.page().await;

        // Use format! to embed the selector directly in the JavaScript
        let script = format!(
            r#"
            () => {{
                const selector = '{}';
                const elements = Array.from(document.querySelectorAll(selector));
                return elements.map(el => ({{
                    tag: el.tagName.toLowerCase(),
                    text: el.innerText?.trim() || '',
                    attributes: Object.fromEntries(
                        Array.from(el.attributes).map(attr => [attr.name, attr.value])
                    )
                }}));
            }}
        "#,
            selector.replace('\'', "\\'")
        );

        let elements: Vec<Element> = page.eval(&script).await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to get elements: {}", e))
        })?;

        serde_json::to_string_pretty(&elements).map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to serialize elements: {}", e))
        })
    }
}

/// Tool for clicking on an element.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, ClickTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.example.com".to_string()).await?;
///
/// let click_tool = ClickTool::with_state(nav_tool.state().clone());
/// let result = click_tool._call_str("button#submit".to_string()).await?;
/// println!("{}", result);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct ClickTool {
    state: BrowserState,
}

impl ClickTool {
    /// Create a new ClickTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl Tool for ClickTool {
    fn name(&self) -> &str {
        "click"
    }

    fn description(&self) -> &str {
        "Click on an element. Input should be a CSS selector for the element to click."
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let selector = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("input")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error(
                        "Expected 'input' field with CSS selector string",
                    )
                })?
                .to_string(),
        };

        let page = self.state.page().await;
        page.click_builder(&selector).click().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to click element: {}", e))
        })?;

        Ok(format!("Clicked on element: {}", selector))
    }
}

/// Tool for getting the current page URL and title.
///
/// # Example
///
/// ```no_run
/// use dashflow_playwright::{NavigateTool, CurrentWebPageTool};
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let nav_tool = NavigateTool::new().await?;
/// nav_tool._call_str("https://www.example.com".to_string()).await?;
///
/// let current_tool = CurrentWebPageTool::with_state(nav_tool.state().clone());
/// let info = current_tool._call_str("".to_string()).await?;
/// println!("Current page: {}", info);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct CurrentWebPageTool {
    state: BrowserState,
}

impl CurrentWebPageTool {
    /// Create a new CurrentWebPageTool with a fresh browser instance.
    pub async fn new() -> Result<Self> {
        Ok(Self {
            state: BrowserState::new().await?,
        })
    }

    /// Create a tool with an existing browser state.
    pub fn with_state(state: BrowserState) -> Self {
        Self { state }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct PageInfo {
    url: String,
    title: String,
}

#[async_trait::async_trait]
impl Tool for CurrentWebPageTool {
    fn name(&self) -> &str {
        "current_page"
    }

    fn description(&self) -> &str {
        "Get information about the current web page (URL and title)."
    }

    async fn call(&self, _input: ToolInput) -> Result<String> {
        let page = self.state.page().await;
        let url = page.url().map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to get current URL: {}", e))
        })?;
        let title = page.title().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to get page title: {}", e))
        })?;

        let info = PageInfo { url, title };
        serde_json::to_string_pretty(&info).map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to serialize page info: {}", e))
        })
    }
}

// Add state() method to NavigateTool for sharing state
impl NavigateTool {
    /// Get the browser state for sharing with other tools.
    pub fn state(&self) -> &BrowserState {
        &self.state
    }
}

// Extended browser automation methods for NatBot support
impl BrowserState {
    /// Scroll the page in the specified direction.
    ///
    /// # Arguments
    /// * `direction` - Either "up" or "down"
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_playwright::BrowserState;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = BrowserState::new().await?;
    /// state.scroll("down").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn scroll(&self, direction: &str) -> Result<()> {
        let page = self.page().await;
        let script = match direction.to_lowercase().as_str() {
            "up" => {
                "(document.scrollingElement || document.body).scrollTop = \
                 (document.scrollingElement || document.body).scrollTop - window.innerHeight;"
            }
            "down" => {
                "(document.scrollingElement || document.body).scrollTop = \
                 (document.scrollingElement || document.body).scrollTop + window.innerHeight;"
            }
            _ => {
                return Err(dashflow::core::Error::tool_error(format!(
                    "Invalid scroll direction: {}. Use 'up' or 'down'",
                    direction
                )))
            }
        };
        page.eval::<()>(script)
            .await
            .map_err(|e| dashflow::core::Error::tool_error(format!("Failed to scroll: {}", e)))?;
        Ok(())
    }

    /// Click at specific screen coordinates.
    ///
    /// # Arguments
    /// * `x` - X coordinate
    /// * `y` - Y coordinate
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_playwright::BrowserState;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = BrowserState::new().await?;
    /// state.click_at(100.0, 200.0).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn click_at(&self, x: f64, y: f64) -> Result<()> {
        let page = self.page().await;
        page.mouse
            .click_builder(x, y)
            .click()
            .await
            .map_err(|e| dashflow::core::Error::tool_error(format!("Failed to click: {}", e)))?;
        Ok(())
    }

    /// Type text at the current cursor position.
    ///
    /// # Arguments
    /// * `text` - Text to type
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_playwright::BrowserState;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = BrowserState::new().await?;
    /// state.type_text("Hello World").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn type_text(&self, text: &str) -> Result<()> {
        let page = self.page().await;
        page.keyboard.r#type(text, None).await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to type text: {}", e))
        })?;
        Ok(())
    }

    /// Press a keyboard key.
    ///
    /// # Arguments
    /// * `key` - Key name (e.g., "Enter", "Escape", "ArrowDown")
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_playwright::BrowserState;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = BrowserState::new().await?;
    /// state.press_key("Enter").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn press_key(&self, key: &str) -> Result<()> {
        let page = self.page().await;
        page.keyboard.press(key, None).await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to press key: {}", e))
        })?;
        Ok(())
    }

    /// Get current page URL.
    pub async fn url(&self) -> Result<String> {
        let page = self.page().await;
        page.url()
            .map_err(|e| dashflow::core::Error::tool_error(format!("Failed to get URL: {}", e)))
    }

    /// Navigate to a URL.
    pub async fn goto(&self, url: &str) -> Result<()> {
        let page = self.page().await;
        page.goto_builder(url).goto().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to navigate: {}", e))
        })?;
        Ok(())
    }

    /// Execute JavaScript and return the result.
    ///
    /// # Arguments
    /// * `script` - JavaScript code to execute
    ///
    /// # Example
    /// ```no_run
    /// # use dashflow_playwright::BrowserState;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let state = BrowserState::new().await?;
    /// let title: String = state.eval("() => document.title").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn eval<T: serde::de::DeserializeOwned>(&self, script: &str) -> Result<T> {
        let page = self.page().await;
        page.eval(script).await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to eval script: {}", e))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_navigate_tool_name() {
        let tool = NavigateTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "navigate");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_navigate_back_tool_name() {
        let tool = NavigateBackTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "navigate_back");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_extract_text_tool_name() {
        let tool = ExtractTextTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "extract_text");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_extract_hyperlinks_tool_name() {
        let tool = ExtractHyperlinksTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "extract_hyperlinks");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_get_elements_tool_name() {
        let tool = GetElementsTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "get_elements");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_click_tool_name() {
        let tool = ClickTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "click");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_current_page_tool_name() {
        let tool = CurrentWebPageTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert_eq!(tool.name(), "current_page");
    }

    #[tokio::test]
    #[ignore = "requires Playwright installed"]
    async fn test_tool_descriptions() {
        let navigate = NavigateTool::new()
            .await
            .expect("Playwright must be installed to run ignored tests");
        assert!(navigate.description().contains("Navigate to a URL"));

        let extract_text = ExtractTextTool::with_state(navigate.state().clone());
        assert!(extract_text
            .description()
            .contains("Extract all visible text"));

        let extract_links = ExtractHyperlinksTool::with_state(navigate.state().clone());
        assert!(extract_links.description().contains("hyperlinks"));
    }
}
