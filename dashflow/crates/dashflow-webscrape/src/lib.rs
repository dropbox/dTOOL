//! # Web Scraping Tools
//!
//! Tools for extracting content from web pages, including text, links, and structured data.
//!
//! ## Features
//!
//! - Extract clean text from HTML pages
//! - Extract all links from a page
//! - Filter content by CSS selectors
//! - Remove scripts, styles, and other non-content elements
//! - Handle relative URLs
//!
//! ## Usage
//!
//! ```no_run
//! use dashflow_webscrape::WebScrapeTool;
//! use dashflow::core::tools::Tool;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let scraper = WebScrapeTool::new();
//!
//! // Scrape a web page
//! let content = scraper._call_str("https://example.com".to_string()).await?;
//! println!("Page content: {}", content);
//! # Ok(())
//! # }
//! ```
//!
//! # See Also
//!
//! - [`Tool`] - The trait this implements
//! - [`dashflow-tavily`](https://docs.rs/dashflow-tavily) - AI-optimized search with built-in summarization
//! - [`dashflow-google-search`](https://docs.rs/dashflow-google-search) - Google Custom Search for web queries
//! - [`dashflow-file-tool`](https://docs.rs/dashflow-file-tool) - File reading/writing tools
//! - [scraper Documentation](https://docs.rs/scraper/) - HTML parsing library

use async_trait::async_trait;
use dashflow::constants::{DEFAULT_HTTP_CONNECT_TIMEOUT, DEFAULT_HTTP_REQUEST_TIMEOUT};
use dashflow::core::tools::{Tool, ToolInput};
use dashflow::core::Result;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
use url::Url;

