//! Markdown rendering component.
//!
//! Converts markdown text to styled terminal output using the inky node tree.
//!
//! # Example
//!
//! ```
//! use inky::prelude::*;
//! use inky::components::Markdown;
//!
//! let content = "# Hello\n\nThis is **bold** and `code`.";
//! let md = Markdown::new(content);
//! let node = md.to_node();
//! ```
//!
//! # Supported Markdown Features
//!
//! - **Headings** (`# H1` through `###### H6`) - Different visual styles per level
//! - **Bold** (`**text**`) - Bold text styling
//! - **Italic** (`*text*`) - Italic text styling
//! - **Strikethrough** (`~~text~~`) - Strikethrough styling
//! - **Inline code** (`` `code` ``) - Highlighted inline code
//! - **Code blocks** (triple backticks) - Indented code blocks
//! - **Lists** (unordered `- item` and ordered `1. item`)
//! - **Links** (`[text](url)`) - Displayed as text with URL in parentheses
//! - **Blockquotes** (`> text`) - Indented quoted text with border marker

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::borrow::Cow;

use crate::components::adaptive::{AdaptiveComponent, Tier0Fallback, TierFeatures};
use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection, TextWrap};
use crate::terminal::RenderTier;

/// Code block color theme.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodeTheme {
    /// Dark theme with cyan code text.
    #[default]
    Dark,
    /// Light theme with dark gray code text.
    Light,
}

#[cfg(feature = "syntax-highlighting")]
use super::syntax::{SyntaxHighlighter, SyntaxTheme};

/// Markdown component for rendering markdown to terminal.
///
/// This component parses markdown text and converts it to an inky node tree
/// that can be rendered in the terminal with appropriate styling.
///
/// # Example
///
/// ```
/// use inky::prelude::*;
/// use inky::components::Markdown;
///
/// // Simple markdown (zero-copy for string literals)
/// let md = Markdown::new("**Hello** World!");
/// let node = md.to_node();
///
/// // Markdown with code theme
/// let md = Markdown::new("```rust\nfn main() {}\n```")
///     .code_theme(inky::components::CodeTheme::Light);
/// ```
///
/// The lifetime parameter allows zero-copy usage with string literals and borrowed
/// content, avoiding allocation when the content is known to outlive the Markdown
/// component.
#[derive(Debug, Clone)]
pub struct Markdown<'a> {
    /// The markdown content to render (Cow for zero-copy static strings).
    content: Cow<'a, str>,
    /// Color theme for code blocks.
    code_theme: CodeTheme,
    /// Whether syntax highlighting is enabled.
    #[cfg(feature = "syntax-highlighting")]
    syntax_highlighting: bool,
    /// Syntax highlighting theme.
    #[cfg(feature = "syntax-highlighting")]
    syntax_theme: SyntaxTheme,
}

impl<'a> Markdown<'a> {
    /// Create a new Markdown component with the given content.
    ///
    /// For string literals, this is zero-copy. For owned strings, it takes ownership.
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::Markdown;
    ///
    /// // Zero-copy for string literals
    /// let md = Markdown::new("# Hello\n\nWorld");
    ///
    /// // Takes ownership of String
    /// let owned = String::from("# Dynamic");
    /// let md = Markdown::new(owned);
    /// ```
    pub fn new(content: impl Into<Cow<'a, str>>) -> Self {
        Self {
            content: content.into(),
            code_theme: CodeTheme::default(),
            #[cfg(feature = "syntax-highlighting")]
            syntax_highlighting: true, // Enable by default when feature is on
            #[cfg(feature = "syntax-highlighting")]
            syntax_theme: SyntaxTheme::default(),
        }
    }

    /// Set the code theme for code blocks (fallback when syntax highlighting is disabled).
    ///
    /// # Example
    ///
    /// ```
    /// use inky::components::{Markdown, CodeTheme};
    ///
    /// let md = Markdown::new("```\ncode\n```")
    ///     .code_theme(CodeTheme::Light);
    /// ```
    pub fn code_theme(mut self, theme: CodeTheme) -> Self {
        self.code_theme = theme;
        self
    }

    /// Enable or disable syntax highlighting for code blocks.
    ///
    /// Requires the `syntax-highlighting` feature.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use inky::components::Markdown;
    ///
    /// let md = Markdown::new("```rust\nfn main() {}\n```")
    ///     .syntax_highlighting(true);
    /// ```
    #[cfg(feature = "syntax-highlighting")]
    pub fn syntax_highlighting(mut self, enabled: bool) -> Self {
        self.syntax_highlighting = enabled;
        self
    }

    /// Set the syntax highlighting theme.
    ///
    /// Requires the `syntax-highlighting` feature.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use inky::components::{Markdown, SyntaxTheme};
    ///
    /// let md = Markdown::new("```rust\nfn main() {}\n```")
    ///     .syntax_theme(SyntaxTheme::SolarizedDark);
    /// ```
    #[cfg(feature = "syntax-highlighting")]
    pub fn syntax_theme(mut self, theme: SyntaxTheme) -> Self {
        self.syntax_theme = theme;
        self
    }

    /// Convert the markdown to an inky Node tree.
    ///
    /// Returns a [`BoxNode`] containing the rendered markdown content with
    /// appropriate styling for headings, bold, italic, code, lists, etc.
    pub fn to_node(&self) -> Node {
        #[cfg(feature = "syntax-highlighting")]
        let renderer =
            MarkdownRenderer::new(self.code_theme, self.syntax_highlighting, self.syntax_theme);
        #[cfg(not(feature = "syntax-highlighting"))]
        let renderer = MarkdownRenderer::new(self.code_theme);
        renderer.render(&self.content)
    }
}

