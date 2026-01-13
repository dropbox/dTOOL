//! @dashflow-module
//! @name document_loaders
//! @category core
//! @status stable
//!
//! Document loaders for various file formats and sources.
//!
//! This module provides implementations of the `DocumentLoader` trait for loading
//! documents from files, URLs, and other sources.
//!
//! # Organization
//!
//! Loaders are organized by category:
//!
//! - [`formats`] - File format loaders (text, JSON, PDF, CSV, etc.)
//! - [`languages`] - Programming language source code loaders
//! - [`config`] - Configuration and build system loaders
//! - [`integrations`] - External data source integrations (databases, cloud storage, APIs)
//! - [`messaging`] - Email and messaging format loaders
//! - [`specialized`] - Domain-specific format loaders
//! - [`knowledge`] - Personal knowledge management system loaders
//! - [`core`] - Core utility loaders (directory, URL, `DataFrame`, etc.)
//!
//! # Migration Status
//!
//! **COMPLETE:** Infrastructure created
//! - Directory structure established
//! - Base module with common utilities
//! - Category modules with placeholders
//!
//! **COMPLETE:** File format loaders extracted
//! - Text formats (5 loaders)
//! - Structured formats (7 loaders)
//! - Document formats (3 loaders)
//! - Archive formats (3 loaders)
//! - Media formats (3 loaders)
//!
//! **COMPLETE:** Programming language loaders extracted
//! - Systems languages (9 loaders)
//! - Scripting languages (7 loaders)
//! - JVM languages (4 loaders)
//! - Functional languages (8 loaders)
//! - Web languages (6 loaders)
//! - Shell languages (6 loaders)
//!
//! **COMPLETE:** Integration loaders extracted
//! - Databases (10 loaders)
//! - Cloud storage (6 loaders)
//! - `SaaS` platforms (10 loaders)
//! - Communication platforms (9 loaders)
//! - Social media (3 loaders)
//! - Content platforms (6 loaders)
//! - Developer platforms (3 loaders)
//!
//! **COMPLETE:** Specialized and knowledge loaders extracted
//! - Email/messaging formats (7 loaders)
//! - Specialized formats (11 loaders)
//! - Knowledge management (5 loaders)
//!
//! **COMPLETE:** Final cleanup complete
//! - Configuration and build systems (config/ module) ✓
//! - Core utility loaders (core/ module) ✓
//! - Legacy.rs file removed ✓
//! - All 169 loaders extracted and properly re-exported ✓
//!
//! During migration, all loaders remain accessible at the top level for backward compatibility.
//!
//! # Examples
//!
//! ```no_run
//! use dashflow::core::document_loaders::TextLoader;
//! use dashflow::core::documents::DocumentLoader;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let loader = TextLoader::new("example.txt");
//! let documents = loader.load().await?;
//! println!("Loaded {} documents", documents.len());
//! # Ok(())
//! # }
//! ```

// Base module with common utilities and DocumentLoader re-export
pub mod base;

// Category modules (organized by loader type)
pub mod config;
pub mod core;
pub mod formats;
pub mod integrations;
pub mod knowledge;
pub mod languages;
pub mod messaging;
pub mod specialized;

// Legacy modules (being migrated)
pub mod documents;
pub mod text;

// Re-export base utilities and DocumentLoader trait
pub use base::*;

// Re-export loaders from legacy text module (to be fully migrated)
pub use text::{BinaryFileLoader, UnstructuredFileLoader};

// ============================================================================
// BACKWARD COMPATIBILITY: Re-export all loaders from legacy file
// ============================================================================
//
// During migration, we maintain full backward compatibility by re-exporting
// everything from the old document_loaders.rs file (now renamed to _legacy.rs).
//
// This ensures that existing code continues to work:
//
//   use dashflow::core::document_loaders::TextLoader;  // Still works!
//
// All loaders have been migrated from the legacy file to category modules.
// Re-exports below provide backward compatibility while enabling organized imports.

// ============================================================================
// PHASE 2: Text Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export text format loaders from formats::text module
// These replace the legacy implementations:
pub use formats::text::{
    AsciiDocLoader, HTMLLoader, MarkdownLoader, RSTLoader, RTFLoader, TextLoader,
};

// ============================================================================
// PHASE 2: Structured Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export structured format loaders from formats::structured module
// These replace the legacy implementations:
pub use formats::structured::{
    CSVLoader, IniLoader, JSONLoader, TOMLLoader, TSVLoader, XMLLoader, YAMLLoader,
};

// ============================================================================
// PHASE 2: Document Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export document format loaders from formats::documents module
// These replace the legacy implementations:
pub use formats::documents::{EpubLoader, PDFLoader, PowerPointLoader, WordDocumentLoader};

