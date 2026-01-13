// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

//! Programming language definitions for code-aware text splitting.
//!
//! This module defines the [`Language`] enum which provides language-specific
//! separators for intelligent code splitting. Each language has carefully chosen
//! separators that split along natural code boundaries like function definitions,
//! class definitions, and control flow statements.

/// Programming languages supported for code splitting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    /// C programming language
    C,
    /// C++ programming language
    Cpp,
    /// C# programming language
    CSharp,
    /// COBOL programming language
    Cobol,
    /// Elixir programming language
    Elixir,
    /// Go programming language
    Go,
    /// Haskell programming language
    Haskell,
    /// HTML markup language
    Html,
    /// Java programming language
    Java,
    /// JavaScript programming language
    Js,
    /// Kotlin programming language
    Kotlin,
    /// LaTeX markup language
    Latex,
    /// Lua programming language
    Lua,
    /// Markdown markup language
    Markdown,
    /// Perl programming language
    Perl,
    /// PHP programming language
    Php,
    /// `PowerShell` scripting language
    PowerShell,
    /// Protocol Buffers
    Proto,
    /// Python programming language
    Python,
    /// reStructuredText markup language
    Rst,
    /// Ruby programming language
    Ruby,
    /// Rust programming language
    Rust,
    /// Scala programming language
    Scala,
    /// Solidity programming language
    Sol,
    /// Swift programming language
    Swift,
    /// TypeScript programming language
    Ts,
    /// Visual Basic 6 programming language
    VisualBasic6,
}