impl<'a> From<Markdown<'a>> for Node {
    fn from(md: Markdown<'a>) -> Self {
        md.to_node()
    }
}

impl<'a> AdaptiveComponent for Markdown<'a> {
    fn render_for_tier(&self, tier: RenderTier) -> Node {
        match tier {
            RenderTier::Tier0Fallback => self.render_tier0(),
            RenderTier::Tier1Ansi => self.render_tier1(),
            RenderTier::Tier2Retained | RenderTier::Tier3Gpu => self.to_node(),
        }
    }

    fn tier_features(&self) -> TierFeatures {
        TierFeatures::new("Markdown")
            .tier0("Plain text summary with word/line counts")
            .tier1("Structured text with ASCII markers, no colors")
            .tier2("Full styled rendering with colors and formatting")
            .tier3("Full rendering with GPU acceleration")
    }

    fn minimum_tier(&self) -> Option<RenderTier> {
        None // Works at all tiers
    }
}

impl<'a> Markdown<'a> {
    /// Render Tier 0: Text-only summary.
    ///
    /// Shows a plain text summary with statistics about the content.
    /// No markdown rendering is performed.
    fn render_tier0(&self) -> Node {
        let word_count = self.content.split_whitespace().count();
        let line_count = self.content.lines().count();
        let has_code = self.content.contains("```") || self.content.contains('`');
        let has_headings = self.content.lines().any(|l| l.starts_with('#'));
        let has_lists = self.content.lines().any(|l| {
            let trimmed = l.trim_start();
            trimmed.starts_with("- ")
                || trimmed.starts_with("* ")
                || trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
                    && trimmed.contains(". ")
        });

        let mut fallback = Tier0Fallback::new("Markdown");
        fallback = fallback.stat("words", word_count.to_string());
        fallback = fallback.stat("lines", line_count.to_string());

        if has_headings {
            fallback = fallback.stat("headings", "yes".to_string());
        }
        if has_code {
            fallback = fallback.stat("code", "yes".to_string());
        }
        if has_lists {
            fallback = fallback.stat("lists", "yes".to_string());
        }

        fallback.into()
    }

    /// Render Tier 1: Plain ASCII with structure but no colors.
    ///
    /// Renders markdown with ASCII markers but without ANSI colors or styling.
    fn render_tier1(&self) -> Node {
        let renderer = MarkdownRendererTier1::new();
        renderer.render(&self.content)
    }
}

/// Internal renderer that processes markdown events.
struct MarkdownRenderer {
    code_theme: CodeTheme,
    /// Stack of inline styles (bold, italic, etc.).
    style_stack: Vec<InlineStyle>,
    /// Current list nesting depth.
    list_depth: usize,
    /// Stack of list indices (None = unordered, Some(n) = ordered starting at n).
    list_indices: Vec<Option<u64>>,
    /// Whether we're in a code block.
    in_code_block: bool,
    /// Accumulated code block content.
    code_block_content: String,
    /// Language of current code block (for syntax highlighting).
    code_block_language: String,
    /// Current link URL (if in a link).
    current_link: Option<String>,
    /// Whether we're in a blockquote.
    in_blockquote: bool,
    /// Whether the next text should start on a new line.
    needs_newline: bool,
    /// Whether we're at the start (no blank lines needed yet).
    at_start: bool,
    /// Whether syntax highlighting is enabled.
    #[cfg(feature = "syntax-highlighting")]
    syntax_highlighting: bool,
    /// Syntax highlighting theme.
    #[cfg(feature = "syntax-highlighting")]
    syntax_theme: SyntaxTheme,
}

/// Inline text styling.
#[derive(Debug, Clone, Copy, Default)]
struct InlineStyle {
    bold: bool,
    italic: bool,
    strikethrough: bool,
    code: bool,
    /// Heading level (0 = not a heading).
    heading_level: u8,
}

impl MarkdownRenderer {
    #[cfg(feature = "syntax-highlighting")]
    fn new(code_theme: CodeTheme, syntax_highlighting: bool, syntax_theme: SyntaxTheme) -> Self {
        Self {
            code_theme,
            style_stack: vec![InlineStyle::default()],
            list_depth: 0,
            list_indices: Vec::new(),
            in_code_block: false,
            code_block_content: String::new(),
            code_block_language: String::new(),
            current_link: None,
            in_blockquote: false,
            needs_newline: false,
            at_start: true,
            syntax_highlighting,
            syntax_theme,
        }
    }

    #[cfg(not(feature = "syntax-highlighting"))]
    fn new(code_theme: CodeTheme) -> Self {
        Self {
            code_theme,
            style_stack: vec![InlineStyle::default()],
            list_depth: 0,
            list_indices: Vec::new(),
            in_code_block: false,
            code_block_content: String::new(),
            code_block_language: String::new(),
            current_link: None,
            in_blockquote: false,
            needs_newline: false,
            at_start: true,
        }
    }

