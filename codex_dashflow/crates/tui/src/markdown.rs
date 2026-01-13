//! Markdown rendering for terminal display
//!
//! Converts markdown text to styled ratatui Text using pulldown-cmark.

use pulldown_cmark::{CodeBlockKind, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Style, Stylize},
    text::{Line, Span, Text},
};

/// Styles used for different markdown elements
struct MarkdownStyles {
    h1: Style,
    h2: Style,
    h3: Style,
    h4: Style,
    h5: Style,
    h6: Style,
    code: Style,
    code_block: Style,
    emphasis: Style,
    strong: Style,
    strikethrough: Style,
    link: Style,
    blockquote: Style,
    list_marker: Style,
    diff_add: Style,
    diff_remove: Style,
    diff_header: Style,
    diff_hunk: Style,
}

impl Default for MarkdownStyles {
    fn default() -> Self {
        Self {
            h1: Style::new().bold().underlined(),
            h2: Style::new().bold(),
            h3: Style::new().bold().italic(),
            h4: Style::new().italic(),
            h5: Style::new().italic(),
            h6: Style::new().italic(),
            code: Style::new().fg(Color::Cyan),
            code_block: Style::new().fg(Color::Cyan),
            emphasis: Style::new().italic(),
            strong: Style::new().bold(),
            strikethrough: Style::new().crossed_out(),
            link: Style::new().fg(Color::Cyan).underlined(),
            blockquote: Style::new().fg(Color::Green),
            list_marker: Style::new().fg(Color::Blue),
            diff_add: Style::new().fg(Color::Green),
            diff_remove: Style::new().fg(Color::Red),
            diff_header: Style::new().fg(Color::Yellow).bold(),
            diff_hunk: Style::new().fg(Color::Cyan),
        }
    }
}

/// Render markdown text to styled ratatui Text
pub fn render_markdown(input: &str) -> Text<'static> {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(input, options);
    let mut writer = Writer::new(parser);
    writer.run();
    writer.text
}

/// Known code block languages that get special handling
#[derive(Clone, Copy, PartialEq, Eq)]
enum CodeBlockLang {
    Diff,
    Other,
}

/// Writer that converts markdown events to styled text
struct Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    iter: I,
    text: Text<'static>,
    styles: MarkdownStyles,
    /// Stack of active inline styles
    inline_styles: Vec<Style>,
    /// Current line being built
    current_line: Vec<Span<'static>>,
    /// Stack of list indices (None = unordered, Some(n) = ordered starting at n)
    list_stack: Vec<Option<u64>>,
    /// Current indent level
    indent_level: usize,
    /// Whether we're in a code block
    in_code_block: bool,
    /// Language of current code block (for syntax highlighting)
    code_block_lang: CodeBlockLang,
    /// Whether we need a newline before the next content
    needs_newline: bool,
    /// Pending link destination
    link_dest: Option<String>,
    /// Whether we're in a blockquote
    in_blockquote: bool,
}