/// Create an HTTP client with standard timeouts
fn create_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(DEFAULT_HTTP_REQUEST_TIMEOUT)
        .connect_timeout(DEFAULT_HTTP_CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// =============================================================================
// SSRF Protection (M-209)
// =============================================================================
//
// SSRF (Server-Side Request Forgery) allows attackers to make the server fetch
// arbitrary URLs, potentially accessing internal network resources, cloud
// metadata endpoints, or bypassing firewalls.
//
// This module validates URLs before fetching to block:
// - Private IP ranges (RFC 1918: 10.x, 172.16-31.x, 192.168.x)
// - Loopback addresses (127.x.x.x, ::1)
// - Link-local addresses (169.254.x.x, fe80::)
// - Cloud metadata endpoints (169.254.169.254)
// - Non-standard ports (only 80, 443 allowed by default)
// =============================================================================

/// SSRF protection configuration
#[derive(Debug, Clone, Default)]
pub struct SsrfConfig {
    /// Allow requests to private IP ranges (default: false)
    pub allow_private_ips: bool,
    /// Allow requests to localhost (default: false)
    pub allow_localhost: bool,
    /// Allow requests to non-standard ports (default: false)
    pub allow_non_standard_ports: bool,
    /// Allowed domain patterns (empty = allow all public domains)
    pub allowed_domains: Vec<String>,
    /// Additional blocked IP addresses
    pub blocked_ips: Vec<IpAddr>,
}

impl SsrfConfig {
    /// Check if an IP address is a private/internal address
    fn is_private_ip(ip: &IpAddr) -> bool {
        match ip {
            IpAddr::V4(ipv4) => {
                // Private ranges (RFC 1918)
                ipv4.is_private()
                    // Loopback (127.0.0.0/8)
                    || ipv4.is_loopback()
                    // Link-local (169.254.0.0/16)
                    || ipv4.is_link_local()
                    // Multicast (224.0.0.0/4)
                    || ipv4.is_multicast()
                    // Unspecified (0.0.0.0)
                    || ipv4.is_unspecified()
                    // Broadcast
                    || ipv4.is_broadcast()
                    // Documentation ranges (RFC 5737)
                    || Self::is_documentation_ipv4(ipv4)
                    // Shared address space (RFC 6598: 100.64.0.0/10)
                    || Self::is_shared_address_space(ipv4)
                    // Cloud metadata endpoint (AWS/GCP/Azure)
                    || Self::is_cloud_metadata_ipv4(ipv4)
            }
            IpAddr::V6(ipv6) => {
                // Loopback (::1)
                ipv6.is_loopback()
                    // Unspecified (::)
                    || ipv6.is_unspecified()
                    // Multicast (ff00::/8)
                    || ipv6.is_multicast()
                    // Link-local (fe80::/10) - using manual check since is_unicast_link_local is unstable
                    || Self::is_link_local_ipv6(ipv6)
                    // Unique local (fc00::/7)
                    || Self::is_unique_local_ipv6(ipv6)
                    // Documentation prefix (RFC 3849: 2001:db8::/32)
                    || Self::is_documentation_ipv6(ipv6)
                    // IPv4-mapped addresses
                    || ipv6.to_ipv4_mapped().is_some_and(|ipv4| Self::is_private_ip(&IpAddr::V4(ipv4)))
            }
        }
    }

    /// Check if IPv4 is in documentation range (RFC 5737)
    fn is_documentation_ipv4(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        // 192.0.2.0/24 (TEST-NET-1)
        (octets[0] == 192 && octets[1] == 0 && octets[2] == 2)
            // 198.51.100.0/24 (TEST-NET-2)
            || (octets[0] == 198 && octets[1] == 51 && octets[2] == 100)
            // 203.0.113.0/24 (TEST-NET-3)
            || (octets[0] == 203 && octets[1] == 0 && octets[2] == 113)
    }

    /// Check if IPv4 is in shared address space (RFC 6598: 100.64.0.0/10)
    fn is_shared_address_space(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 100 && (octets[1] & 0xC0) == 64
    }

    /// Check if IPv4 is a cloud metadata endpoint
    fn is_cloud_metadata_ipv4(ip: &Ipv4Addr) -> bool {
        let octets = ip.octets();
        // AWS/GCP/Azure metadata: 169.254.169.254
        octets[0] == 169 && octets[1] == 254 && octets[2] == 169 && octets[3] == 254
        // AWS IMDSv2 alternative: fd00:ec2::254 (handled in IPv6)
        // GCP metadata alternate: metadata.google.internal resolves to 169.254.169.254
    }

    /// Check if IPv6 is link-local (fe80::/10)
    fn is_link_local_ipv6(ip: &Ipv6Addr) -> bool {
        let segments = ip.segments();
        (segments[0] & 0xffc0) == 0xfe80
    }

    /// Check if IPv6 is unique local (fc00::/7)
    fn is_unique_local_ipv6(ip: &Ipv6Addr) -> bool {
        let segments = ip.segments();
        (segments[0] & 0xfe00) == 0xfc00
    }

    /// Check if IPv6 is in documentation prefix (RFC 3849: 2001:db8::/32)
    fn is_documentation_ipv6(ip: &Ipv6Addr) -> bool {
        let segments = ip.segments();
        segments[0] == 0x2001 && segments[1] == 0x0db8
    }

    fn validate_url_pre_resolve<'a>(
        &self,
        url: &'a Url,
    ) -> std::result::Result<(url::Host<&'a str>, u16, &'a str), String> {
        // Only allow http and https schemes
        match url.scheme() {
            "http" | "https" => {}
            scheme => {
                return Err(format!(
                    "SSRF protection: scheme '{}' not allowed (only http/https)",
                    scheme
                ))
            }
        }

        // Check port restrictions
        let port = url.port_or_known_default().unwrap_or(80);
        if !self.allow_non_standard_ports && port != 80 && port != 443 {
            return Err(format!(
                "SSRF protection: port {} not allowed (only 80/443)",
                port
            ));
        }

        // Get the host
        let host_str = url
            .host_str()
            .filter(|host| !host.is_empty())
            .ok_or_else(|| "SSRF protection: URL has no host".to_string())?;

        // Check domain allowlist if configured
        if !self.allowed_domains.is_empty() {
            let domain_allowed = self.allowed_domains.iter().any(|pattern| {
                if let Some(suffix) = pattern.strip_prefix("*.") {
                    // Wildcard match: *.example.com matches sub.example.com
                    host_str == suffix || host_str.ends_with(&format!(".{}", suffix))
                } else {
                    host_str == pattern
                }
            });
            if !domain_allowed {
                return Err(format!(
                    "SSRF protection: domain '{}' not in allowlist",
                    host_str
                ));
            }
        }

        let host = url
            .host()
            .ok_or_else(|| "SSRF protection: URL has no host".to_string())?;

        Ok((host, port, host_str))
    }

    fn resolve_host_to_ips(
        host: url::Host<&str>,
        host_str: &str,
        port: u16,
    ) -> std::result::Result<Vec<IpAddr>, String> {
        match host {
            url::Host::Ipv4(ip) => Ok(vec![IpAddr::V4(ip)]),
            url::Host::Ipv6(ip) => Ok(vec![IpAddr::V6(ip)]),
            url::Host::Domain(domain) => {
                let socket_addr = format!("{}:{}", domain, port);
                let resolved_ips: Vec<IpAddr> = socket_addr
                    .to_socket_addrs()
                    .map_err(|e| format!("SSRF protection: failed to resolve '{}': {}", host_str, e))?
                    .map(|addr| addr.ip())
                    .collect();
                Ok(resolved_ips)
            }
        }
    }

    fn validate_resolved_ips(&self, resolved_ips: &[IpAddr]) -> std::result::Result<(), String> {
        for ip in resolved_ips {
            // Check blocked IP list
            if self.blocked_ips.contains(ip) {
                return Err(format!("SSRF protection: IP {} is explicitly blocked", ip));
            }

            // Check private IP ranges
            if !self.allow_private_ips && Self::is_private_ip(ip) {
                return Err(format!(
                    "SSRF protection: IP {} is a private/internal address",
                    ip
                ));
            }

            // Check localhost specifically (even if allow_private_ips is true)
            if !self.allow_localhost && ip.is_loopback() {
                return Err(format!(
                    "SSRF protection: localhost/loopback addresses not allowed ({})",
                    ip
                ));
            }
        }

        Ok(())
    }

    #[cfg(test)]
    fn validate_url_with_resolved_ips(
        &self,
        url: &Url,
        resolved_ips: &[IpAddr],
    ) -> std::result::Result<(), String> {
        let (_host, _port, host_str) = self.validate_url_pre_resolve(url)?;

        if resolved_ips.is_empty() {
            return Err(format!(
                "SSRF protection: hostname '{}' did not resolve to any IP",
                host_str
            ));
        }

        self.validate_resolved_ips(resolved_ips)
    }

    /// Validate a URL for SSRF protection
    ///
    /// Returns Ok(()) if the URL is safe to fetch, or an error describing why not.
    pub fn validate_url(&self, url: &Url) -> std::result::Result<(), String> {
        let (host, port, host_str) = self.validate_url_pre_resolve(url)?;
        let resolved_ips = Self::resolve_host_to_ips(host, host_str, port)?;
        if resolved_ips.is_empty() {
            return Err(format!(
                "SSRF protection: hostname '{}' did not resolve to any IP",
                host_str
            ));
        }

        self.validate_resolved_ips(&resolved_ips)
    }
}

