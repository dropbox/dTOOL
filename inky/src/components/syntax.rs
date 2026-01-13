//! Syntax highlighting support for code blocks.
//!
//! This module provides syntax highlighting using syntect when the
//! `syntax-highlighting` feature is enabled.
//!
//! # Example
//!
//! ```ignore
//! use inky::components::syntax::{SyntaxHighlighter, SyntaxTheme};
//!
//! let highlighter = SyntaxHighlighter::new();
//! let highlighted = highlighter.highlight("fn main() {}", "rust");
//! ```

use crate::node::{BoxNode, Node, TextNode};
use crate::style::{Color, FlexDirection, TextWrap};

/// Available syntax highlighting themes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SyntaxTheme {
    /// Base16 Ocean Dark - good for dark terminals (default)
    #[default]
    Base16OceanDark,
    /// Base16 Ocean Light - good for light terminals
    Base16OceanLight,
    /// Base16 Eighties Dark
    Base16EightiesDark,
    /// Base16 Mocha Dark
    Base16MochaDark,
    /// InspiredGitHub - light theme
    InspiredGitHub,
    /// Solarized Dark
    SolarizedDark,
    /// Solarized Light
    SolarizedLight,
}

impl SyntaxTheme {
    /// Get the syntect theme name.
    #[cfg(feature = "syntax-highlighting")]
    fn theme_name(&self) -> &'static str {
        match self {
            Self::Base16OceanDark => "base16-ocean.dark",
            Self::Base16OceanLight => "base16-ocean.light",
            Self::Base16EightiesDark => "base16-eighties.dark",
            Self::Base16MochaDark => "base16-mocha.dark",
            Self::InspiredGitHub => "InspiredGitHub",
            Self::SolarizedDark => "Solarized (dark)",
            Self::SolarizedLight => "Solarized (light)",
        }
    }
}

/// A highlighted line of code with styled spans.
#[derive(Debug, Clone)]
pub struct HighlightedLine {
    /// The spans that make up this line.
    pub spans: Vec<HighlightedSpan>,
}

/// A span of highlighted text with color information.
#[derive(Debug, Clone)]
pub struct HighlightedSpan {
    /// The text content.
    pub text: String,
    /// Foreground color (if any).
    pub fg: Option<Color>,
    /// Whether the text is bold.
    pub bold: bool,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text is underlined.
    pub underline: bool,
}

/// Syntax highlighter using syntect.
///
/// This is a lazy-loaded singleton that caches syntax definitions and themes.
#[derive(Debug, Clone)]
pub struct SyntaxHighlighter {
    theme: SyntaxTheme,
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

impl SyntaxHighlighter {
    /// Create a new syntax highlighter with the default theme.
    pub fn new() -> Self {
        Self {
            theme: SyntaxTheme::default(),
        }
    }

    /// Set the theme for highlighting.
    pub fn theme(mut self, theme: SyntaxTheme) -> Self {
        self.theme = theme;
        self
    }

    /// Highlight code and return styled lines.
    ///
    /// Returns `None` if highlighting is not available (feature disabled or
    /// language not recognized).
    pub fn highlight(&self, code: &str, language: &str) -> Option<Vec<HighlightedLine>> {
        #[cfg(feature = "syntax-highlighting")]
        {
            self.highlight_with_syntect(code, language)
        }

        #[cfg(not(feature = "syntax-highlighting"))]
        {
            let _ = (code, language);
            None
        }
    }

    /// Highlight code and convert directly to inky Nodes.
    ///
    /// If highlighting fails, returns plain monospace nodes.
    pub fn highlight_to_nodes(
        &self,
        code: &str,
        language: &str,
        fallback_color: Color,
    ) -> Vec<Node> {
        if let Some(highlighted_lines) = self.highlight(code, language) {
            highlighted_lines
                .into_iter()
                .map(|line| self.line_to_node(line))
                .collect()
        } else {
            // Fallback: plain monospace with fallback color
            code.lines()
                .map(|line| {
                    TextNode::new(format!("│ {}", line))
                        .color(fallback_color)
                        .wrap(TextWrap::NoWrap)
                        .into()
                })
                .collect()
        }
    }