    fn render(mut self, content: &str) -> Node {
        let mut root = BoxNode::new().flex_direction(FlexDirection::Column);
        let mut current_line: Vec<Node> = Vec::new();
        let mut lines: Vec<Node> = Vec::new();

        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(content, options);

        // Stream events directly instead of collecting to Vec
        for event in parser {
            match event {
                Event::Start(tag) => {
                    self.handle_start_tag(&tag, &mut current_line, &mut lines);
                }
                Event::End(tag) => {
                    self.handle_end_tag(&tag, &mut current_line, &mut lines);
                }
                Event::Text(text) => {
                    self.handle_text(&text, &mut current_line);
                }
                Event::Code(code) => {
                    self.handle_inline_code(&code, &mut current_line);
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.flush_line(&mut current_line, &mut lines);
                }
                Event::Rule => {
                    self.flush_line(&mut current_line, &mut lines);
                    lines.push(TextNode::new("───────────────────").dim().into());
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    // Render HTML as plain text
                    self.handle_text(&html, &mut current_line);
                }
                Event::FootnoteReference(_) | Event::TaskListMarker(_) => {
                    // Not supported, ignore
                }
            }
        }

        if self.in_code_block {
            self.finish_code_block(&mut lines);
        }

        // Flush any remaining content
        self.flush_line(&mut current_line, &mut lines);

        for line in lines {
            root = root.child(line);
        }

        root.into()
    }