/// Web scraping tool for extracting content from HTML pages
///
/// This tool fetches a URL and extracts clean text content, optionally including
/// links and other structured information.
///
/// **Security:** SSRF protection is enabled by default to prevent requests to
/// private networks, localhost, and cloud metadata endpoints. Use the builder
/// to configure or disable these protections if needed.
///
/// # Example
///
/// ```no_run
/// use dashflow_webscrape::WebScrapeTool;
/// use dashflow::core::tools::Tool;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let scraper = WebScrapeTool::builder()
///     .include_links(true)
///     .max_content_length(5000)
///     .build();
///
/// let content = scraper._call_str("https://example.com".to_string())
///     .await?;
/// println!("Scraped: {}", content);
/// # Ok(())
/// # }
/// ```
pub struct WebScrapeTool {
    include_links: bool,
    max_content_length: usize,
    client: reqwest::Client,
    /// SSRF protection configuration (enabled by default)
    ssrf_config: SsrfConfig,
}

impl WebScrapeTool {
    /// Create a new web scraping tool with default settings
    ///
    /// SSRF protection is enabled by default.
    #[must_use]
    pub fn new() -> Self {
        Self {
            include_links: false,
            max_content_length: 10000,
            client: create_http_client(),
            ssrf_config: SsrfConfig::default(),
        }
    }

    /// Create a builder for `WebScrapeTool`
    #[must_use]
    pub fn builder() -> WebScrapeToolBuilder {
        WebScrapeToolBuilder::default()
    }

    /// Fetch and scrape a URL
    async fn scrape(&self, url_str: String) -> Result<ScrapedContent> {
        // Validate URL
        let url = Url::parse(&url_str).map_err(|e| {
            dashflow::core::Error::tool_error(format!("Invalid URL '{url_str}': {e}"))
        })?;

        // SSRF protection: validate URL before fetching
        self.ssrf_config.validate_url(&url).map_err(|e| {
            dashflow::core::Error::tool_error(e)
        })?;

        // Fetch the page
        let response =
            self.client.get(url.as_str()).send().await.map_err(|e| {
                dashflow::core::Error::tool_error(format!("Failed to fetch URL: {e}"))
            })?;

        if !response.status().is_success() {
            return Err(dashflow::core::Error::tool_error(format!(
                "HTTP error {}: Failed to fetch URL",
                response.status()
            )));
        }

        let html_content = response.text().await.map_err(|e| {
            dashflow::core::Error::tool_error(format!("Failed to read response body: {e}"))
        })?;

        // Parse HTML
        let document = Html::parse_document(&html_content);

        // Extract title
        let title = Self::extract_title(&document);

        // Extract text content
        let text = Self::extract_text(&document);

        // Extract links if requested
        let links = if self.include_links {
            Some(Self::extract_links(&document, &url))
        } else {
            None
        };

        Ok(ScrapedContent {
            url: url_str,
            title,
            text,
            links,
        })
    }

    /// Extract the page title
    fn extract_title(document: &Html) -> Option<String> {
        let title_selector = Selector::parse("title").ok()?;
        document
            .select(&title_selector)
            .next()
            .map(|el| {
                el.text()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
    }

    /// Extract clean text content from the page
    fn extract_text(document: &Html) -> String {
        // Extract visible text, excluding non-content tags (script/style/noscript).
        let mut text_parts = Vec::new();

        // Try to get main content areas first
        let content_selectors = vec!["article", "main", ".content", "#content", "body"];

        for selector_str in content_selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                for element in document.select(&selector) {
                    let text = Self::extract_visible_text(&element);
                    if !text.trim().is_empty() {
                        text_parts.push(text);
                        break; // Use first matching content area
                    }
                }
                if !text_parts.is_empty() {
                    break;
                }
            }
        }

        // If no content area found, fall back to body
        if text_parts.is_empty() {
            if let Ok(body_selector) = Selector::parse("body") {
                for element in document.select(&body_selector) {
                    let text = Self::extract_visible_text(&element);
                    text_parts.push(text);
                }
            }
        }

        // Clean up the text
        let full_text = text_parts.join("\n");
        Self::clean_text(&full_text)
    }

