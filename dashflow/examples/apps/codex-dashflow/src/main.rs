//! Codex DashFlow CLI - AI-powered code generation and understanding
//!
//! ## Usage
//!
//! ```bash
//! # Generate code from description
//! cargo run -p codex-dashflow -- generate "a function that calculates fibonacci"
//!
//! # Explain code
//! cargo run -p codex-dashflow -- explain --file src/lib.rs
//!
//! # Suggest refactoring
//! cargo run -p codex-dashflow -- refactor --file src/lib.rs
//! ```

use anyhow::Result;
use clap::{Parser, Subcommand};
use codex_dashflow::docs_generator::DocsStyle;
use codex_dashflow::explainer::DetailLevel;
use codex_dashflow::refactor::RefactorFocus;
use codex_dashflow::test_generator::TestStyle;
use codex_dashflow::{
    default_agent_state, default_session_path, git_apply, git_diff, git_diff_staged,
    load_or_create_session, run_chat_loop_with_session, run_chat_loop_with_session_streaming,
    run_single_query, run_single_query_streaming, run_single_query_streaming_with_state,
    run_single_query_with_state, save_session, CodeExplainer, CodeGenerator, CodexConfig,
    DocsGenerator, McpStdioServer, RefactorSuggester, TestGenerator,
};
use common::{create_llm, LLMRequirements};
use dashflow::core::config_loader::env_vars::DASHFLOW_TELEMETRY_DISABLED;
use dashflow_observability::{init_tracing, TracingConfig};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