// ============================================================================
// PHASE 2: Archive Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export archive format loaders from formats::archives module
// These replace the legacy implementations:
pub use formats::archives::{GzipFileLoader, TarFileLoader, ZipFileLoader};

// ============================================================================
// PHASE 2: Media Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export media format loaders from formats::media module
// These replace the legacy implementations:
pub use formats::media::{NotebookLoader, SRTLoader, WebVTTLoader};

// ============================================================================
// PHASE 3: Systems Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export systems language loaders from languages::systems module
// These replace the legacy implementations:
pub use languages::systems::{
    CppLoader, CrystalLoader, DLoader, GoLoader, NimLoader, RustFileLoader, SwiftLoader, VLoader,
    WASMLoader, ZigLoader,
};

// ============================================================================
// PHASE 3: Scripting Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export scripting language loaders from languages::scripting module
// These replace the legacy implementations:
pub use languages::scripting::{
    BashScriptLoader, FishLoader, JavaScriptLoader, PowerShellLoader, PythonFileLoader,
    TypeScriptLoader, ZshLoader,
};

// ============================================================================
// PHASE 3: JVM Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export JVM language loaders from languages::jvm module
// These replace the legacy implementations:
pub use languages::jvm::{GroovyLoader, JavaLoader, KotlinLoader, ScalaLoader};

// ============================================================================
// PHASE 3: Functional Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export functional language loaders from languages::functional module
// These replace the legacy implementations:
pub use languages::functional::{
    ClojureLoader, ElixirLoader, ErlangLoader, FSharpLoader, HaskellLoader, OCamlLoader,
    RacketLoader, SchemeLoader,
};

// ============================================================================
// PHASE 3: Web Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export web language loaders from languages::web module
// These replace the legacy implementations:
pub use languages::web::{JuliaLoader, LuaLoader, PerlLoader, PhpLoader, RLoader, RubyLoader};

// ============================================================================
// PHASE 3: Shell Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export shell language loaders from languages::shell module
// These replace the legacy implementations:
pub use languages::shell::{AwkLoader, CshLoader, KshLoader, SedLoader, TclLoader, TcshLoader};

// ============================================================================
// PHASE 4: Database Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export database loaders from integrations::databases module
// These replace the legacy implementations:
// NOTE: Currently no database loaders are implemented (all removed as dead code placeholders in N=301).
// When database loaders are implemented, export them here.

// ============================================================================
// PHASE 4: Cloud Storage Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export cloud storage loaders from integrations::cloud module
// These replace the legacy implementations:
// NOTE: Currently no cloud storage loaders are implemented (all removed as dead code placeholders in N=301).
// When cloud storage loaders are implemented, export them here.

// ============================================================================
// PHASE 4: SaaS Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export SaaS platform loaders from integrations::saas module
// NOTE: Currently no SaaS loaders are implemented (all removed as dead code placeholders in N=301).
// When SaaS loaders are implemented, export them here.

// ============================================================================
// PHASE 4: Communication Platform Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export communication platform loaders from integrations::communication module
// These replace the legacy implementations:
pub use integrations::communication::{
    DiscordLoader, FacebookChatLoader, IMessageChatLoader, SlackChatLoader, SlackExportLoader,
    TelegramChatLoader, TelegramLoader, WhatsAppChatLoader,
};
// NOTE: MicrosoftTeamsLoader removed (placeholder). Re-export here when implemented.

// ============================================================================
// PHASE 4: Social Media Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export social media loaders from integrations::social module
// These replace the legacy implementations:
pub use integrations::social::MastodonLoader;

// ============================================================================
// PHASE 4: Content Platform Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export content platform loaders from integrations::content module
// These replace the legacy implementations:
pub use integrations::content::{ArXivLoader, NewsLoader, WikipediaLoader};
// NOTE: 5 placeholder loaders removed. Re-export here when implemented:
//   YouTubeTranscriptLoader, GoogleSpeechToTextLoader, AssemblyAILoader, WeatherLoader, PsychicLoader

// ============================================================================
// PHASE 4: Developer Platform Integration Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export developer platform loaders from integrations::developer module
// These replace the legacy implementations:
pub use integrations::developer::{GitBookLoader, GitLoader};
// NOTE: BrowserlessLoader, ChromiumLoader, GitHubIssuesLoader removed (placeholders).
// Re-export here when implemented.

// ============================================================================
// PHASE 5: Email and Messaging Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export email and messaging loaders from messaging::email module
// These replace the legacy implementations:
pub use messaging::email::{
    EMLLoader, EMLXLoader, EmailLoader, ICSLoader, MBOXLoader, MHTMLLoader, VCFLoader,
};