    /// Clean extracted text by removing extra whitespace
    fn clean_text(text: &str) -> String {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn extract_visible_text(element: &scraper::ElementRef<'_>) -> String {
        use ego_tree::iter::Edge;
        let mut text_chunks: Vec<&str> = Vec::new();
        let mut skip_depth = 0usize;

        for edge in element.traverse() {
            match edge {
                Edge::Open(node) => match node.value() {
                    scraper::Node::Element(el) => {
                        if matches!(el.name(), "script" | "style" | "noscript") {
                            skip_depth += 1;
                        }
                    }
                    scraper::Node::Text(text) => {
                        if skip_depth == 0 {
                            text_chunks.push(text);
                        }
                    }
                    _ => {}
                },
                Edge::Close(node) => {
                    if let scraper::Node::Element(el) = node.value() {
                        if matches!(el.name(), "script" | "style" | "noscript") && skip_depth > 0 {
                            skip_depth -= 1;
                        }
                    }
                }
            }
        }

        text_chunks.join(" ")
    }

    /// Extract all links from the page
    #[allow(clippy::unwrap_used)] // Static CSS selector "a[href]" is always valid
    fn extract_links(document: &Html, base_url: &Url) -> Vec<Link> {
        let link_selector = Selector::parse("a[href]").unwrap();
        let mut links = Vec::new();

        for element in document.select(&link_selector) {
            if let Some(href) = element.value().attr("href") {
                // Resolve relative URLs
                if let Ok(absolute_url) = base_url.join(href) {
                    let text = element
                        .text()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .trim()
                        .to_string();
                    links.push(Link {
                        url: absolute_url.to_string(),
                        text: (!text.is_empty()).then_some(text),
                    });
                }
            }
        }

        links
    }

    /// Format scraped content as a string
    fn format_content(&self, content: ScrapedContent) -> String {
        let ScrapedContent {
            url,
            title,
            text,
            links,
        } = content;

        let mut output = format!("URL: {}\n\n", url);

        if let Some(title) = title {
            output.push_str(&format!("Title: {title}\n\n"));
        }

        output.push_str("Content:\n");

        // Truncate if too long (by character count, not bytes).
        if self.max_content_length == 0 {
            output.push_str(&format!(
                "...\n\n[Content truncated to {} characters]",
                self.max_content_length
            ));
        } else if let Some((byte_idx, _)) = text.char_indices().nth(self.max_content_length) {
            output.push_str(&format!(
                "{}...\n\n[Content truncated to {} characters]",
                &text[..byte_idx],
                self.max_content_length
            ));
        } else {
            output.push_str(&text);
        }
        output.push_str("\n\n");

        if let Some(links) = links {
            if !links.is_empty() {
                output.push_str(&format!("Links ({}):\n", links.len()));
                for (i, link) in links.iter().take(20).enumerate() {
                    if let Some(text) = &link.text {
                        output.push_str(&format!("{}. {} - {}\n", i + 1, text, link.url));
                    } else {
                        output.push_str(&format!("{}. {}\n", i + 1, link.url));
                    }
                }
                if links.len() > 20 {
                    output.push_str(&format!("\n... and {} more links\n", links.len() - 20));
                }
            }
        }

        output
    }
}

impl Default for WebScrapeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebScrapeTool {
    fn name(&self) -> &'static str {
        "web_scrape"
    }

    fn description(&self) -> &'static str {
        "Scrapes content from a web page given its URL. \
         Extracts clean text content, page title, and optionally links. \
         Removes scripts, styles, and other non-content elements. \
         Returns formatted text content suitable for analysis."
    }

    fn args_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL of the web page to scrape"
                }
            },
            "required": ["url"]
        })
    }

    async fn _call(&self, input: ToolInput) -> Result<String> {
        let url = match input {
            ToolInput::String(s) => s,
            ToolInput::Structured(v) => v
                .get("url")
                .and_then(|u| u.as_str())
                .ok_or_else(|| {
                    dashflow::core::Error::tool_error(
                        "Missing 'url' field in structured input".to_string(),
                    )
                })?
                .to_string(),
        };

        let content = self.scrape(url).await?;
        Ok(self.format_content(content))
    }
}

/// Scraped content from a web page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrapedContent {
    /// The URL that was scraped
    pub url: String,
    /// Page title
    pub title: Option<String>,
    /// Extracted text content
    pub text: String,
    /// Links found on the page
    pub links: Option<Vec<Link>>,
}

/// A link extracted from a web page
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    /// The URL of the link
    pub url: String,
    /// The link text (anchor text)
    pub text: Option<String>,
}

/// Builder for `WebScrapeTool`
#[derive(Default)]
pub struct WebScrapeToolBuilder {
    include_links: Option<bool>,
    max_content_length: Option<usize>,
    ssrf_config: Option<SsrfConfig>,
}

impl WebScrapeToolBuilder {
    /// Include links in the scraped output
    #[must_use]
    pub fn include_links(mut self, include: bool) -> Self {
        self.include_links = Some(include);
        self
    }

    /// Set maximum content length before truncation
    #[must_use]
    pub fn max_content_length(mut self, length: usize) -> Self {
        self.max_content_length = Some(length);
        self
    }

    /// Set custom SSRF protection configuration
    ///
    /// By default, SSRF protection blocks requests to private IPs, localhost,
    /// and cloud metadata endpoints. Use this to customize the behavior.
    #[must_use]
    pub fn ssrf_config(mut self, config: SsrfConfig) -> Self {
        self.ssrf_config = Some(config);
        self
    }

    /// Allow requests to private IP ranges (10.x, 172.16-31.x, 192.168.x)
    ///
    /// **WARNING:** This disables SSRF protection for internal networks.
    /// Only use in trusted environments.
    #[must_use]
    pub fn allow_private_ips(mut self, allow: bool) -> Self {
        let config = self.ssrf_config.get_or_insert_with(SsrfConfig::default);
        config.allow_private_ips = allow;
        self
    }