    /// Convert a highlighted line to an inky Node.
    fn line_to_node(&self, line: HighlightedLine) -> Node {
        if line.spans.is_empty() {
            return TextNode::new("│ ").wrap(TextWrap::NoWrap).into();
        }

        if line.spans.len() == 1 {
            let span = &line.spans[0];
            let mut node = TextNode::new(format!("│ {}", span.text)).wrap(TextWrap::NoWrap);
            if let Some(fg) = span.fg {
                node = node.color(fg);
            }
            if span.bold {
                node = node.bold();
            }
            if span.italic {
                node = node.italic();
            }
            if span.underline {
                node = node.underline();
            }
            return node.into();
        }

        // Multiple spans: create a row box
        let mut row = BoxNode::new().flex_direction(FlexDirection::Row);

        // Add the line prefix
        row = row.child(TextNode::new("│ ").wrap(TextWrap::NoWrap));

        for span in line.spans {
            let mut node = TextNode::new(&span.text).wrap(TextWrap::NoWrap);
            if let Some(fg) = span.fg {
                node = node.color(fg);
            }
            if span.bold {
                node = node.bold();
            }
            if span.italic {
                node = node.italic();
            }
            if span.underline {
                node = node.underline();
            }
            row = row.child(node);
        }

        row.into()
    }

    /// Get the list of supported language extensions.
    pub fn supported_languages() -> &'static [&'static str] {
        &[
            "rust",
            "rs",
            "python",
            "py",
            "javascript",
            "js",
            "typescript",
            "ts",
            "go",
            "java",
            "c",
            "cpp",
            "cc",
            "cxx",
            "h",
            "hpp",
            "bash",
            "sh",
            "zsh",
            "json",
            "yaml",
            "yml",
            "toml",
            "html",
            "htm",
            "css",
            "scss",
            "sass",
            "sql",
            "ruby",
            "rb",
            "php",
            "swift",
            "kotlin",
            "kt",
            "scala",
            "haskell",
            "hs",
            "lua",
            "perl",
            "pl",
            "r",
            "julia",
            "jl",
            "elixir",
            "ex",
            "exs",
            "erlang",
            "erl",
            "clojure",
            "clj",
            "lisp",
            "scheme",
            "ocaml",
            "ml",
            "fsharp",
            "fs",
            "csharp",
            "cs",
            "vb",
            "powershell",
            "ps1",
            "dockerfile",
            "makefile",
            "cmake",
            "nginx",
            "xml",
            "markdown",
            "md",
            "tex",
            "latex",
            "diff",
            "patch",
            "ini",
            "cfg",
            "conf",
            "vim",
            "awk",
            "sed",
            "graphql",
            "protobuf",
            "proto",
        ]
    }

    #[cfg(feature = "syntax-highlighting")]
    fn highlight_with_syntect(&self, code: &str, language: &str) -> Option<Vec<HighlightedLine>> {
        use syntect::highlighting::{FontStyle, ThemeSet};
        use syntect::parsing::SyntaxSet;

        // Use OnceLock for global syntax/theme sets (available since Rust 1.70)
        use std::sync::OnceLock;

        static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
        static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

        let syntax_set = SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines);
        let theme_set = THEME_SET.get_or_init(ThemeSet::load_defaults);

        let syntax = syntax_set
            .find_syntax_by_extension(language)
            .or_else(|| syntax_set.find_syntax_by_name(language))
            .or_else(|| {
                // Try common aliases and full language names
                let alias = match language.to_lowercase().as_str() {
                    "rust" | "rs" => "Rust",
                    "python" | "py" => "Python",
                    "javascript" | "js" => "JavaScript",
                    "typescript" | "ts" => "TypeScript",
                    "ruby" | "rb" => "Ruby",
                    "shell" | "sh" | "bash" | "zsh" => "Bourne Again Shell (bash)",
                    "yaml" | "yml" => "YAML",
                    "markdown" | "md" => "Markdown",
                    "dockerfile" => "Dockerfile",
                    "c" => "C",
                    "cpp" | "c++" | "cxx" => "C++",
                    "go" | "golang" => "Go",
                    "java" => "Java",
                    "json" => "JSON",
                    "toml" => "TOML",
                    "html" | "htm" => "HTML",
                    "css" => "CSS",
                    "sql" => "SQL",
                    _ => return None,
                };
                syntax_set.find_syntax_by_name(alias)
            })?;