    fn handle_start_tag(&mut self, tag: &Tag, current_line: &mut Vec<Node>, lines: &mut Vec<Node>) {
        match tag {
            Tag::Paragraph => {
                if !self.at_start && !self.in_blockquote {
                    self.needs_newline = true;
                }
            }
            Tag::Heading { level, .. } => {
                self.flush_line(current_line, lines);
                if !self.at_start {
                    lines.push(TextNode::new("").into()); // Blank line before heading
                }
                let heading_level = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                // Add heading marker
                let marker = "#".repeat(heading_level as usize);
                let marker_node = match heading_level {
                    1 => TextNode::new(format!("{} ", marker))
                        .bold()
                        .color(Color::Cyan),
                    2 => TextNode::new(format!("{} ", marker)).bold(),
                    _ => TextNode::new(format!("{} ", marker)).bold().dim(),
                };
                current_line.push(marker_node.into());

                self.push_style(InlineStyle {
                    heading_level,
                    ..Default::default()
                });
            }
            Tag::BlockQuote => {
                self.flush_line(current_line, lines);
                self.in_blockquote = true;
            }
            Tag::CodeBlock(kind) => {
                self.flush_line(current_line, lines);
                if !self.at_start {
                    lines.push(TextNode::new("").into()); // Blank line before code block
                }
                self.in_code_block = true;
                self.code_block_content.clear();
                self.code_block_language.clear();
                // Note the language if specified (for syntax highlighting)
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() {
                        self.code_block_language = lang.to_string();
                        let lang_node = TextNode::new(format!("┌─ {} ", lang))
                            .dim()
                            .color(Color::BrightBlack);
                        lines.push(lang_node.into());
                    }
                }
            }
            Tag::List(start) => {
                self.flush_line(current_line, lines);
                if self.list_depth == 0 && !self.at_start {
                    lines.push(TextNode::new("").into()); // Blank line before list
                }
                self.list_indices.push(*start);
                self.list_depth += 1;
            }
            Tag::Item => {
                self.flush_line(current_line, lines);
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                let marker = if let Some(Some(idx)) = self.list_indices.last_mut() {
                    let marker = format!("{}. ", idx);
                    *idx += 1;
                    marker
                } else {
                    "• ".to_string()
                };
                let marker_node = TextNode::new(format!("{}{}", indent, marker));
                current_line.push(marker_node.into());
            }
            Tag::Emphasis => {
                self.push_style(InlineStyle {
                    italic: true,
                    ..Default::default()
                });
            }
            Tag::Strong => {
                self.push_style(InlineStyle {
                    bold: true,
                    ..Default::default()
                });
            }
            Tag::Strikethrough => {
                self.push_style(InlineStyle {
                    strikethrough: true,
                    ..Default::default()
                });
            }
            Tag::Link { dest_url, .. } => {
                self.current_link = Some(dest_url.to_string());
            }
            Tag::Image { .. }
            | Tag::Table(_)
            | Tag::TableHead
            | Tag::TableRow
            | Tag::TableCell
            | Tag::FootnoteDefinition(_)
            | Tag::MetadataBlock(_)
            | Tag::HtmlBlock => {
                // Not fully supported, ignore
            }
        }
    }

    fn handle_end_tag(
        &mut self,
        tag: &TagEnd,
        current_line: &mut Vec<Node>,
        lines: &mut Vec<Node>,
    ) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line(current_line, lines);
                self.needs_newline = false;
                self.at_start = false;
            }
            TagEnd::Heading(_) => {
                self.pop_style();
                self.flush_line(current_line, lines);
                self.at_start = false;
            }
            TagEnd::BlockQuote => {
                self.flush_line(current_line, lines);
                self.in_blockquote = false;
                self.at_start = false;
            }
            TagEnd::CodeBlock => {
                self.finish_code_block(lines);
            }
            TagEnd::List(_) => {
                self.list_indices.pop();
                self.list_depth = self.list_depth.saturating_sub(1);
                if self.list_depth == 0 {
                    self.at_start = false;
                }
            }
            TagEnd::Item => {
                self.flush_line(current_line, lines);
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Link => {
                // Append the URL after the link text
                if let Some(url) = self.current_link.take() {
                    let url_node = TextNode::new(format!(" ({})", url))
                        .color(Color::Blue)
                        .underline();
                    current_line.push(url_node.into());
                }
            }
            TagEnd::Image
            | TagEnd::Table
            | TagEnd::TableHead
            | TagEnd::TableRow
            | TagEnd::TableCell
            | TagEnd::FootnoteDefinition
            | TagEnd::MetadataBlock(_)
            | TagEnd::HtmlBlock => {
                // Not fully supported, ignore
            }
        }
    }

    fn handle_text(&mut self, text: &str, current_line: &mut Vec<Node>) {
        if self.in_code_block {
            self.code_block_content.push_str(text);
            return;
        }

        let style = self.current_style();
        let mut node = TextNode::new(text);

        // Apply heading styles
        match style.heading_level {
            1 => {
                node = node.bold().color(Color::Cyan);
            }
            2 => {
                node = node.bold();
            }
            3..=6 => {
                node = node.bold().dim();
            }
            _ => {}
        }

        // Apply inline styles
        if style.bold {
            node = node.bold();
        }
        if style.italic {
            node = node.italic();
        }
        if style.strikethrough {
            node = node.strikethrough();
        }

        // Apply blockquote styling
        if self.in_blockquote {
            // Prepend with blockquote marker
            let quote_node = TextNode::new("│ ").color(Color::Green);
            current_line.push(quote_node.into());
            node = node.color(Color::Green);
        }

        current_line.push(node.into());
        self.at_start = false;
    }

    fn handle_inline_code(&mut self, code: &str, current_line: &mut Vec<Node>) {
        let code_color = match self.code_theme {
            CodeTheme::Dark => Color::Cyan,
            CodeTheme::Light => Color::BrightBlack,
        };
        let node = TextNode::new(format!("`{}`", code)).color(code_color);
        current_line.push(node.into());
        self.at_start = false;
    }

    fn finish_code_block(&mut self, lines: &mut Vec<Node>) {
        if !self.in_code_block {
            return;
        }

        let code_color = match self.code_theme {
            CodeTheme::Dark => Color::Cyan,
            CodeTheme::Light => Color::BrightBlack,
        };

        // Try syntax highlighting if enabled and language is specified
        #[cfg(feature = "syntax-highlighting")]
        if self.syntax_highlighting && !self.code_block_language.is_empty() {
            let highlighter = SyntaxHighlighter::new().theme(self.syntax_theme);
            let highlighted_nodes = highlighter.highlight_to_nodes(
                &self.code_block_content,
                &self.code_block_language,
                code_color,
            );
            for node in highlighted_nodes {
                lines.push(node);
            }
            self.in_code_block = false;
            self.code_block_content.clear();
            self.code_block_language.clear();
            self.at_start = false;
            return;
        }

        // Fallback: plain colored code
        for line in self.code_block_content.lines() {
            // Use NoWrap for code blocks to preserve formatting
            let code_node = TextNode::new(format!("│ {}", line))
                .color(code_color)
                .wrap(TextWrap::NoWrap);
            lines.push(code_node.into());
        }

        self.in_code_block = false;
        self.code_block_content.clear();
        self.code_block_language.clear();
        self.at_start = false;
    }

    fn flush_line(&mut self, current_line: &mut Vec<Node>, lines: &mut Vec<Node>) {
        if current_line.is_empty() {
            if self.needs_newline {
                lines.push(TextNode::new("").into());
                self.needs_newline = false;
            }
            return;
        }

        // Check if we need a blank line before this content
        if self.needs_newline && !lines.is_empty() {
            lines.push(TextNode::new("").into());
            self.needs_newline = false;
        }

        // Create a horizontal box for the line
        if current_line.len() == 1 {
            lines.push(current_line.remove(0));
        } else {
            let mut line_box = BoxNode::new().flex_direction(FlexDirection::Row);
            for node in current_line.drain(..) {
                line_box = line_box.child(node);
            }
            lines.push(line_box.into());
        }
    }

    fn push_style(&mut self, style: InlineStyle) {
        let current = self.current_style();
        self.style_stack.push(InlineStyle {
            bold: current.bold || style.bold,
            italic: current.italic || style.italic,
            strikethrough: current.strikethrough || style.strikethrough,
            code: current.code || style.code,
            heading_level: if style.heading_level > 0 {
                style.heading_level
            } else {
                current.heading_level
            },
        });
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn current_style(&self) -> InlineStyle {
        self.style_stack.last().copied().unwrap_or_default()
    }
}

/// Tier 1 renderer: ASCII-only output with structure but no colors.
///
/// This renderer produces readable markdown output without ANSI colors,
/// suitable for terminals with limited capabilities.
struct MarkdownRendererTier1 {
    /// Current list nesting depth.
    list_depth: usize,
    /// Stack of list indices (None = unordered, Some(n) = ordered starting at n).
    list_indices: Vec<Option<u64>>,
    /// Whether we're in a code block.
    in_code_block: bool,
    /// Accumulated code block content.
    code_block_content: String,
    /// Current link URL (if in a link).
    current_link: Option<String>,
    /// Whether we're in a blockquote.
    in_blockquote: bool,
    /// Whether the next text should start on a new line.
    needs_newline: bool,
    /// Whether we're at the start (no blank lines needed yet).
    at_start: bool,
    /// Current heading level (0 = not a heading).
    current_heading: u8,
}