    /// Allow requests to localhost (127.x.x.x, ::1)
    ///
    /// **WARNING:** This allows the tool to access local services.
    /// Only use in trusted environments.
    #[must_use]
    pub fn allow_localhost(mut self, allow: bool) -> Self {
        let config = self.ssrf_config.get_or_insert_with(SsrfConfig::default);
        config.allow_localhost = allow;
        self
    }

    /// Restrict to specific domains only
    ///
    /// Supports wildcards: `*.example.com` matches `sub.example.com`
    #[must_use]
    pub fn allowed_domains(mut self, domains: Vec<String>) -> Self {
        let config = self.ssrf_config.get_or_insert_with(SsrfConfig::default);
        config.allowed_domains = domains;
        self
    }

    /// Build the `WebScrapeTool`
    #[must_use]
    pub fn build(self) -> WebScrapeTool {
        WebScrapeTool {
            include_links: self.include_links.unwrap_or(false),
            max_content_length: self.max_content_length.unwrap_or(10000),
            client: create_http_client(),
            ssrf_config: self.ssrf_config.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dashflow::core::tools::Tool;

    fn url(url: &str) -> Url {
        Url::parse(url).unwrap()
    }

    fn ip4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    fn ip6(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(a, b, c, d, e, f, g, h))
    }

    fn validate_with_ips(
        config: &SsrfConfig,
        url_str: &str,
        resolved_ips: &[IpAddr],
    ) -> std::result::Result<(), String> {
        let url = url(url_str);
        config.validate_url_with_resolved_ips(&url, resolved_ips)
    }

    #[test]
    fn test_webscrape_tool_creation() {
        let scraper = WebScrapeTool::new();
        assert_eq!(scraper.name(), "web_scrape");
        assert!(scraper.description().contains("Scrapes content"));
        assert!(!scraper.include_links);
        assert_eq!(scraper.max_content_length, 10000);
        assert!(!scraper.ssrf_config.allow_private_ips);
        assert!(!scraper.ssrf_config.allow_localhost);
        assert!(!scraper.ssrf_config.allow_non_standard_ports);
        assert!(scraper.ssrf_config.allowed_domains.is_empty());
        assert!(scraper.ssrf_config.blocked_ips.is_empty());
    }

    #[test]
    fn test_webscrape_tool_builder() {
        let scraper = WebScrapeTool::builder()
            .include_links(true)
            .max_content_length(5000)
            .build();

        assert!(scraper.include_links);
        assert_eq!(scraper.max_content_length, 5000);
    }

    #[test]
    fn test_webscrape_tool_builder_defaults() {
        let scraper = WebScrapeTool::builder().build();
        assert!(!scraper.include_links);
        assert_eq!(scraper.max_content_length, 10000);
        assert!(!scraper.ssrf_config.allow_private_ips);
        assert!(!scraper.ssrf_config.allow_localhost);
        assert!(!scraper.ssrf_config.allow_non_standard_ports);
    }

    #[test]
    fn test_args_schema() {
        let scraper = WebScrapeTool::new();
        let schema = scraper.args_schema();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["url"].is_object());
        assert_eq!(schema["required"][0], "url");
    }

    #[test]
    fn test_clean_text() {
        let input = "  Line 1  \n\n  Line 2  \n   \n  Line 3  ";
        let cleaned = WebScrapeTool::clean_text(input);
        assert_eq!(cleaned, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_clean_text_windows_newlines_and_tabs() {
        let input = "\tLine 1\r\n\r\n\tLine 2\r\n  \r\nLine 3\t";
        let cleaned = WebScrapeTool::clean_text(input);
        assert_eq!(cleaned, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_clean_text_all_whitespace_returns_empty() {
        let input = "   \n\t\r\n  \n";
        let cleaned = WebScrapeTool::clean_text(input);
        assert_eq!(cleaned, "");
    }

    #[test]
    fn test_extract_title() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Page Title</title></head>
            <body><p>Content</p></body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let title = WebScrapeTool::extract_title(&document);
        assert_eq!(title, Some("Test Page Title".to_string()));
    }

    #[test]
    fn test_extract_title_missing_returns_none() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head></head>
            <body><p>Content</p></body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let title = WebScrapeTool::extract_title(&document);
        assert_eq!(title, None);
    }

    #[test]
    fn test_extract_title_trims_whitespace() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>
              Title With    Whitespace
            </title></head>
            <body></body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let title = WebScrapeTool::extract_title(&document);
        assert_eq!(title, Some("Title With Whitespace".to_string()));
    }

    #[test]
    fn test_extract_text() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <article>
                    <h1>Heading</h1>
                    <p>Paragraph 1</p>
                    <p>Paragraph 2</p>
                </article>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let text = WebScrapeTool::extract_text(&document);
        assert!(text.contains("Heading"));
        assert!(text.contains("Paragraph 1"));
        assert!(text.contains("Paragraph 2"));
    }