impl<'a, I> Writer<'a, I>
where
    I: Iterator<Item = Event<'a>>,
{
    fn new(iter: I) -> Self {
        Self {
            iter,
            text: Text::default(),
            styles: MarkdownStyles::default(),
            inline_styles: Vec::new(),
            current_line: Vec::new(),
            list_stack: Vec::new(),
            indent_level: 0,
            in_code_block: false,
            code_block_lang: CodeBlockLang::Other,
            needs_newline: false,
            link_dest: None,
            in_blockquote: false,
        }
    }

    fn run(&mut self) {
        while let Some(event) = self.iter.next() {
            self.handle_event(event);
        }
        self.flush_line();
    }

    fn handle_event(&mut self, event: Event<'a>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.text(text),
            Event::Code(code) => self.inline_code(code),
            Event::SoftBreak | Event::HardBreak => self.line_break(),
            Event::Rule => self.horizontal_rule(),
            Event::Html(html) | Event::InlineHtml(html) => self.html(html),
            Event::FootnoteReference(_)
            | Event::TaskListMarker(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_) => {}
        }
    }

    fn start_tag(&mut self, tag: Tag<'a>) {
        match tag {
            Tag::Paragraph => {
                if self.needs_newline {
                    self.push_empty_line();
                }
            }
            Tag::Heading { level, .. } => {
                if self.needs_newline {
                    self.push_empty_line();
                }
                let style = self.heading_style(level);
                let prefix = format!("{} ", "#".repeat(level as usize));
                self.current_line.push(Span::styled(prefix, style));
                self.inline_styles.push(style);
            }
            Tag::BlockQuote(_) => {
                if self.needs_newline && !self.in_blockquote {
                    self.push_empty_line();
                }
                self.in_blockquote = true;
                self.indent_level += 1;
            }
            Tag::CodeBlock(kind) => {
                if self.needs_newline {
                    self.push_empty_line();
                }
                self.in_code_block = true;
                // Detect language for syntax highlighting
                self.code_block_lang = if let CodeBlockKind::Fenced(ref lang) = kind {
                    if lang.as_ref() == "diff" {
                        CodeBlockLang::Diff
                    } else {
                        CodeBlockLang::Other
                    }
                } else {
                    CodeBlockLang::Other
                };
                // Add language indicator if present (skip for diff - content is self-explanatory)
                if let CodeBlockKind::Fenced(lang) = kind {
                    if !lang.is_empty() && self.code_block_lang != CodeBlockLang::Diff {
                        self.current_line
                            .push(Span::styled(format!("```{}", lang), self.styles.code_block));
                        self.flush_line();
                    }
                }
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() && self.needs_newline {
                    self.push_empty_line();
                }
                self.list_stack.push(start);
            }
            Tag::Item => {
                self.flush_line();
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                let marker = if let Some(Some(n)) = self.list_stack.last_mut() {
                    let m = format!("{}. ", n);
                    *n += 1;
                    m
                } else {
                    "• ".to_string()
                };
                self.current_line
                    .push(Span::styled(indent, Style::default()));
                self.current_line
                    .push(Span::styled(marker, self.styles.list_marker));
            }
            Tag::Emphasis => {
                self.push_inline_style(self.styles.emphasis);
            }
            Tag::Strong => {
                self.push_inline_style(self.styles.strong);
            }
            Tag::Strikethrough => {
                self.push_inline_style(self.styles.strikethrough);
            }
            Tag::Link { dest_url, .. } => {
                self.link_dest = Some(dest_url.to_string());
                self.push_inline_style(self.styles.link);
            }
            _ => {}
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line();
                self.needs_newline = true;
            }
            TagEnd::Heading(_) => {
                self.inline_styles.pop();
                self.flush_line();
                self.needs_newline = true;
            }
            TagEnd::BlockQuote(_) => {
                self.indent_level = self.indent_level.saturating_sub(1);
                if self.indent_level == 0 {
                    self.in_blockquote = false;
                }
                self.needs_newline = true;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                self.code_block_lang = CodeBlockLang::Other;
                self.flush_line();
                self.needs_newline = true;
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.needs_newline = true;
                }
            }
            TagEnd::Item => {
                self.flush_line();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.inline_styles.pop();
            }
            TagEnd::Link => {
                self.inline_styles.pop();
                if let Some(dest) = self.link_dest.take() {
                    self.current_line.push(Span::raw(" ("));
                    self.current_line.push(Span::styled(dest, self.styles.link));
                    self.current_line.push(Span::raw(")"));
                }
            }
            _ => {}
        }
    }

    fn text(&mut self, text: CowStr<'a>) {
        let base_style = self.current_style();

        for (i, line) in text.lines().enumerate() {
            if i > 0 {
                self.flush_line();
            }
            if self.in_blockquote && self.current_line.is_empty() {
                let prefix = "> ".repeat(self.indent_level);
                self.current_line
                    .push(Span::styled(prefix, self.styles.blockquote));
            }

            // Determine style based on context
            let style = if self.in_code_block && self.code_block_lang == CodeBlockLang::Diff {
                // Apply diff-specific syntax highlighting
                self.diff_line_style(line)
            } else if self.in_code_block {
                base_style.patch(self.styles.code_block)
            } else if self.in_blockquote {
                base_style.patch(self.styles.blockquote)
            } else {
                base_style
            };

            self.current_line
                .push(Span::styled(line.to_string(), style));
        }
        self.needs_newline = false;
    }

    /// Determine the style for a diff line based on its prefix
    fn diff_line_style(&self, line: &str) -> Style {
        if line.starts_with("+++") || line.starts_with("---") {
            // File headers
            self.styles.diff_header
        } else if line.starts_with("@@") {
            // Hunk headers
            self.styles.diff_hunk
        } else if line.starts_with('+') {
            // Added lines
            self.styles.diff_add
        } else if line.starts_with('-') {
            // Removed lines
            self.styles.diff_remove
        } else if line.starts_with("diff ") || line.starts_with("index ") {
            // Git diff metadata
            self.styles.diff_header
        } else {
            // Context lines
            self.styles.code_block
        }
    }

    fn inline_code(&mut self, code: CowStr<'a>) {
        self.current_line
            .push(Span::styled(code.into_string(), self.styles.code));
    }

    fn line_break(&mut self) {
        self.flush_line();
    }

    fn horizontal_rule(&mut self) {
        self.flush_line();
        if !self.text.lines.is_empty() {
            self.push_empty_line();
        }
        self.text.lines.push(Line::from("───────────────────"));
        self.needs_newline = true;
    }

    fn html(&mut self, html: CowStr<'a>) {
        let style = self.current_style();
        for (i, line) in html.lines().enumerate() {
            if i > 0 {
                self.flush_line();
            }
            self.current_line
                .push(Span::styled(line.to_string(), style));
        }
    }

    fn heading_style(&self, level: HeadingLevel) -> Style {
        match level {
            HeadingLevel::H1 => self.styles.h1,
            HeadingLevel::H2 => self.styles.h2,
            HeadingLevel::H3 => self.styles.h3,
            HeadingLevel::H4 => self.styles.h4,
            HeadingLevel::H5 => self.styles.h5,
            HeadingLevel::H6 => self.styles.h6,
        }
    }

    fn current_style(&self) -> Style {
        self.inline_styles.last().copied().unwrap_or_default()
    }

    fn push_inline_style(&mut self, style: Style) {
        let current = self.current_style();
        self.inline_styles.push(current.patch(style));
    }

    fn flush_line(&mut self) {
        if !self.current_line.is_empty() {
            let spans = std::mem::take(&mut self.current_line);
            self.text.lines.push(Line::from(spans));
        }
    }

    fn push_empty_line(&mut self) {
        self.flush_line();
        self.text.lines.push(Line::default());
        self.needs_newline = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn lines_to_strings(text: &Text<'_>) -> Vec<String> {
        text.lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect()
    }

    #[test]
    fn test_plain_text() {
        let text = render_markdown("Hello, world!");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["Hello, world!"]);
    }

    #[test]
    fn test_heading_h1() {
        let text = render_markdown("# Heading 1");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["# Heading 1"]);
    }

    #[test]
    fn test_heading_h2() {
        let text = render_markdown("## Heading 2");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["## Heading 2"]);
    }

    #[test]
    fn test_bold_text() {
        let text = render_markdown("This is **bold** text");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["This is bold text"]);
        // Check that bold span exists
        assert!(text.lines[0].spans.len() >= 3);
    }

    #[test]
    fn test_italic_text() {
        let text = render_markdown("This is *italic* text");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["This is italic text"]);
    }

    #[test]
    fn test_inline_code() {
        let text = render_markdown("Use `code` here");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["Use code here"]);
    }

    #[test]
    fn test_code_block() {
        let text = render_markdown("```rust\nfn main() {}\n```");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["```rust", "fn main() {}"]);
    }

    #[test]
    fn test_unordered_list() {
        let text = render_markdown("- Item 1\n- Item 2\n- Item 3");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["• Item 1", "• Item 2", "• Item 3"]);
    }

    #[test]
    fn test_ordered_list() {
        let text = render_markdown("1. First\n2. Second\n3. Third");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["1. First", "2. Second", "3. Third"]);
    }

    #[test]
    fn test_blockquote() {
        let text = render_markdown("> This is a quote");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["> This is a quote"]);
    }

    #[test]
    fn test_link() {
        let text = render_markdown("[Click here](https://example.com)");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["Click here (https://example.com)"]);
    }

    #[test]
    fn test_horizontal_rule() {
        let text = render_markdown("Above\n\n---\n\nBelow");
        let lines = lines_to_strings(&text);
        assert!(lines.contains(&"───────────────────".to_string()));
    }

    #[test]
    fn test_paragraphs() {
        let text = render_markdown("First paragraph.\n\nSecond paragraph.");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["First paragraph.", "", "Second paragraph."]);
    }

    #[test]
    fn test_nested_list() {
        let text = render_markdown("- Outer\n  - Inner");
        let lines = lines_to_strings(&text);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("Outer"));
        assert!(lines[1].contains("Inner"));
    }

    #[test]
    fn test_strikethrough() {
        let text = render_markdown("This is ~~deleted~~ text");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["This is deleted text"]);
    }

    #[test]
    fn test_mixed_formatting() {
        let text = render_markdown("**Bold** and *italic* and `code`");
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["Bold and italic and code"]);
    }

    #[test]
    fn test_empty_input() {
        let text = render_markdown("");
        assert!(text.lines.is_empty());
    }

    #[test]
    fn test_multiline_code_block() {
        let text = render_markdown("```\nline1\nline2\nline3\n```");
        let lines = lines_to_strings(&text);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines, vec!["line1", "line2", "line3"]);
    }

    #[test]
    fn test_diff_syntax_highlighting_added_line() {
        let diff = "```diff\n+added line\n```";
        let text = render_markdown(diff);
        // Should have the added line
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["+added line"]);
        // Check it has green color (diff_add style)
        let style = text.lines[0].spans[0].style;
        assert_eq!(style.fg, Some(Color::Green));
    }

    #[test]
    fn test_diff_syntax_highlighting_removed_line() {
        let diff = "```diff\n-removed line\n```";
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["-removed line"]);
        // Check it has red color (diff_remove style)
        let style = text.lines[0].spans[0].style;
        assert_eq!(style.fg, Some(Color::Red));
    }

    #[test]
    fn test_diff_syntax_highlighting_file_header() {
        let diff = "```diff\n--- a/file.rs\n+++ b/file.rs\n```";
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["--- a/file.rs", "+++ b/file.rs"]);
        // File headers should have yellow bold style
        let style = text.lines[0].spans[0].style;
        assert_eq!(style.fg, Some(Color::Yellow));
    }

    #[test]
    fn test_diff_syntax_highlighting_hunk_header() {
        let diff = "```diff\n@@ -1,3 +1,4 @@\n```";
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec!["@@ -1,3 +1,4 @@"]);
        // Hunk headers should have cyan style
        let style = text.lines[0].spans[0].style;
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_diff_syntax_highlighting_context_line() {
        let diff = "```diff\n context line\n```";
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        assert_eq!(lines, vec![" context line"]);
        // Context lines should have default code_block style (cyan)
        let style = text.lines[0].spans[0].style;
        assert_eq!(style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_diff_syntax_highlighting_full_diff() {
        let diff = r#"```diff
diff --git a/src/main.rs b/src/main.rs
index abc123..def456 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
-    println!("Hello");
+    println!("Hello, world!");
+    println!("Goodbye");
 }
```"#;
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        assert_eq!(lines.len(), 10);
        // Check specific line styles
        // Line 0: "diff --git..." - header (yellow)
        assert_eq!(text.lines[0].spans[0].style.fg, Some(Color::Yellow));
        // Line 1: "index..." - header (yellow)
        assert_eq!(text.lines[1].spans[0].style.fg, Some(Color::Yellow));
        // Line 2: "--- a/src/main.rs" - header (yellow)
        assert_eq!(text.lines[2].spans[0].style.fg, Some(Color::Yellow));
        // Line 3: "+++ b/src/main.rs" - header (yellow)
        assert_eq!(text.lines[3].spans[0].style.fg, Some(Color::Yellow));
        // Line 4: "@@ -1,3 +1,4 @@" - hunk (cyan)
        assert_eq!(text.lines[4].spans[0].style.fg, Some(Color::Cyan));
        // Line 5: " fn main() {" - context (cyan)
        assert_eq!(text.lines[5].spans[0].style.fg, Some(Color::Cyan));
        // Line 6: "-    println..." - removed (red)
        assert_eq!(text.lines[6].spans[0].style.fg, Some(Color::Red));
        // Line 7: "+    println..." - added (green)
        assert_eq!(text.lines[7].spans[0].style.fg, Some(Color::Green));
        // Line 8: "+    println..." - added (green)
        assert_eq!(text.lines[8].spans[0].style.fg, Some(Color::Green));
        // Line 9: " }" - context (cyan)
        assert_eq!(text.lines[9].spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_diff_no_language_indicator_shown() {
        // For diff blocks, we skip the "```diff" indicator
        let diff = "```diff\n+added\n```";
        let text = render_markdown(diff);
        let lines = lines_to_strings(&text);
        // Should NOT contain "```diff" in output
        assert!(!lines.iter().any(|l| l.contains("```diff")));
        assert_eq!(lines, vec!["+added"]);
    }

    #[test]
    fn test_non_diff_code_block_shows_language() {
        // Non-diff code blocks should still show language indicator
        let code = "```rust\nfn main() {}\n```";
        let text = render_markdown(code);
        let lines = lines_to_strings(&text);
        assert_eq!(lines[0], "```rust");
    }
}