impl MarkdownRendererTier1 {
    fn new() -> Self {
        Self {
            list_depth: 0,
            list_indices: Vec::new(),
            in_code_block: false,
            code_block_content: String::new(),
            current_link: None,
            in_blockquote: false,
            needs_newline: false,
            at_start: true,
            current_heading: 0,
        }
    }

    fn render(mut self, content: &str) -> Node {
        let mut root = BoxNode::new().flex_direction(FlexDirection::Column);
        let mut current_line: Vec<String> = Vec::new();
        let mut lines: Vec<String> = Vec::new();

        let mut options = Options::empty();
        options.insert(Options::ENABLE_STRIKETHROUGH);
        let parser = Parser::new_ext(content, options);

        for event in parser {
            match event {
                Event::Start(tag) => {
                    self.handle_start_tag_tier1(&tag, &mut current_line, &mut lines);
                }
                Event::End(tag) => {
                    self.handle_end_tag_tier1(&tag, &mut current_line, &mut lines);
                }
                Event::Text(text) => {
                    self.handle_text_tier1(&text, &mut current_line);
                }
                Event::Code(code) => {
                    // Inline code with backticks (no color)
                    current_line.push(format!("`{}`", code));
                    self.at_start = false;
                }
                Event::SoftBreak | Event::HardBreak => {
                    self.flush_line_tier1(&mut current_line, &mut lines);
                }
                Event::Rule => {
                    self.flush_line_tier1(&mut current_line, &mut lines);
                    lines.push("---".to_string());
                }
                Event::Html(html) | Event::InlineHtml(html) => {
                    current_line.push(html.to_string());
                }
                Event::FootnoteReference(_) | Event::TaskListMarker(_) => {
                    // Not supported
                }
            }
        }

        if self.in_code_block {
            self.finish_code_block_tier1(&mut lines);
        }

        // Flush remaining content
        self.flush_line_tier1(&mut current_line, &mut lines);

        for line in lines {
            root = root.child(TextNode::new(line));
        }

        root.into()
    }