// ============================================================================
// PHASE 5: Specialized Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export specialized format loaders from specialized module
// These replace the legacy implementations:
pub use specialized::{
    // Semantic/query languages and specialized data formats
    ARFFLoader,
    // Academic formats
    BibTeXLoader,
    CoNLLULoader,
    CypherLoader,
    GraphQLLoader,
    LaTeXLoader,
    NFOLoader,
    SGMLLoader,
    SPARQLLoader,
    TexinfoLoader,
    WARCLoader,
    XQueryLoader,
};

// ============================================================================
// PHASE 5: Knowledge Management Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export knowledge management loaders from knowledge module
// These replace the legacy implementations:
pub use knowledge::{ObsidianLoader, OrgModeLoader, RSSLoader, RoamLoader, SitemapLoader};

// ============================================================================
// PHASE 6: Configuration Format Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export configuration format loaders from config::formats module
// These replace the legacy implementations:
pub use config::{DhallLoader, EnvLoader, HCLLoader, JsonnetLoader, NixLoader, StarlarkLoader};

// ============================================================================
// PHASE 6: Build System Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export build system loaders from config::build module
// These replace the legacy implementations:
pub use config::{CMakeLoader, DockerfileLoader, MakefileLoader};

// ============================================================================
// PHASE 6: Template Language Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export template language loaders from config::templates module
// These replace the legacy implementations:
pub use config::{
    ERBLoader, HandlebarsLoader, Jinja2Loader, LiquidLoader, MustacheLoader, PugLoader,
};

// ============================================================================
// PHASE 6: Core Data Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export core data structure loaders from core::data module
// These replace the legacy implementations:
pub use core::{DataFrameLoader, DirectoryLoader, ExcelLoader, URLLoader};