#[derive(Parser)]
#[command(
    author,
    version,
    about = "Codex DashFlow - AI-powered code generation and understanding"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Disable telemetry
    #[arg(long, global = true)]
    no_telemetry: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate code from natural language description
    Generate {
        /// Natural language description of what to generate
        description: String,

        /// Target language (rust, python, typescript)
        #[arg(long, default_value = "rust")]
        language: String,

        /// Output file path (prints to stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Include tests in generated code
        #[arg(long)]
        with_tests: bool,

        /// Include documentation in generated code
        #[arg(long)]
        with_docs: bool,
    },

    /// Explain code in plain English
    Explain {
        /// Path to the source file
        #[arg(short, long)]
        file: PathBuf,

        /// Specific symbol to explain (function, struct, etc.)
        #[arg(short, long)]
        symbol: Option<String>,

        /// Detail level: brief, normal, detailed
        #[arg(long, default_value = "normal")]
        detail: String,
    },

    /// Suggest refactoring improvements
    Refactor {
        /// Path to the source file
        #[arg(short, long)]
        file: PathBuf,

        /// Focus area: performance, readability, safety, all
        #[arg(long, default_value = "all")]
        focus: String,

        /// Apply suggestions automatically (creates backup)
        #[arg(long)]
        apply: bool,
    },

    /// Generate unit tests for code
    Test {
        /// Path to the source file
        #[arg(short, long)]
        file: PathBuf,

        /// Specific function to test
        #[arg(long)]
        function: Option<String>,

        /// Test style: unit, integration, property
        #[arg(long, default_value = "unit")]
        style: String,

        /// Output file for tests (appends to file if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Generate documentation for code
    Docs {
        /// Path to the source file
        #[arg(short, long)]
        file: PathBuf,

        /// Documentation style: rustdoc, docstring, markdown
        #[arg(long, default_value = "rustdoc")]
        style: String,

        /// Add examples to documentation
        #[arg(long)]
        with_examples: bool,
    },

    /// Interactive chat mode for code assistance
    Chat {
        /// Working directory context
        #[arg(short, long)]
        context: Option<PathBuf>,

        /// Persist conversation state to this session file (resumes if it exists)
        #[arg(long)]
        session: Option<PathBuf>,

        /// Require an existing session (defaults to ~/.codex-dashflow/sessions/default.json)
        #[arg(long)]
        resume: bool,

        /// Stream model output and tool activity as it happens (best-effort)
        #[arg(long)]
        stream: bool,
    },

    /// Execute a single prompt non-interactively
    ///
    /// Runs the coding agent with the given prompt, executes any necessary
    /// tool calls, and prints the result. Useful for scripting and CI/CD.
    Exec {
        /// The prompt/instruction to execute
        prompt: String,

        /// Working directory for file operations
        #[arg(short = 'd', long)]
        working_dir: Option<PathBuf>,

        /// Context files to include (read and added to prompt)
        #[arg(short, long)]
        context: Vec<PathBuf>,

        /// Output format: text (default), json
        #[arg(short, long, default_value = "text")]
        format: String,

        /// Persist conversation state to this session file (resumes if it exists)
        #[arg(long)]
        session: Option<PathBuf>,

        /// Require an existing session (defaults to ~/.codex-dashflow/sessions/default.json)
        #[arg(long)]
        resume: bool,

        /// Stream model output and tool activity as it happens (best-effort)
        #[arg(long)]
        stream: bool,
    },

    /// Run as MCP (Model Context Protocol) stdio server
    ///
    /// Exposes Codex DashFlow tools via MCP protocol over stdin/stdout.
    /// This allows LLM clients (Claude, OpenAI, etc.) to connect and use the tools.
    #[command(name = "mcp-server")]
    McpServer {
        /// Working directory for file operations
        #[arg(short, long)]
        working_dir: Option<PathBuf>,
    },

    /// Apply a git patch to the working tree
    ///
    /// Applies unified diffs to the current git working tree using `git apply`.
    /// Can apply from a file, stdin, or from working tree changes.
    Apply {
        /// Path to a patch/diff file to apply (reads from stdin if not provided)
        #[arg(short, long)]
        patch: Option<PathBuf>,

        /// Working directory (git repository)
        #[arg(short = 'd', long)]
        working_dir: Option<PathBuf>,

        /// Only check if the patch applies cleanly (don't modify files)
        #[arg(long)]
        dry_run: bool,

        /// Show the current unstaged changes as a diff
        #[arg(long, conflicts_with = "patch")]
        show_diff: bool,

        /// Show staged changes as a diff
        #[arg(long, conflicts_with = "patch")]
        show_staged: bool,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.no_telemetry {
        env::set_var(DASHFLOW_TELEMETRY_DISABLED, "1");
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();
    } else {
        let mut config = TracingConfig::new().with_service_name("codex-dashflow");
        if let Ok(endpoint) = env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
            if !endpoint.trim().is_empty() {
                config = config.with_otlp_endpoint(endpoint);
            }
        }
        init_tracing(config)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to initialize tracing: {}", e))?;
    }

    match cli.command {
        Commands::Generate {
            description,
            language,
            output,
            with_tests,
            with_docs,
        } => {
            let start = Instant::now();
            info!("Generating {} code from description", language);
            info!("Description: {}", description);

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;

            // Create config for the target language
            let config = match language.to_lowercase().as_str() {
                "python" => CodexConfig::for_python(),
                "typescript" | "ts" => CodexConfig::for_typescript(),
                _ => CodexConfig::for_rust(),
            };

            let generator = CodeGenerator::new(Arc::clone(&model), config);

            let code = if with_tests {
                generator.generate_with_tests(&description).await?
            } else {
                generator.generate(&description).await?
            };

            let output_text = if with_docs {
                let style = match language.to_lowercase().as_str() {
                    "python" => DocsStyle::Docstring,
                    "typescript" | "ts" => DocsStyle::Jsdoc,
                    _ => DocsStyle::Rustdoc,
                };
                let docs_generator = DocsGenerator::new(model);
                docs_generator.generate(&code, style, false).await?.content
            } else {
                code
            };

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                code_len = output_text.len(),
                "Code generation completed"
            );

            match output {
                Some(path) => {
                    tokio::fs::write(&path, &output_text).await?;
                    println!("Generated code written to {:?}", path);
                }
                None => println!("{}", output_text),
            }
        }

        Commands::Explain {
            file,
            symbol,
            detail,
        } => {
            let start = Instant::now();
            info!("Explaining code in {:?}", file);

            // Read the source file
            let code = tokio::fs::read_to_string(&file).await?;

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;
            let explainer = CodeExplainer::new(model);

            let explanation = match symbol {
                Some(sym) => explainer.explain_symbol(&code, &sym).await?,
                None => {
                    let detail_level: DetailLevel = detail.parse()?;
                    explainer.explain(&code, detail_level).await?
                }
            };

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                explanation_len = explanation.len(),
                "Code explanation completed"
            );

            println!("{}", explanation);
        }

        Commands::Refactor { file, focus, apply } => {
            let start = Instant::now();
            info!("Analyzing {:?} for refactoring suggestions", file);

            // Read the source file
            let code = tokio::fs::read_to_string(&file).await?;

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;
            let suggester = RefactorSuggester::new(model);

            let focus_level: RefactorFocus = focus.parse()?;
            let suggestions = suggester.suggest(&code, focus_level).await?;

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                suggestion_count = suggestions.len(),
                "Refactoring analysis completed"
            );

            if suggestions.is_empty() {
                println!("No refactoring suggestions found.");
            } else {
                println!("Found {} refactoring suggestion(s):\n", suggestions.len());
                for (i, suggestion) in suggestions.iter().enumerate() {
                    println!(
                        "{}. [{}] {}",
                        i + 1,
                        suggestion.priority,
                        suggestion.description
                    );
                    if let Some(ref original) = suggestion.original {
                        println!("   Original: {}", original);
                    }
                    if let Some(ref suggested) = suggestion.suggested {
                        println!("   Suggested: {}", suggested);
                    }
                    println!();
                }

                // Apply first suggestion if --apply flag is set
                if apply && !suggestions.is_empty() {
                    info!("Applying first suggestion...");
                    let backup_path = file.with_extension("rs.bak");
                    tokio::fs::copy(&file, &backup_path).await?;
                    println!("Backup created at {:?}", backup_path);

                    let modified = suggester.apply_suggestion(&code, &suggestions[0]).await?;
                    tokio::fs::write(&file, &modified).await?;
                    info!(file = %file.display(), "Refactoring applied");
                    println!("Refactoring applied to {:?}", file);
                }
            }
        }

        Commands::Test {
            file,
            function,
            style,
            output,
        } => {
            let start = Instant::now();
            info!("Generating tests for {:?}", file);

            // Read the source file
            let code = tokio::fs::read_to_string(&file).await?;

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;
            let generator = TestGenerator::new(model);

            let test_style: TestStyle = style.parse()?;
            let result = match function {
                Some(func) => {
                    generator
                        .generate_for_function(&code, &func, test_style)
                        .await?
                }
                None => generator.generate(&code, test_style).await?,
            };

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                test_count = result.test_count,
                "Test generation completed"
            );

            println!("{}", result.description);
            println!();

            match output {
                Some(path) => {
                    tokio::fs::write(&path, &result.code).await?;
                    println!("Tests written to {:?}", path);
                }
                None => println!("{}", result.code),
            }
        }

        Commands::Docs {
            file,
            style,
            with_examples,
        } => {
            let start = Instant::now();
            info!("Generating documentation for {:?}", file);

            // Read the source file
            let code = tokio::fs::read_to_string(&file).await?;

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;
            let generator = DocsGenerator::new(model);

            let docs_style: DocsStyle = style.parse()?;
            let result = generator.generate(&code, docs_style, with_examples).await?;

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                style = %result.style,
                "Documentation generation completed"
            );

            println!("{}", result.content);
        }

        Commands::Chat {
            context,
            session,
            resume,
            stream,
        } => {
            info!("Starting interactive chat mode");

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;

            // Run the agentic chat loop with file/shell tools
            let session_path = if resume && session.is_none() {
                Some(default_session_path()?)
            } else {
                session
            };
            if stream {
                run_chat_loop_with_session_streaming(
                    model,
                    context.as_deref(),
                    session_path.as_deref(),
                    resume,
                )
                .await?;
            } else {
                run_chat_loop_with_session(model, context.as_deref(), session_path.as_deref(), resume)
                    .await?;
            }
        }

        Commands::Exec {
            prompt,
            working_dir,
            context,
            format,
            session,
            resume,
            stream,
        } => {
            let start = Instant::now();
            info!("Executing single prompt non-interactively");

            if stream && format.as_str() == "json" {
                return Err(anyhow::anyhow!(
                    "--stream is not supported with --format json (streaming writes to stdout)"
                ));
            }

            // Build full prompt with context files
            let mut full_prompt = String::new();

            // Read and include context files
            for context_path in &context {
                match tokio::fs::read_to_string(&context_path).await {
                    Ok(content) => {
                        full_prompt.push_str(&format!(
                            "--- Content of {} ---\n{}\n---\n\n",
                            context_path.display(),
                            content
                        ));
                    }
                    Err(e) => {
                        eprintln!(
                            "Warning: Could not read context file {}: {}",
                            context_path.display(),
                            e
                        );
                    }
                }
            }

            full_prompt.push_str(&prompt);

            // Create LLM via factory
            let model = create_llm(LLMRequirements::default()).await?;

            let session_path = if resume && session.is_none() {
                Some(default_session_path()?)
            } else {
                session
            };

            let result = if let Some(session_path) = session_path.as_deref() {
                let mut sess = load_or_create_session(
                    session_path,
                    resume,
                    default_agent_state(),
                    working_dir.as_deref(),
                )
                .await?;
                let state = std::mem::replace(&mut sess.state, default_agent_state());
                let (response, final_state) = if stream {
                    run_single_query_streaming_with_state(
                        model,
                        state,
                        &full_prompt,
                        working_dir.as_deref(),
                    )
                    .await?
                } else {
                    run_single_query_with_state(model, state, &full_prompt, working_dir.as_deref())
                        .await?
                };
                sess.update_state(final_state);
                save_session(session_path, &sess).await?;
                response
            } else if stream {
                run_single_query_streaming(model, &full_prompt, working_dir.as_deref()).await?
            } else {
                run_single_query(model, &full_prompt, working_dir.as_deref()).await?
            };

            let duration = start.elapsed();
            info!(
                duration_ms = duration.as_millis() as u64,
                result_len = result.len(),
                "Exec completed"
            );

            // Output based on format
            if !stream {
                match format.as_str() {
                    "json" => {
                        let output = serde_json::json!({
                            "success": true,
                            "result": result,
                            "duration_ms": duration.as_millis() as u64
                        });
                        println!("{}", serde_json::to_string_pretty(&output)?);
                    }
                    _ => {
                        println!("{}", result);
                    }
                }
            }
        }

        Commands::McpServer { working_dir } => {
            let dir = working_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            info!(working_dir = %dir.display(), "Starting MCP stdio server");

            let mut server = McpStdioServer::new(dir);
            server.run().await?;
        }

        Commands::Apply {
            patch,
            working_dir,
            dry_run,
            show_diff,
            show_staged,
        } => {
            let dir = working_dir.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            info!(working_dir = %dir.display(), dry_run, "Apply command");

            // Handle show-diff mode
            if show_diff {
                let diff = git_diff(&dir).await?;
                if diff.is_empty() {
                    println!("No unstaged changes.");
                } else {
                    println!("{}", diff);
                }
                return Ok(());
            }

            // Handle show-staged mode
            if show_staged {
                let diff = git_diff_staged(&dir).await?;
                if diff.is_empty() {
                    println!("No staged changes.");
                } else {
                    println!("{}", diff);
                }
                return Ok(());
            }

            // Read patch content
            let patch_content = match patch {
                Some(path) => {
                    tokio::fs::read_to_string(&path)
                        .await
                        .map_err(|e| anyhow::anyhow!("Failed to read patch file: {}", e))?
                }
                None => {
                    // Read from stdin
                    use std::io::Read;
                    let mut content = String::new();
                    std::io::stdin()
                        .read_to_string(&mut content)
                        .map_err(|e| anyhow::anyhow!("Failed to read patch from stdin: {}", e))?;
                    content
                }
            };

            if patch_content.trim().is_empty() {
                println!("No patch content provided.");
                return Ok(());
            }

            // Apply the patch
            let result = git_apply(&dir, &patch_content, dry_run).await?;

            if result.success {
                println!("{}", result.message);
                if !result.modified_files.is_empty() {
                    println!("\nFiles affected:");
                    for file in &result.modified_files {
                        println!("  {}", file.display());
                    }
                }
            } else {
                eprintln!("{}", result.message);
                std::process::exit(1);
            }
        }
    }

    Ok(())
}