    fn handle_start_tag_tier1(
        &mut self,
        tag: &Tag,
        current_line: &mut Vec<String>,
        lines: &mut Vec<String>,
    ) {
        match tag {
            Tag::Paragraph => {
                if !self.at_start && !self.in_blockquote {
                    self.needs_newline = true;
                }
            }
            Tag::Heading { level, .. } => {
                self.flush_line_tier1(current_line, lines);
                if !self.at_start {
                    lines.push(String::new());
                }
                self.current_heading = match level {
                    HeadingLevel::H1 => 1,
                    HeadingLevel::H2 => 2,
                    HeadingLevel::H3 => 3,
                    HeadingLevel::H4 => 4,
                    HeadingLevel::H5 => 5,
                    HeadingLevel::H6 => 6,
                };
                // Add heading marker
                let marker = "#".repeat(self.current_heading as usize);
                current_line.push(format!("{} ", marker));
            }
            Tag::BlockQuote => {
                self.flush_line_tier1(current_line, lines);
                self.in_blockquote = true;
            }
            Tag::CodeBlock(kind) => {
                self.flush_line_tier1(current_line, lines);
                if !self.at_start {
                    lines.push(String::new());
                }
                self.in_code_block = true;
                self.code_block_content.clear();
                // Language indicator
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() {
                        lines.push(format!("--- {} ---", lang));
                    } else {
                        lines.push("---".to_string());
                    }
                } else {
                    lines.push("---".to_string());
                }
            }
            Tag::List(start) => {
                self.flush_line_tier1(current_line, lines);
                if self.list_depth == 0 && !self.at_start {
                    lines.push(String::new());
                }
                self.list_indices.push(*start);
                self.list_depth += 1;
            }
            Tag::Item => {
                self.flush_line_tier1(current_line, lines);
                let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                let marker = if let Some(Some(idx)) = self.list_indices.last_mut() {
                    let m = format!("{}. ", idx);
                    *idx += 1;
                    m
                } else {
                    "- ".to_string()
                };
                current_line.push(format!("{}{}", indent, marker));
            }
            Tag::Emphasis => {
                current_line.push("*".to_string());
            }
            Tag::Strong => {
                current_line.push("**".to_string());
            }
            Tag::Strikethrough => {
                current_line.push("~~".to_string());
            }
            Tag::Link { dest_url, .. } => {
                self.current_link = Some(dest_url.to_string());
            }
            _ => {}
        }
    }

    fn handle_end_tag_tier1(
        &mut self,
        tag: &TagEnd,
        current_line: &mut Vec<String>,
        lines: &mut Vec<String>,
    ) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line_tier1(current_line, lines);
                self.needs_newline = false;
                self.at_start = false;
            }
            TagEnd::Heading(_) => {
                self.current_heading = 0;
                self.flush_line_tier1(current_line, lines);
                self.at_start = false;
            }
            TagEnd::BlockQuote => {
                self.flush_line_tier1(current_line, lines);
                self.in_blockquote = false;
                self.at_start = false;
            }
            TagEnd::CodeBlock => {
                self.finish_code_block_tier1(lines);
            }
            TagEnd::List(_) => {
                self.list_indices.pop();
                self.list_depth = self.list_depth.saturating_sub(1);
                if self.list_depth == 0 {
                    self.at_start = false;
                }
            }
            TagEnd::Item => {
                self.flush_line_tier1(current_line, lines);
            }
            TagEnd::Emphasis => {
                current_line.push("*".to_string());
            }
            TagEnd::Strong => {
                current_line.push("**".to_string());
            }
            TagEnd::Strikethrough => {
                current_line.push("~~".to_string());
            }
            TagEnd::Link => {
                if let Some(url) = self.current_link.take() {
                    current_line.push(format!(" ({})", url));
                }
            }
            _ => {}
        }
    }

    fn handle_text_tier1(&mut self, text: &str, current_line: &mut Vec<String>) {
        if self.in_code_block {
            self.code_block_content.push_str(text);
            return;
        }

        if self.in_blockquote {
            current_line.push(format!("> {}", text));
        } else {
            current_line.push(text.to_string());
        }
        self.at_start = false;
    }

    fn finish_code_block_tier1(&mut self, lines: &mut Vec<String>) {
        if !self.in_code_block {
            return;
        }

        // Emit code block content with indent
        for line in self.code_block_content.lines() {
            lines.push(format!("  {}", line));
        }
        lines.push("---".to_string());
        self.in_code_block = false;
        self.code_block_content.clear();
        self.at_start = false;
    }

    fn flush_line_tier1(&mut self, current_line: &mut Vec<String>, lines: &mut Vec<String>) {
        if current_line.is_empty() {
            if self.needs_newline {
                lines.push(String::new());
                self.needs_newline = false;
            }
            return;
        }

        if self.needs_newline && !lines.is_empty() {
            lines.push(String::new());
            self.needs_newline = false;
        }

        let line = current_line.join("");
        lines.push(line);
        current_line.clear();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    /// Helper to convert a node tree to a simple string representation.
    fn node_to_text(node: &Node) -> String {
        match node {
            Node::Text(t) => t.content.to_string(),
            Node::Box(b) => {
                let mut result = String::new();
                for c in &b.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Root(r) => {
                let mut result = String::new();
                for c in &r.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Static(s) => {
                let mut result = String::new();
                for c in &s.children {
                    result.push_str(&node_to_text(c));
                }
                result
            }
            Node::Custom(c) => {
                let mut result = String::new();
                for child in c.widget().children() {
                    result.push_str(&node_to_text(child));
                }
                result
            }
        }
    }

    #[test]
    fn test_basic_text() {
        let md = Markdown::new("Hello World");
        let node = md.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("Hello World"));
    }

    #[test]
    fn test_heading_levels() {
        let md = Markdown::new("# H1\n## H2\n### H3");
        let node = md.to_node();
        let text = node_to_text(&node);

        // Check that heading markers are present
        assert!(text.contains("# "));
        assert!(text.contains("## "));
        assert!(text.contains("### "));
        assert!(text.contains("H1"));
        assert!(text.contains("H2"));
        assert!(text.contains("H3"));
    }

    #[test]
    fn test_bold_text() {
        let md = Markdown::new("This is **bold** text");
        let node = md.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("bold"));

        // Verify the bold text node has bold styling
        fn find_bold_node(node: &Node) -> bool {
            match node {
                Node::Text(t) if t.content.contains("bold") => t.text_style.bold,
                Node::Box(b) => b.children.iter().any(|c| find_bold_node(c)),
                _ => false,
            }
        }
        assert!(find_bold_node(&node));
    }

    #[test]
    fn test_italic_text() {
        let md = Markdown::new("This is *italic* text");
        let node = md.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("italic"));

        fn find_italic_node(node: &Node) -> bool {
            match node {
                Node::Text(t) if t.content.contains("italic") => t.text_style.italic,
                Node::Box(b) => b.children.iter().any(|c| find_italic_node(c)),
                _ => false,
            }
        }
        assert!(find_italic_node(&node));
    }

    #[test]
    fn test_strikethrough() {
        let md = Markdown::new("This is ~~deleted~~ text");
        let node = md.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("deleted"));

        fn find_strikethrough_node(node: &Node) -> bool {
            match node {
                Node::Text(t) if t.content.contains("deleted") => t.text_style.strikethrough,
                Node::Box(b) => b.children.iter().any(|c| find_strikethrough_node(c)),
                _ => false,
            }
        }
        assert!(find_strikethrough_node(&node));
    }

    #[test]
    fn test_inline_code() {
        let md = Markdown::new("Use `println!()` to print");
        let node = md.to_node();
        let text = node_to_text(&node);
        assert!(text.contains("`println!()`"));
    }

    #[test]
    fn test_code_block() {
        let md = Markdown::new("```rust\nfn main() {\n    println!(\"Hello\");\n}\n```");
        let node = md.to_node();
        let text = node_to_text(&node);

        // Code block should contain the code
        assert!(text.contains("fn main()"));
        assert!(text.contains("println!"));
        // Should have language indicator
        assert!(text.contains("rust"));
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_code_block_with_syntax_highlighting() {
        use crate::components::SyntaxTheme;

        let md = Markdown::new("```rust\nfn main() {\n    println!(\"Hello\");\n}\n```")
            .syntax_highlighting(true)
            .syntax_theme(SyntaxTheme::Base16OceanDark);
        let node = md.to_node();
        let text = node_to_text(&node);

        // Code block should contain the code
        assert!(text.contains("fn main()"));
        assert!(text.contains("println!"));
        // Should have language indicator
        assert!(text.contains("rust"));

        // With syntax highlighting, the code should be rendered with colors
        // (we can't easily check colors in text output, but ensure it doesn't break)
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_code_block_syntax_highlighting_disabled() {
        let md = Markdown::new("```rust\nfn main() {}\n```").syntax_highlighting(false);
        let node = md.to_node();
        let text = node_to_text(&node);

        // Should still render the code
        assert!(text.contains("fn main()"));
    }

    #[test]
    fn test_code_block_nowrap() {
        use crate::style::TextWrap;

        let md = Markdown::new("```\nlong code line here\n```");
        let node = md.to_node();

        // Find code block text nodes and verify they use NoWrap
        fn find_code_wrap_mode(node: &Node) -> Option<TextWrap> {
            match node {
                Node::Text(t) if t.content.contains("long code") => Some(t.text_style.wrap),
                Node::Box(b) => b.children.iter().find_map(|c| find_code_wrap_mode(c)),
                _ => None,
            }
        }
        assert_eq!(find_code_wrap_mode(&node), Some(TextWrap::NoWrap));
    }

    #[test]
    fn test_unordered_list() {
        let md = Markdown::new("- Item 1\n- Item 2\n- Item 3");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("•"));
        assert!(text.contains("Item 1"));
        assert!(text.contains("Item 2"));
        assert!(text.contains("Item 3"));
    }

    #[test]
    fn test_ordered_list() {
        let md = Markdown::new("1. First\n2. Second\n3. Third");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("1."));
        assert!(text.contains("2."));
        assert!(text.contains("3."));
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
        assert!(text.contains("Third"));
    }

    #[test]
    fn test_links() {
        let md = Markdown::new("[Click here](https://example.com)");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Click here"));
        assert!(text.contains("https://example.com"));
    }

    #[test]
    fn test_blockquote() {
        let md = Markdown::new("> This is a quote");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("│"));
        assert!(text.contains("This is a quote"));
    }

    #[test]
    fn test_nested_formatting() {
        let md = Markdown::new("**bold with `code` inside**");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("bold with"));
        assert!(text.contains("`code`"));
        assert!(text.contains("inside"));
    }

    #[test]
    fn test_horizontal_rule() {
        let md = Markdown::new("Above\n\n---\n\nBelow");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Above"));
        assert!(text.contains("───"));
        assert!(text.contains("Below"));
    }

    #[test]
    fn test_code_theme_dark() {
        let md = Markdown::new("`code`").code_theme(CodeTheme::Dark);
        let node = md.to_node();

        fn find_code_color(node: &Node) -> Option<Color> {
            match node {
                Node::Text(t) if t.content.contains("code") => t.text_style.color,
                Node::Box(b) => b.children.iter().find_map(|c| find_code_color(c)),
                _ => None,
            }
        }
        assert_eq!(find_code_color(&node), Some(Color::Cyan));
    }

    #[test]
    fn test_code_theme_light() {
        let md = Markdown::new("`code`").code_theme(CodeTheme::Light);
        let node = md.to_node();

        fn find_code_color(node: &Node) -> Option<Color> {
            match node {
                Node::Text(t) if t.content.contains("code") => t.text_style.color,
                Node::Box(b) => b.children.iter().find_map(|c| find_code_color(c)),
                _ => None,
            }
        }
        assert_eq!(find_code_color(&node), Some(Color::BrightBlack));
    }

    #[test]
    fn test_empty_content() {
        let md = Markdown::new("");
        let node = md.to_node();
        // Should not panic, just return empty box
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_nested_lists() {
        let md = Markdown::new("- Outer\n  - Inner 1\n  - Inner 2\n- Outer 2");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("Outer"));
        assert!(text.contains("Inner 1"));
        assert!(text.contains("Inner 2"));
        assert!(text.contains("Outer 2"));
    }

    #[test]
    fn test_real_readme() {
        // Test with a realistic README content
        let readme = r#"# inky-tui

A Rust-native terminal UI library.

## Features

- **Flexbox layout** via Taffy
- **React-like components**
- `Signal<T>` for reactive state

### Installation

```bash
cargo add inky-tui
```

For more information, see [the docs](https://docs.rs/inky-tui).
"#;
        let md = Markdown::new(readme);
        let node = md.to_node();
        let text = node_to_text(&node);

        // Should render without panic and contain key content
        assert!(text.contains("inky-tui"));
        assert!(text.contains("Flexbox layout"));
        assert!(text.contains("cargo add"));
        assert!(text.contains("docs.rs"));
    }

    #[test]
    fn test_multiple_paragraphs() {
        let md = Markdown::new("First paragraph.\n\nSecond paragraph.\n\nThird paragraph.");
        let node = md.to_node();
        let text = node_to_text(&node);

        assert!(text.contains("First paragraph"));
        assert!(text.contains("Second paragraph"));
        assert!(text.contains("Third paragraph"));
    }

    #[test]
    fn test_from_impl() {
        // Test the From<Markdown> impl for Node
        let md = Markdown::new("test");
        let node: Node = md.into();
        assert!(matches!(node, Node::Box(_)));
    }

    #[test]
    fn test_cow_zero_copy() {
        use std::borrow::Cow;

        // Static strings use borrowed Cow (no allocation)
        let md = Markdown::new("# Hello");
        assert!(matches!(md.content, Cow::Borrowed(_)));

        // Owned strings use owned Cow
        let owned = String::from("# Dynamic");
        let md = Markdown::new(owned);
        assert!(matches!(md.content, Cow::Owned(_)));

        // Both work correctly
        let md1 = Markdown::new("**static**");
        let md2 = Markdown::new(String::from("**owned**"));
        let text1 = node_to_text(&md1.to_node());
        let text2 = node_to_text(&md2.to_node());
        assert!(text1.contains("static"));
        assert!(text2.contains("owned"));
    }

    // ==================== Adaptive Rendering Tests ====================

    #[test]
    fn test_adaptive_tier0_summary() {
        let content = "# Hello\n\nThis is **bold** and `code`.\n\n- item 1\n- item 2";
        let md = Markdown::new(content);
        let node = md.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        // Tier 0 should show summary stats, not rendered content
        assert!(text.contains("[Markdown]"));
        assert!(text.contains("words="));
        assert!(text.contains("lines="));
        assert!(text.contains("headings=yes"));
        assert!(text.contains("code=yes"));
        assert!(text.contains("lists=yes"));
    }

    #[test]
    fn test_adaptive_tier0_simple() {
        let md = Markdown::new("Just plain text.");
        let node = md.render_for_tier(RenderTier::Tier0Fallback);
        let text = node_to_text(&node);

        assert!(text.contains("[Markdown]"));
        assert!(text.contains("words=3"));
        // Should NOT have headings/code/lists markers
        assert!(!text.contains("headings="));
        assert!(!text.contains("code="));
        assert!(!text.contains("lists="));
    }

    #[test]
    fn test_adaptive_tier1_headings() {
        let md = Markdown::new("# H1\n## H2\n### H3");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Tier 1 should show structure but no colors
        assert!(text.contains("# H1"));
        assert!(text.contains("## H2"));
        assert!(text.contains("### H3"));
    }

    #[test]
    fn test_adaptive_tier1_lists() {
        let md = Markdown::new("- Item 1\n- Item 2\n- Item 3");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("- Item 1"));
        assert!(text.contains("- Item 2"));
        assert!(text.contains("- Item 3"));
    }

    #[test]
    fn test_adaptive_tier1_ordered_list() {
        let md = Markdown::new("1. First\n2. Second\n3. Third");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("1."));
        assert!(text.contains("2."));
        assert!(text.contains("3."));
        assert!(text.contains("First"));
        assert!(text.contains("Second"));
        assert!(text.contains("Third"));
    }

    #[test]
    fn test_adaptive_tier1_code_block() {
        let md = Markdown::new("```rust\nfn main() {}\n```");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Should have language marker and code content
        assert!(text.contains("rust"));
        assert!(text.contains("fn main()"));
    }

    #[test]
    fn test_adaptive_tier1_inline_code() {
        let md = Markdown::new("Use `println!()` here");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("`println!()`"));
    }

    #[test]
    fn test_adaptive_tier1_bold_italic() {
        let md = Markdown::new("This is **bold** and *italic*.");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        // Tier 1 should preserve markdown markers
        assert!(text.contains("**bold**"));
        assert!(text.contains("*italic*"));
    }

    #[test]
    fn test_adaptive_tier1_links() {
        let md = Markdown::new("[Click](https://example.com)");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("Click"));
        assert!(text.contains("https://example.com"));
    }

    #[test]
    fn test_adaptive_tier1_blockquote() {
        let md = Markdown::new("> This is quoted");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("> This is quoted"));
    }

    #[test]
    fn test_adaptive_tier2_full_rendering() {
        let md = Markdown::new("# Hello\n\n**bold** text");
        let tier2_node = md.render_for_tier(RenderTier::Tier2Retained);
        let normal_node = md.to_node();

        // Tier 2 should produce the same output as normal rendering
        let tier2_text = node_to_text(&tier2_node);
        let normal_text = node_to_text(&normal_node);
        assert_eq!(tier2_text, normal_text);
    }

    #[test]
    fn test_adaptive_tier3_same_as_tier2() {
        let md = Markdown::new("# Test content\n\n- List item");
        let tier2_node = md.render_for_tier(RenderTier::Tier2Retained);
        let tier3_node = md.render_for_tier(RenderTier::Tier3Gpu);

        // Tier 3 should produce the same output as Tier 2
        let tier2_text = node_to_text(&tier2_node);
        let tier3_text = node_to_text(&tier3_node);
        assert_eq!(tier2_text, tier3_text);
    }

    #[test]
    fn test_adaptive_tier_features() {
        let md = Markdown::new("test");
        let features = md.tier_features();

        assert_eq!(features.name, Some("Markdown"));
        assert!(features.tier0_description.is_some());
        assert!(features.tier1_description.is_some());
        assert!(features.tier2_description.is_some());
        assert!(features.tier3_description.is_some());
    }

    #[test]
    fn test_adaptive_minimum_tier() {
        let md = Markdown::new("test");
        // Markdown works at all tiers
        assert!(md.minimum_tier().is_none());
        assert!(md.supports_tier(RenderTier::Tier0Fallback));
        assert!(md.supports_tier(RenderTier::Tier1Ansi));
        assert!(md.supports_tier(RenderTier::Tier2Retained));
        assert!(md.supports_tier(RenderTier::Tier3Gpu));
    }

    #[test]
    fn test_adaptive_tier1_horizontal_rule() {
        let md = Markdown::new("Above\n\n---\n\nBelow");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("Above"));
        assert!(text.contains("---"));
        assert!(text.contains("Below"));
    }

    #[test]
    fn test_adaptive_tier1_strikethrough() {
        let md = Markdown::new("This is ~~deleted~~ text");
        let node = md.render_for_tier(RenderTier::Tier1Ansi);
        let text = node_to_text(&node);

        assert!(text.contains("~~deleted~~"));
    }
}