// ============================================================================
// PHASE 6: Core Utility Loaders (Extracted from legacy.rs)
// ============================================================================
//
// Re-export core utility loaders from core::utility module
// These replace the legacy implementations:
pub use core::{DiffLoader, ForthLoader, LogFileLoader, SQLLoader, SVGLoader, XAMLLoader};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::documents::DocumentLoader;

    // ==========================================================================
    // Re-export Verification Tests
    // These tests verify that all loaders are properly re-exported from this module
    // ==========================================================================

    #[test]
    fn test_text_format_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<TextLoader>();
        _assert_document_loader::<MarkdownLoader>();
        _assert_document_loader::<HTMLLoader>();
        _assert_document_loader::<RSTLoader>();
        _assert_document_loader::<RTFLoader>();
        _assert_document_loader::<AsciiDocLoader>();
    }

    #[test]
    fn test_structured_format_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<JSONLoader>();
        _assert_document_loader::<CSVLoader>();
        _assert_document_loader::<TSVLoader>();
        _assert_document_loader::<XMLLoader>();
        _assert_document_loader::<YAMLLoader>();
        _assert_document_loader::<TOMLLoader>();
        _assert_document_loader::<IniLoader>();
    }

    #[test]
    fn test_document_format_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<PDFLoader>();
        _assert_document_loader::<WordDocumentLoader>();
        _assert_document_loader::<PowerPointLoader>();
        _assert_document_loader::<EpubLoader>();
    }

    #[test]
    fn test_archive_format_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<ZipFileLoader>();
        _assert_document_loader::<TarFileLoader>();
        _assert_document_loader::<GzipFileLoader>();
    }

    #[test]
    fn test_media_format_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<NotebookLoader>();
        _assert_document_loader::<SRTLoader>();
        _assert_document_loader::<WebVTTLoader>();
    }

    #[test]
    fn test_systems_language_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<RustFileLoader>();
        _assert_document_loader::<CppLoader>();
        _assert_document_loader::<GoLoader>();
        _assert_document_loader::<SwiftLoader>();
        _assert_document_loader::<ZigLoader>();
    }

    #[test]
    fn test_scripting_language_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<PythonFileLoader>();
        _assert_document_loader::<JavaScriptLoader>();
        _assert_document_loader::<TypeScriptLoader>();
        _assert_document_loader::<BashScriptLoader>();
    }

    #[test]
    fn test_jvm_language_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<JavaLoader>();
        _assert_document_loader::<KotlinLoader>();
        _assert_document_loader::<ScalaLoader>();
        _assert_document_loader::<GroovyLoader>();
    }

    #[test]
    fn test_functional_language_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<HaskellLoader>();
        _assert_document_loader::<OCamlLoader>();
        _assert_document_loader::<ElixirLoader>();
        _assert_document_loader::<ClojureLoader>();
    }

    #[test]
    fn test_web_language_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<PhpLoader>();
        _assert_document_loader::<RubyLoader>();
        _assert_document_loader::<PerlLoader>();
        _assert_document_loader::<LuaLoader>();
    }

    #[test]
    fn test_communication_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<SlackChatLoader>();
        _assert_document_loader::<DiscordLoader>();
        _assert_document_loader::<TelegramLoader>();
        _assert_document_loader::<WhatsAppChatLoader>();
    }

    #[test]
    fn test_content_platform_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<WikipediaLoader>();
        _assert_document_loader::<ArXivLoader>();
        _assert_document_loader::<NewsLoader>();
    }

    #[test]
    fn test_developer_platform_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<GitLoader>();
        _assert_document_loader::<GitBookLoader>();
    }

    #[test]
    fn test_email_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<EmailLoader>();
        _assert_document_loader::<EMLLoader>();
        _assert_document_loader::<MBOXLoader>();
        _assert_document_loader::<ICSLoader>();
        _assert_document_loader::<VCFLoader>();
    }

    #[test]
    fn test_specialized_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<BibTeXLoader>();
        _assert_document_loader::<LaTeXLoader>();
        _assert_document_loader::<TexinfoLoader>();
        _assert_document_loader::<GraphQLLoader>();
        _assert_document_loader::<SPARQLLoader>();
    }

    #[test]
    fn test_knowledge_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<ObsidianLoader>();
        _assert_document_loader::<OrgModeLoader>();
        _assert_document_loader::<RoamLoader>();
        _assert_document_loader::<RSSLoader>();
        _assert_document_loader::<SitemapLoader>();
    }

    #[test]
    fn test_config_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<EnvLoader>();
        _assert_document_loader::<HCLLoader>();
        _assert_document_loader::<NixLoader>();
        _assert_document_loader::<DhallLoader>();
        _assert_document_loader::<JsonnetLoader>();
        _assert_document_loader::<StarlarkLoader>();
    }

    #[test]
    fn test_build_system_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<MakefileLoader>();
        _assert_document_loader::<CMakeLoader>();
        _assert_document_loader::<DockerfileLoader>();
    }

    #[test]
    fn test_template_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<Jinja2Loader>();
        _assert_document_loader::<MustacheLoader>();
        _assert_document_loader::<HandlebarsLoader>();
        _assert_document_loader::<LiquidLoader>();
        _assert_document_loader::<ERBLoader>();
        _assert_document_loader::<PugLoader>();
    }

    #[test]
    fn test_core_data_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<DirectoryLoader>();
        _assert_document_loader::<URLLoader>();
        _assert_document_loader::<ExcelLoader>();
        _assert_document_loader::<DataFrameLoader>();
    }

    #[test]
    fn test_core_utility_loaders_exported() {
        fn _assert_document_loader<T: DocumentLoader>() {}

        _assert_document_loader::<DiffLoader>();
        _assert_document_loader::<LogFileLoader>();
        _assert_document_loader::<SQLLoader>();
        _assert_document_loader::<SVGLoader>();
        _assert_document_loader::<XAMLLoader>();
        _assert_document_loader::<ForthLoader>();
    }

    // ==========================================================================
    // Loader Construction Tests
    // ==========================================================================

    #[test]
    fn test_text_loader_construction() {
        let loader = TextLoader::new("test.txt");
        assert!(loader.file_path.ends_with("test.txt"));
    }

    #[test]
    fn test_json_loader_construction() {
        let loader = JSONLoader::new("test.json");
        assert!(loader.file_path.ends_with("test.json"));
    }

    #[test]
    fn test_csv_loader_construction() {
        let loader = CSVLoader::new("test.csv");
        assert!(loader.file_path.ends_with("test.csv"));
    }

    #[test]
    fn test_pdf_loader_construction() {
        let loader = PDFLoader::new("test.pdf");
        assert!(loader.file_path.ends_with("test.pdf"));
    }

    #[test]
    fn test_zip_loader_construction() {
        let loader = ZipFileLoader::new("test.zip");
        assert!(loader.file_path.ends_with("test.zip"));
    }

    #[test]
    fn test_directory_loader_construction() {
        let loader = DirectoryLoader::new("./docs");
        assert!(loader.dir_path.ends_with("docs"));
    }

    #[test]
    fn test_url_loader_construction() {
        let loader = URLLoader::new("https://example.com");
        assert_eq!(loader.url, "https://example.com");
    }

    #[test]
    fn test_wikipedia_loader_construction() {
        // WikipediaLoader fields are private, just test it can be constructed
        let _loader = WikipediaLoader::new("Rust");
    }

    #[test]
    fn test_arxiv_loader_construction() {
        // ArXivLoader fields are private, just test it can be constructed
        let _loader = ArXivLoader::new("2103.03404");
    }

    #[test]
    fn test_bibtex_loader_construction() {
        let loader = BibTeXLoader::new("refs.bib");
        assert!(loader.file_path.ends_with("refs.bib"));
    }

    #[test]
    fn test_latex_loader_construction() {
        let loader = LaTeXLoader::new("paper.tex");
        assert!(loader.file_path.ends_with("paper.tex"));
    }
}