impl Language {
    /// Get the language-specific separators for code splitting
    ///
    /// Returns a list of separators that are tried in order when splitting code.
    /// The separators are chosen to split along natural code boundaries like
    /// function definitions, class definitions, and control flow statements.
    #[must_use]
    pub fn get_separators(&self) -> Vec<String> {
        match self {
            Language::C | Language::Cpp => vec![
                "\nclass ".into(),
                "\nvoid ".into(),
                "\nint ".into(),
                "\nfloat ".into(),
                "\ndouble ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Go => vec![
                "\nfunc ".into(),
                "\nvar ".into(),
                "\nconst ".into(),
                "\ntype ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Java => vec![
                "\nclass ".into(),
                "\npublic ".into(),
                "\nprotected ".into(),
                "\nprivate ".into(),
                "\nstatic ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Kotlin => vec![
                "\nclass ".into(),
                "\npublic ".into(),
                "\nprotected ".into(),
                "\nprivate ".into(),
                "\ninternal ".into(),
                "\ncompanion ".into(),
                "\nfun ".into(),
                "\nval ".into(),
                "\nvar ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nwhen ".into(),
                "\ncase ".into(),
                "\nelse ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Js => vec![
                "\nfunction ".into(),
                "\nconst ".into(),
                "\nlet ".into(),
                "\nvar ".into(),
                "\nclass ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\ndefault ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Ts => vec![
                "\nenum ".into(),
                "\ninterface ".into(),
                "\nnamespace ".into(),
                "\ntype ".into(),
                "\nclass ".into(),
                "\nfunction ".into(),
                "\nconst ".into(),
                "\nlet ".into(),
                "\nvar ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\ndefault ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Php => vec![
                "\nfunction ".into(),
                "\nclass ".into(),
                "\nif ".into(),
                "\nforeach ".into(),
                "\nwhile ".into(),
                "\ndo ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Proto => vec![
                "\nmessage ".into(),
                "\nservice ".into(),
                "\nenum ".into(),
                "\noption ".into(),
                "\nimport ".into(),
                "\nsyntax ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Python => vec![
                "\nclass ".into(),
                "\ndef ".into(),
                "\n\tdef ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Rst => vec![
                "\n=+\n".into(),
                "\n-+\n".into(),
                "\n\\*+\n".into(),
                "\n\n.. *\n\n".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Ruby => vec![
                "\ndef ".into(),
                "\nclass ".into(),
                "\nif ".into(),
                "\nunless ".into(),
                "\nwhile ".into(),
                "\nfor ".into(),
                "\ndo ".into(),
                "\nbegin ".into(),
                "\nrescue ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Rust => vec![
                "\nfn ".into(),
                "\nconst ".into(),
                "\nlet ".into(),
                "\nif ".into(),
                "\nwhile ".into(),
                "\nfor ".into(),
                "\nloop ".into(),
                "\nmatch ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Scala => vec![
                "\nclass ".into(),
                "\nobject ".into(),
                "\ndef ".into(),
                "\nval ".into(),
                "\nvar ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nmatch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Swift => vec![
                "\nfunc ".into(),
                "\nclass ".into(),
                "\nstruct ".into(),
                "\nenum ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\ncase ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Markdown => vec![
                "\n#{1,6} ".into(),
                "```\n".into(),
                "\n\\*\\*\\*+\n".into(),
                "\n---+\n".into(),
                "\n___+\n".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Latex => vec![
                "\n\\\\chapter{".into(),
                "\n\\\\section{".into(),
                "\n\\\\subsection{".into(),
                "\n\\\\subsubsection{".into(),
                "\n\\\\begin{enumerate}".into(),
                "\n\\\\begin{itemize}".into(),
                "\n\\\\begin{description}".into(),
                "\n\\\\begin{list}".into(),
                "\n\\\\begin{quote}".into(),
                "\n\\\\begin{quotation}".into(),
                "\n\\\\begin{verse}".into(),
                "\n\\\\begin{verbatim}".into(),
                "\n\\\\begin{align}".into(),
                "$$".into(),
                "$".into(),
                " ".into(),
                String::new(),
            ],
            Language::Html => vec![
                "<body".into(),
                "<div".into(),
                "<p".into(),
                "<br".into(),
                "<li".into(),
                "<h1".into(),
                "<h2".into(),
                "<h3".into(),
                "<h4".into(),
                "<h5".into(),
                "<h6".into(),
                "<span".into(),
                "<table".into(),
                "<tr".into(),
                "<td".into(),
                "<th".into(),
                "<ul".into(),
                "<ol".into(),
                "<header".into(),
                "<footer".into(),
                "<nav".into(),
                "<head".into(),
                "<style".into(),
                "<script".into(),
                "<meta".into(),
                "<title".into(),
                String::new(),
            ],
            Language::Sol => vec![
                "\npragma ".into(),
                "\nusing ".into(),
                "\ncontract ".into(),
                "\ninterface ".into(),
                "\nlibrary ".into(),
                "\nconstructor ".into(),
                "\ntype ".into(),
                "\nfunction ".into(),
                "\nevent ".into(),
                "\nmodifier ".into(),
                "\nerror ".into(),
                "\nstruct ".into(),
                "\nenum ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\ndo while ".into(),
                "\nassembly ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::CSharp => vec![
                "\ninterface ".into(),
                "\nenum ".into(),
                "\nimplements ".into(),
                "\ndelegate ".into(),
                "\nevent ".into(),
                "\nclass ".into(),
                "\nabstract ".into(),
                "\npublic ".into(),
                "\nprotected ".into(),
                "\nprivate ".into(),
                "\nstatic ".into(),
                "\nreturn ".into(),
                "\nif ".into(),
                "\ncontinue ".into(),
                "\nfor ".into(),
                "\nforeach ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\nbreak ".into(),
                "\ncase ".into(),
                "\nelse ".into(),
                "\ntry ".into(),
                "\nthrow ".into(),
                "\nfinally ".into(),
                "\ncatch ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Cobol => vec![
                "\nIDENTIFICATION DIVISION.".into(),
                "\nENVIRONMENT DIVISION.".into(),
                "\nDATA DIVISION.".into(),
                "\nPROCEDURE DIVISION.".into(),
                "\nWORKING-STORAGE SECTION.".into(),
                "\nLINKAGE SECTION.".into(),
                "\nFILE SECTION.".into(),
                "\nINPUT-OUTPUT SECTION.".into(),
                "\nOPEN ".into(),
                "\nCLOSE ".into(),
                "\nREAD ".into(),
                "\nWRITE ".into(),
                "\nIF ".into(),
                "\nELSE ".into(),
                "\nMOVE ".into(),
                "\nPERFORM ".into(),
                "\nUNTIL ".into(),
                "\nVARYING ".into(),
                "\nACCEPT ".into(),
                "\nDISPLAY ".into(),
                "\nSTOP RUN.".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Lua => vec![
                "\nfunction ".into(),
                "\nlocal function ".into(),
                "\nif ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nrepeat ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Perl => vec![
                "\nsub ".into(),
                "\nif ".into(),
                "\nforeach ".into(),
                "\nwhile ".into(),
                "\ndo ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Haskell => vec![
                "\n-- | ".into(),
                "\nmodule ".into(),
                "\nimport ".into(),
                "\ndata ".into(),
                "\nnewtype ".into(),
                "\ntype ".into(),
                "\nclass ".into(),
                "\ninstance ".into(),
                "\nwhere\n".into(),
                "\nlet ".into(),
                "\nin ".into(),
                "\ncase ".into(),
                "\nof ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::Elixir => vec![
                "\ndef ".into(),
                "\ndefp ".into(),
                "\ndefmodule ".into(),
                "\ndefprotocol ".into(),
                "\ndefimpl ".into(),
                "\nif ".into(),
                "\nunless ".into(),
                "\ncase ".into(),
                "\ncond ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::PowerShell => vec![
                "\nfunction ".into(),
                "\nfilter ".into(),
                "\nif ".into(),
                "\nforeach ".into(),
                "\nfor ".into(),
                "\nwhile ".into(),
                "\nswitch ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
            Language::VisualBasic6 => vec![
                "\nSub ".into(),
                "\nFunction ".into(),
                "\nProperty Get ".into(),
                "\nProperty Set ".into(),
                "\nProperty Let ".into(),
                "\nIf ".into(),
                "\nFor ".into(),
                "\nDo ".into(),
                "\nWhile ".into(),
                "\nSelect Case ".into(),
                "\n\n".into(),
                "\n".into(),
                " ".into(),
                String::new(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Language enum tests
    // ========================================================================

    #[test]
    fn test_language_eq() {
        assert_eq!(Language::Rust, Language::Rust);
        assert_eq!(Language::Python, Language::Python);
    }

    #[test]
    fn test_language_ne() {
        assert_ne!(Language::Rust, Language::Python);
        assert_ne!(Language::Go, Language::Java);
    }

    #[test]
    fn test_language_clone() {
        let lang = Language::Ts;
        let cloned = lang;
        assert_eq!(lang, cloned);
    }

    #[test]
    fn test_language_copy() {
        let lang = Language::Js;
        let copied: Language = lang;
        assert_eq!(lang, copied);
    }

    #[test]
    fn test_language_debug() {
        let debug = format!("{:?}", Language::Python);
        assert_eq!(debug, "Python");
    }

    // ========================================================================
    // get_separators tests - all languages return non-empty
    // ========================================================================

    #[test]
    fn test_all_languages_have_separators() {
        let languages = [
            Language::C,
            Language::Cpp,
            Language::CSharp,
            Language::Cobol,
            Language::Elixir,
            Language::Go,
            Language::Haskell,
            Language::Html,
            Language::Java,
            Language::Js,
            Language::Kotlin,
            Language::Latex,
            Language::Lua,
            Language::Markdown,
            Language::Perl,
            Language::Php,
            Language::PowerShell,
            Language::Proto,
            Language::Python,
            Language::Rst,
            Language::Ruby,
            Language::Rust,
            Language::Scala,
            Language::Sol,
            Language::Swift,
            Language::Ts,
            Language::VisualBasic6,
        ];

        for lang in languages {
            let separators = lang.get_separators();
            assert!(
                !separators.is_empty(),
                "{:?} should have separators",
                lang
            );
        }
    }

    #[test]
    fn test_all_languages_end_with_empty_string() {
        // All languages should have empty string as last separator (fallback)
        let languages = [
            Language::C,
            Language::Cpp,
            Language::CSharp,
            Language::Cobol,
            Language::Elixir,
            Language::Go,
            Language::Haskell,
            Language::Html,
            Language::Java,
            Language::Js,
            Language::Kotlin,
            Language::Latex,
            Language::Lua,
            Language::Markdown,
            Language::Perl,
            Language::Php,
            Language::PowerShell,
            Language::Proto,
            Language::Python,
            Language::Rst,
            Language::Ruby,
            Language::Rust,
            Language::Scala,
            Language::Sol,
            Language::Swift,
            Language::Ts,
            Language::VisualBasic6,
        ];

        for lang in languages {
            let separators = lang.get_separators();
            assert_eq!(
                separators.last(),
                Some(&String::new()),
                "{:?} should end with empty string fallback",
                lang
            );
        }
    }

    // ========================================================================
    // Language-specific separator tests
    // ========================================================================

    #[test]
    fn test_rust_separators_contain_fn() {
        let seps = Language::Rust.get_separators();
        assert!(seps.contains(&"\nfn ".to_string()));
    }

    #[test]
    fn test_rust_separators_contain_match() {
        let seps = Language::Rust.get_separators();
        assert!(seps.contains(&"\nmatch ".to_string()));
    }

    #[test]
    fn test_python_separators_contain_class() {
        let seps = Language::Python.get_separators();
        assert!(seps.contains(&"\nclass ".to_string()));
    }

    #[test]
    fn test_python_separators_contain_def() {
        let seps = Language::Python.get_separators();
        assert!(seps.contains(&"\ndef ".to_string()));
    }

    #[test]
    fn test_go_separators_contain_func() {
        let seps = Language::Go.get_separators();
        assert!(seps.contains(&"\nfunc ".to_string()));
    }

    #[test]
    fn test_javascript_separators_contain_function() {
        let seps = Language::Js.get_separators();
        assert!(seps.contains(&"\nfunction ".to_string()));
    }

    #[test]
    fn test_typescript_separators_contain_interface() {
        let seps = Language::Ts.get_separators();
        assert!(seps.contains(&"\ninterface ".to_string()));
    }

    #[test]
    fn test_java_separators_contain_class() {
        let seps = Language::Java.get_separators();
        assert!(seps.contains(&"\nclass ".to_string()));
    }

    #[test]
    fn test_markdown_separators_contain_code_block() {
        let seps = Language::Markdown.get_separators();
        assert!(seps.contains(&"```\n".to_string()));
    }

    #[test]
    fn test_html_separators_contain_div() {
        let seps = Language::Html.get_separators();
        assert!(seps.contains(&"<div".to_string()));
    }

    #[test]
    fn test_proto_separators_contain_message() {
        let seps = Language::Proto.get_separators();
        assert!(seps.contains(&"\nmessage ".to_string()));
    }

    #[test]
    fn test_cobol_separators_contain_division() {
        let seps = Language::Cobol.get_separators();
        assert!(seps.contains(&"\nPROCEDURE DIVISION.".to_string()));
    }

    #[test]
    fn test_haskell_separators_contain_module() {
        let seps = Language::Haskell.get_separators();
        assert!(seps.contains(&"\nmodule ".to_string()));
    }

    #[test]
    fn test_elixir_separators_contain_defmodule() {
        let seps = Language::Elixir.get_separators();
        assert!(seps.contains(&"\ndefmodule ".to_string()));
    }

    // ========================================================================
    // Separator count tests (sanity check)
    // ========================================================================

    #[test]
    fn test_c_cpp_have_same_separators() {
        // C and Cpp use the same match arm
        assert_eq!(Language::C.get_separators(), Language::Cpp.get_separators());
    }

    #[test]
    fn test_separators_not_too_few() {
        // Each language should have at least 4 separators
        // (typically includes specific keywords + "\n\n" + "\n" + " " + "")
        let languages = [
            Language::C,
            Language::Python,
            Language::Rust,
            Language::Go,
            Language::Java,
        ];

        for lang in languages {
            let count = lang.get_separators().len();
            assert!(
                count >= 4,
                "{:?} should have at least 4 separators, got {}",
                lang,
                count
            );
        }
    }

    #[test]
    fn test_double_newline_present_in_most_languages() {
        // Most code languages have "\n\n" as a separator
        let languages = [
            Language::C,
            Language::Python,
            Language::Rust,
            Language::Go,
            Language::Java,
            Language::Js,
            Language::Ts,
        ];

        for lang in languages {
            let seps = lang.get_separators();
            assert!(
                seps.contains(&"\n\n".to_string()),
                "{:?} should contain double newline separator",
                lang
            );
        }
    }
}