    #[test]
    fn test_extract_text_prefers_article_over_body() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <article><p>Preferred</p></article>
                <p>Fallback</p>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let text = WebScrapeTool::extract_text(&document);
        assert!(text.contains("Preferred"));
        assert!(!text.contains("Fallback"));
    }

    #[test]
    fn test_extract_text_prefers_main_when_no_article() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <main><p>Main Content</p></main>
                <div class="content"><p>Secondary</p></div>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let text = WebScrapeTool::extract_text(&document);
        assert!(text.contains("Main Content"));
        assert!(!text.contains("Secondary"));
    }

    #[test]
    fn test_extract_text_excludes_script_style_noscript() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <article>
                    <p>Visible</p>
                    <script>console.log("SECRET");</script>
                    <style>body { display: none; }</style>
                    <noscript>Hidden</noscript>
                </article>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let text = WebScrapeTool::extract_text(&document);
        assert!(text.contains("Visible"));
        assert!(!text.contains("SECRET"));
        assert!(!text.contains("display"));
        assert!(!text.contains("Hidden"));
    }

    #[test]
    fn test_extract_visible_text_excludes_nested_script() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <div id="content">
                    <p>Text 1</p>
                    <div>
                        <script>nested()</script>
                        <p>Text 2</p>
                    </div>
                </div>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let selector = Selector::parse("#content").unwrap();
        let el = document.select(&selector).next().unwrap();
        let text = WebScrapeTool::extract_visible_text(&el);
        assert!(text.contains("Text 1"));
        assert!(text.contains("Text 2"));
        assert!(!text.contains("nested"));
    }

    #[test]
    fn test_extract_links() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <a href="https://example.com">Example</a>
                <a href="/relative">Relative Link</a>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let base_url = Url::parse("https://test.com").unwrap();
        let links = WebScrapeTool::extract_links(&document, &base_url);

        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/");
        assert_eq!(links[0].text, Some("Example".to_string()));
        assert_eq!(links[1].url, "https://test.com/relative");
    }

    #[test]
    fn test_extract_links_resolves_relative_with_base_path() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <a href="./child">Child</a>
                <a href="../parent">Parent</a>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let base_url = Url::parse("https://example.com/a/b/").unwrap();
        let links = WebScrapeTool::extract_links(&document, &base_url);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].url, "https://example.com/a/b/child");
        assert_eq!(links[1].url, "https://example.com/a/parent");
    }

    #[test]
    fn test_extract_links_empty_anchor_text_sets_none() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <a href="https://example.com"></a>
                <a href="https://example.com/2">   </a>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let base_url = Url::parse("https://test.com").unwrap();
        let links = WebScrapeTool::extract_links(&document, &base_url);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].text, None);
        assert_eq!(links[1].text, None);
    }

    #[test]
    fn test_extract_links_ignores_invalid_urls() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <body>
                <a href="https://example.com">Ok</a>
                <a href="http://[::1">Bad</a>
            </body>
            </html>
        "#;
        let document = Html::parse_document(html);
        let base_url = Url::parse("https://test.com").unwrap();
        let links = WebScrapeTool::extract_links(&document, &base_url);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].url, "https://example.com/");
    }

    // ==========================================================================
    // format_content() Tests
    // ==========================================================================

    #[test]
    fn test_format_content_includes_url_title_and_content() {
        let scraper = WebScrapeTool::new();
        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: Some("Example".to_string()),
            text: "Hello\nWorld".to_string(),
            links: None,
        };
        let out = scraper.format_content(content);
        assert!(out.contains("URL: https://example.com"));
        assert!(out.contains("Title: Example"));
        assert!(out.contains("Content:\nHello\nWorld"));
    }

    #[test]
    fn test_format_content_omits_title_when_none() {
        let scraper = WebScrapeTool::new();
        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: None,
            text: "Hello".to_string(),
            links: None,
        };
        let out = scraper.format_content(content);
        assert!(out.contains("URL: https://example.com"));
        assert!(!out.contains("Title:"));
        assert!(out.contains("Content:\nHello"));
    }

    #[test]
    fn test_format_content_truncates_by_char_count_and_is_utf8_safe() {
        let scraper = WebScrapeTool::builder().max_content_length(5).build();
        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: None,
            text: "abcdéFGHIJ".to_string(),
            links: None,
        };
        let out = scraper.format_content(content);
        assert!(out.contains("abcdé..."));
        assert!(out.contains("[Content truncated to 5 characters]"));
    }

    #[test]
    fn test_format_content_max_length_zero_is_handled() {
        let scraper = WebScrapeTool::builder().max_content_length(0).build();
        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: None,
            text: "Hello".to_string(),
            links: None,
        };
        let out = scraper.format_content(content);
        assert!(out.contains("...\n\n[Content truncated to 0 characters]"));
    }

    #[test]
    fn test_format_content_lists_links_with_and_without_text() {
        let scraper = WebScrapeTool::builder().include_links(true).build();
        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: None,
            text: "Text".to_string(),
            links: Some(vec![
                Link {
                    url: "https://a.example/".to_string(),
                    text: Some("A".to_string()),
                },
                Link {
                    url: "https://b.example/".to_string(),
                    text: None,
                },
            ]),
        };
        let out = scraper.format_content(content);
        assert!(out.contains("Links (2):"));
        assert!(out.contains("1. A - https://a.example/"));
        assert!(out.contains("2. https://b.example/"));
    }

    #[test]
    fn test_format_content_limits_link_output_to_20() {
        let scraper = WebScrapeTool::builder().include_links(true).build();
        let links = (0..25)
            .map(|i| Link {
                url: format!("https://example.com/{}", i),
                text: None,
            })
            .collect::<Vec<_>>();

        let content = ScrapedContent {
            url: "https://example.com".to_string(),
            title: None,
            text: "Text".to_string(),
            links: Some(links),
        };
        let out = scraper.format_content(content);
        assert!(out.contains("Links (25):"));
        assert!(out.contains("... and 5 more links"));
        assert!(!out.contains("https://example.com/24"));
    }

    // ==========================================================================
    // Tool::_call() Input Validation (no network)
    // ==========================================================================

    #[tokio::test]
    async fn test_tool_call_rejects_non_http_scheme_before_fetch() {
        let tool = WebScrapeTool::new();
        let err = tool
            ._call_str("file:///etc/passwd".to_string())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("scheme"));
    }

    #[tokio::test]
    async fn test_tool_call_rejects_invalid_url_before_fetch() {
        let tool = WebScrapeTool::new();
        let err = tool
            ._call_str("not a url".to_string())
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Invalid URL"));
    }

    #[tokio::test]
    async fn test_tool_call_structured_missing_url_field() {
        let tool = WebScrapeTool::new();
        let err = tool
            ._call(ToolInput::Structured(json!({})))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Missing 'url' field"));
    }

    #[tokio::test]
    async fn test_tool_call_structured_url_wrong_type() {
        let tool = WebScrapeTool::new();
        let err = tool
            ._call(ToolInput::Structured(json!({ "url": 123 })))
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Missing 'url' field"));
    }

    // ==========================================================================
    // SSRF Protection Tests (M-209)
    // ==========================================================================

    #[test]
    fn test_ssrf_blocks_private_ip_10x() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://10.0.0.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_private_ip_172x() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://172.16.0.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_private_ip_192x() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://192.168.1.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_localhost_127() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://127.0.0.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        // Could be "loopback" or "private/internal" - both are correct
        let err = result.unwrap_err();
        assert!(err.contains("loopback") || err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_localhost_name() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://localhost/").unwrap();
        let result = config.validate_url_with_resolved_ips(&url, &[ip4(127, 0, 0, 1)]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_cloud_metadata_aws() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://169.254.169.254/latest/meta-data/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_file_scheme() {
        let config = SsrfConfig::default();
        let url = Url::parse("file:///etc/passwd").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("scheme"));
    }

    #[test]
    fn test_ssrf_blocks_non_standard_ports() {
        let config = SsrfConfig::default();
        let url = Url::parse("http://example.com:8080/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("port"));
    }

    #[test]
    fn test_ssrf_allows_standard_https_port() {
        let config = SsrfConfig::default();
        let result =
            validate_with_ips(&config, "https://example.com:443/", &[ip4(93, 184, 216, 34)]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_allows_public_ipv6_when_resolved_for_domain() {
        let config = SsrfConfig::default();
        let result = validate_with_ips(
            &config,
            "https://example.com:443/",
            &[ip6(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)],
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_config_allow_private_ips() {
        let mut config = SsrfConfig::default();
        config.allow_private_ips = true;
        let url = Url::parse("http://192.168.1.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_config_allow_localhost() {
        let mut config = SsrfConfig::default();
        config.allow_localhost = true;
        config.allow_private_ips = true; // localhost is also a private IP
        let url = Url::parse("http://127.0.0.1/").unwrap();
        let result = config.validate_url(&url);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_domain_allowlist() {
        let mut config = SsrfConfig::default();
        config.allowed_domains = vec!["example.com".to_string()];

        let result1 =
            validate_with_ips(&config, "https://example.com/page", &[ip4(93, 184, 216, 34)]);
        assert!(result1.is_ok());

        // Non-allowed domain should be blocked
        let url2 = Url::parse("https://other.com/page").unwrap();
        let result2 = config.validate_url(&url2);
        assert!(result2.is_err());
        assert!(result2.unwrap_err().contains("not in allowlist"));
    }

    #[test]
    fn test_ssrf_wildcard_domain_allowlist() {
        let mut config = SsrfConfig::default();
        config.allowed_domains = vec!["*.example.com".to_string()];

        let result1 = validate_with_ips(
            &config,
            "https://sub.example.com/page",
            &[ip4(93, 184, 216, 34)],
        );
        assert!(result1.is_ok());

        let result2 =
            validate_with_ips(&config, "https://example.com/page", &[ip4(93, 184, 216, 34)]);
        assert!(result2.is_ok());
    }

    #[test]
    fn test_ssrf_wildcard_allowlist_does_not_match_similar_suffix() {
        let mut config = SsrfConfig::default();
        config.allowed_domains = vec!["*.example.com".to_string()];
        let err = validate_with_ips(
            &config,
            "https://evil-example.com/page",
            &[ip4(93, 184, 216, 34)],
        )
        .unwrap_err();
        assert!(err.contains("not in allowlist"));
    }

    #[test]
    fn test_ssrf_blocks_ftp_scheme() {
        let config = SsrfConfig::default();
        let err = validate_with_ips(&config, "ftp://example.com/", &[ip4(93, 184, 216, 34)])
            .unwrap_err();
        assert!(err.contains("scheme"));
    }

    #[test]
    fn test_ssrf_blocks_data_scheme() {
        let config = SsrfConfig::default();
        let err = validate_with_ips(&config, "data:text/plain,hello", &[ip4(93, 184, 216, 34)])
            .unwrap_err();
        assert!(err.contains("scheme"));
    }

    #[test]
    fn test_ssrf_blocks_explicitly_blocked_ip() {
        let mut config = SsrfConfig::default();
        config.blocked_ips.push(ip4(93, 184, 216, 34));
        let err = validate_with_ips(&config, "https://example.com/", &[ip4(93, 184, 216, 34)])
            .unwrap_err();
        assert!(err.contains("explicitly blocked"));
    }

    #[test]
    fn test_ssrf_allows_public_ipv4_literal() {
        let config = SsrfConfig::default();
        let result = config.validate_url(&url("http://8.8.8.8/"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_allows_public_ipv6_literal() {
        let config = SsrfConfig::default();
        let result = config.validate_url(&url("http://[2001:4860:4860::8888]/"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_blocks_ipv6_loopback_literal() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://[::1]/")).unwrap_err();
        assert!(err.contains("private/internal") || err.contains("loopback"));
    }

    #[test]
    fn test_ssrf_blocks_ipv6_documentation_prefix() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://[2001:db8::1]/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv6_link_local() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://[fe80::1]/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv6_unique_local() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://[fc00::1]/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv6_multicast() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://[ff02::1]/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv4_broadcast() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://255.255.255.255/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv4_shared_address_space() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://100.64.0.1/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv4_documentation_range() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://192.0.2.1/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv4_unspecified() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://0.0.0.0/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_blocks_ipv4_multicast() {
        let config = SsrfConfig::default();
        let err = config.validate_url(&url("http://224.0.0.1/")).unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_allows_non_standard_ports_when_enabled() {
        let mut config = SsrfConfig::default();
        config.allow_non_standard_ports = true;
        let result =
            validate_with_ips(&config, "http://example.com:8080/", &[ip4(93, 184, 216, 34)]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ssrf_private_ips_allowed_but_loopback_still_blocked_by_default() {
        let mut config = SsrfConfig::default();
        config.allow_private_ips = true;
        let err = config.validate_url(&url("http://127.0.0.1/")).unwrap_err();
        assert!(err.contains("loopback") || err.contains("localhost"));
    }

    #[test]
    fn test_ssrf_ipv6_mapped_ipv4_is_treated_as_private() {
        let config = SsrfConfig::default();
        let err = config
            .validate_url(&url("http://[::ffff:10.0.0.1]/"))
            .unwrap_err();
        assert!(err.contains("private/internal"));
    }

    #[test]
    fn test_ssrf_ip_helper_functions() {
        // Test documentation IPs (RFC 5737)
        assert!(SsrfConfig::is_documentation_ipv4(&Ipv4Addr::new(192, 0, 2, 1)));
        assert!(SsrfConfig::is_documentation_ipv4(&Ipv4Addr::new(198, 51, 100, 1)));
        assert!(SsrfConfig::is_documentation_ipv4(&Ipv4Addr::new(203, 0, 113, 1)));
        assert!(!SsrfConfig::is_documentation_ipv4(&Ipv4Addr::new(8, 8, 8, 8)));

        // Test shared address space (RFC 6598)
        assert!(SsrfConfig::is_shared_address_space(&Ipv4Addr::new(100, 64, 0, 1)));
        assert!(SsrfConfig::is_shared_address_space(&Ipv4Addr::new(100, 127, 255, 254)));
        assert!(!SsrfConfig::is_shared_address_space(&Ipv4Addr::new(100, 63, 0, 1)));

        // Test cloud metadata
        assert!(SsrfConfig::is_cloud_metadata_ipv4(&Ipv4Addr::new(169, 254, 169, 254)));
        assert!(!SsrfConfig::is_cloud_metadata_ipv4(&Ipv4Addr::new(169, 254, 1, 1)));

        // Test IPv6 link-local
        assert!(SsrfConfig::is_link_local_ipv6(&Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1)));
        assert!(!SsrfConfig::is_link_local_ipv6(&Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)));

        // Test IPv6 unique local
        assert!(SsrfConfig::is_unique_local_ipv6(&Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 1)));
        assert!(SsrfConfig::is_unique_local_ipv6(&Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)));
        assert!(!SsrfConfig::is_unique_local_ipv6(&Ipv6Addr::new(0x2001, 0, 0, 0, 0, 0, 0, 1)));

        // Test IPv6 documentation prefix (RFC 3849)
        assert!(SsrfConfig::is_documentation_ipv6(&Ipv6Addr::new(
            0x2001, 0x0db8, 0, 0, 0, 0, 0, 1
        )));
        assert!(!SsrfConfig::is_documentation_ipv6(&Ipv6Addr::new(
            0x2001, 0x4860, 0, 0, 0, 0, 0, 1
        )));
    }

    #[test]
    fn test_ssrf_builder_config() {
        let scraper = WebScrapeTool::builder()
            .allow_localhost(true)
            .allow_private_ips(true)
            .allowed_domains(vec!["example.com".to_string()])
            .build();

        assert!(scraper.ssrf_config.allow_localhost);
        assert!(scraper.ssrf_config.allow_private_ips);
        assert_eq!(scraper.ssrf_config.allowed_domains, vec!["example.com"]);
    }
}