        let theme = theme_set.themes.get(self.theme.theme_name())?;

        let highlighter = syntect::highlighting::Highlighter::new(theme);
        let mut highlight_state = syntect::highlighting::HighlightState::new(
            &highlighter,
            syntect::parsing::ScopeStack::new(),
        );
        let mut parse_state = syntect::parsing::ParseState::new(syntax);

        let mut result = Vec::new();

        for line in syntect::util::LinesWithEndings::from(code) {
            let ops = parse_state.parse_line(line, syntax_set).ok()?;
            let styled = syntect::highlighting::HighlightIterator::new(
                &mut highlight_state,
                &ops,
                line,
                &highlighter,
            );

            let mut spans = Vec::new();
            for (style, text) in styled {
                // Strip trailing newline from text
                let text = text.trim_end_matches('\n').to_string();
                if text.is_empty() {
                    continue;
                }

                let fg = Some(Color::Rgb(
                    style.foreground.r,
                    style.foreground.g,
                    style.foreground.b,
                ));

                spans.push(HighlightedSpan {
                    text,
                    fg,
                    bold: style.font_style.contains(FontStyle::BOLD),
                    italic: style.font_style.contains(FontStyle::ITALIC),
                    underline: style.font_style.contains(FontStyle::UNDERLINE),
                });
            }

            result.push(HighlightedLine { spans });
        }

        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_syntax_theme_variants() {
        // Just ensure all variants can be created
        let themes = [
            SyntaxTheme::Base16OceanDark,
            SyntaxTheme::Base16OceanLight,
            SyntaxTheme::Base16EightiesDark,
            SyntaxTheme::Base16MochaDark,
            SyntaxTheme::InspiredGitHub,
            SyntaxTheme::SolarizedDark,
            SyntaxTheme::SolarizedLight,
        ];
        for theme in themes {
            let _ = SyntaxHighlighter::new().theme(theme);
        }
    }

    #[test]
    fn test_supported_languages() {
        let langs = SyntaxHighlighter::supported_languages();
        assert!(langs.contains(&"rust"));
        assert!(langs.contains(&"python"));
        assert!(langs.contains(&"javascript"));
    }

    #[test]
    fn test_highlight_to_nodes_fallback() {
        let highlighter = SyntaxHighlighter::new();
        let nodes =
            highlighter.highlight_to_nodes("let x = 1;", "nonexistent_lang_xyz", Color::Cyan);
        // Should return fallback nodes
        assert!(!nodes.is_empty());
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_highlight_rust() {
        let highlighter = SyntaxHighlighter::new();
        // Use "rs" extension which syntect recognizes
        let result = highlighter.highlight("fn main() { println!(\"Hello\"); }", "rs");
        assert!(
            result.is_some(),
            "Should highlight Rust code with 'rs' extension"
        );
        let lines = result.expect("highlight returned None");
        assert!(!lines.is_empty());
        // Should have some colored spans
        assert!(!lines[0].spans.is_empty());
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_highlight_python() {
        let highlighter = SyntaxHighlighter::new();
        // Use "py" extension which syntect recognizes
        let result = highlighter.highlight("def hello():\n    print('world')", "py");
        assert!(
            result.is_some(),
            "Should highlight Python code with 'py' extension"
        );
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_highlight_javascript() {
        let highlighter = SyntaxHighlighter::new();
        let result = highlighter.highlight("const x = () => console.log('hi');", "js");
        assert!(result.is_some());
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_highlight_to_nodes_with_syntect() {
        let highlighter = SyntaxHighlighter::new();
        let nodes = highlighter.highlight_to_nodes("fn main() {}", "rs", Color::Cyan);
        assert!(!nodes.is_empty());
    }

    #[cfg(feature = "syntax-highlighting")]
    #[test]
    fn test_theme_selection() {
        let highlighter = SyntaxHighlighter::new().theme(SyntaxTheme::SolarizedDark);
        let result = highlighter.highlight("let x = 1;", "rs");
        assert!(
            result.is_some(),
            "Should highlight with Solarized Dark theme"
        );
    }
}
