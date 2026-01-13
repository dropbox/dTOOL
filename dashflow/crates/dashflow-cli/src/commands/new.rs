//! `dashflow new` - Create a new DashFlow application with production defaults
//!
//! This command scaffolds a new DashFlow project with best practices:
//! - Provider-agnostic LLM via dashflow-factories
//! - TracedChatModel with callbacks, retry, and rate limiting
//! - CostTracker for budget management
//! - Optional DashStream observability
//! - Proper Cargo.toml with all dependencies
//!
//! # Usage
//!
//! ```bash
//! # Create a basic app (ReAct agent pattern)
//! dashflow new my-agent
//!
//! # Create a RAG pipeline
//! dashflow new my-rag --template rag
//!
//! # Create with observability enabled by default
//! dashflow new my-monitored-agent --with-observability
//! ```

use anyhow::{Context, Result};
use clap::Args;
use std::fs;
use std::path::Path;

/// Create a new DashFlow application with production defaults
#[derive(Args, Debug)]
pub struct NewArgs {
    /// Name of the new project (will create a directory with this name)
    pub name: String,

    /// Template to use for the project
    #[arg(long, short, default_value = "agent")]
    pub template: Template,

    /// Include DashStream observability feature by default
    #[arg(long)]
    pub with_observability: bool,

    /// Include example tests
    #[arg(long, default_value = "true")]
    pub with_tests: bool,

    /// Path to create the project in (default: current directory)
    #[arg(long, short)]
    pub path: Option<String>,

    /// Skip git initialization
    #[arg(long)]
    pub no_git: bool,

    /// Force overwrite if directory exists
    #[arg(long, short)]
    pub force: bool,
}

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Template {
    /// Simple ReAct agent with tool calling
    Agent,
    /// RAG pipeline with embeddings and vector search
    Rag,
    /// Multi-model comparison setup
    Comparison,
    /// Minimal example (just DashFlowApp builder)
    Minimal,
}

pub async fn run(args: NewArgs) -> Result<()> {
    // Determine project directory and name
    let project_dir = match &args.path {
        Some(p) => Path::new(p).join(&args.name),
        None => Path::new(&args.name).to_path_buf(),
    };

    // Extract just the project name (final component of path)
    let project_name = project_dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&args.name)
        .to_string();

    // Create args with normalized name for template generation
    let args = NewArgs {
        name: project_name.clone(),
        template: args.template,
        with_observability: args.with_observability,
        with_tests: args.with_tests,
        path: args.path.clone(),
        no_git: args.no_git,
        force: args.force,
    };

    // Check if directory exists
    if project_dir.exists() && !args.force {
        anyhow::bail!(
            "Directory '{}' already exists. Use --force to overwrite.",
            project_dir.display()
        );
    }

    // Create directory structure
    fs::create_dir_all(&project_dir).context("Failed to create project directory")?;
    fs::create_dir_all(project_dir.join("src")).context("Failed to create src directory")?;

    if args.with_tests {
        fs::create_dir_all(project_dir.join("tests"))
            .context("Failed to create tests directory")?;
    }

    // Generate files based on template
    let cargo_toml = generate_cargo_toml(&args);
    let main_rs = generate_main_rs(&args);
    let readme = generate_readme(&args);

    // Write files
    fs::write(project_dir.join("Cargo.toml"), cargo_toml).context("Failed to write Cargo.toml")?;
    fs::write(project_dir.join("src/main.rs"), main_rs).context("Failed to write src/main.rs")?;
    fs::write(project_dir.join("README.md"), readme).context("Failed to write README.md")?;

    // Add test file if requested
    if args.with_tests {
        let test_rs = generate_test_rs(&args);
        fs::write(project_dir.join("tests/integration.rs"), test_rs)
            .context("Failed to write tests/integration.rs")?;
    }

    // Initialize git if not disabled
    if !args.no_git {
        let gitignore = generate_gitignore();
        fs::write(project_dir.join(".gitignore"), gitignore)
            .context("Failed to write .gitignore")?;

        // Try to initialize git, but don't fail if git isn't available
        // M-499: Log warning on git init failure instead of silent swallow
        match std::process::Command::new("git")
            .args(["init"])
            .current_dir(&project_dir)
            .output()
        {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!(
                        "Warning: git init failed with status {}. \
                         Project created without git initialization. \
                         You can run 'git init' manually later.",
                        output.status
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not run git init: {}. \
                     Project created without git initialization. \
                     Ensure git is installed and in PATH, or use --no-git flag.",
                    e
                );
            }
        }
    }

    // Print success message
    println!("Created new DashFlow project: {}", args.name);
    println!();
    println!("Project structure:");
    println!("  {}/", args.name);
    println!("  ├── Cargo.toml");
    println!("  ├── README.md");
    println!("  ├── src/");
    println!("  │   └── main.rs");
    if args.with_tests {
        println!("  └── tests/");
        println!("      └── integration.rs");
    }
    println!();
    println!("To get started:");
    println!("  cd {}", args.name);
    println!("  cargo run");
    println!();

    if args.with_observability {
        println!("Observability is enabled. To start the observability stack:");
        println!("  docker-compose -f docker-compose.dashstream.yml up -d");
        println!("  cargo run --features dashstream");
        println!();
    }

    Ok(())
}

fn generate_cargo_toml(args: &NewArgs) -> String {
    let observability_deps = if args.with_observability {
        r#"
# Observability (optional)
dashflow-streaming = { git = "https://github.com/dashflow-ai/dashflow", optional = true }
"#
    } else {
        ""
    };

    let observability_features = if args.with_observability {
        r#"
# Enable DashStream observability
dashstream = ["dashflow/dashstream", "dep:dashflow-streaming"]
"#
    } else {
        ""
    };

    let extra_deps = match args.template {
        Template::Rag => {
            r#"
# RAG dependencies
dashflow-chroma = { git = "https://github.com/dashflow-ai/dashflow" }
"#
        }
        _ => "",
    };

    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2021"

[dependencies]
# Core DashFlow
dashflow = {{ git = "https://github.com/dashflow-ai/dashflow" }}
dashflow-factories = {{ git = "https://github.com/dashflow-ai/dashflow" }}
dashflow-observability = {{ git = "https://github.com/dashflow-ai/dashflow" }}

# Provider crates (add the ones you need)
dashflow-openai = {{ git = "https://github.com/dashflow-ai/dashflow" }}
# dashflow-anthropic = {{ git = "https://github.com/dashflow-ai/dashflow" }}
# dashflow-ollama = {{ git = "https://github.com/dashflow-ai/dashflow" }}
{extra_deps}{observability_deps}
# Async runtime
tokio = {{ version = "1", features = ["full"] }}

# Error handling
anyhow = "1.0"

# Logging
tracing = "0.1"
tracing-subscriber = {{ version = "0.3", features = ["env-filter"] }}

[features]
default = []
{observability_features}
[dev-dependencies]
# Testing
tokio-test = "0.4"
"#,
        name = args.name,
        extra_deps = extra_deps,
        observability_deps = observability_deps,
        observability_features = observability_features,
    )
}

fn generate_main_rs(args: &NewArgs) -> String {
    match args.template {
        Template::Agent => generate_agent_main(args),
        Template::Rag => generate_rag_main(args),
        Template::Comparison => generate_comparison_main(args),
        Template::Minimal => generate_minimal_main(args),
    }
}

fn generate_agent_main(args: &NewArgs) -> String {
    let observability_imports = if args.with_observability {
        r#"
#[cfg(feature = "dashstream")]
use dashflow_streaming::DashStreamCallback;
"#
    } else {
        ""
    };

    let observability_setup = if args.with_observability {
        r#"
    // Set up DashStream callback for observability
    #[cfg(feature = "dashstream")]
    {
        let kafka_brokers = std::env::var("KAFKA_BROKERS")
            .unwrap_or_else(|_| "localhost:9092".to_string());
        let topic = std::env::var("DASHSTREAM_TOPIC")
            .unwrap_or_else(|_| "dashstream-quality".to_string());

        if let Ok(callback) = DashStreamCallback::new(&kafka_brokers, &topic) {
            tracing::info!("DashStream observability enabled");
            // Add callback to your graph execution
        }
    }
"#
    } else {
        ""
    };

    format!(
        r#"//! {name} - A DashFlow Agent Application
//!
//! This agent uses the ReAct pattern with tool calling to solve tasks.
//!
//! ## Features
//!
//! - Provider-agnostic LLM (auto-detects OpenAI, Anthropic, Ollama, etc.)
//! - TracedChatModel with callbacks, retry, and rate limiting
//! - CostTracker for budget management
//! - Ready for production deployment
//!
//! ## Usage
//!
//! ```bash
//! # Set your LLM provider API key
//! export OPENAI_API_KEY="your-key"
//!
//! # Run the agent
//! cargo run
//! ```

use anyhow::Result;
use dashflow::prelude::*;
use dashflow_factories::{{create_llm, LLMRequirements}};
use dashflow_observability::cost::CostTracker;
use dashflow::core::{{
    callbacks::{{CallbackManager, ConsoleCallbackHandler}},
    config::RunnableConfig,
    language_models::traced::TracedChatModel,
    rate_limiters::InMemoryRateLimiter,
    retry::RetryPolicy,
}};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing_subscriber::{{fmt, prelude::*, EnvFilter}};
{observability_imports}
#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    tracing::info!("Starting {name}");

    // Create provider-agnostic LLM
    let raw_llm = create_llm(LLMRequirements {{
        needs_tools: true,
        ..Default::default()
    }})
    .await?;

    // Set up rate limiter
    let rate_limiter = Arc::new(InMemoryRateLimiter::new(
        10.0,  // 10 requests per second
        Duration::from_millis(50),
        20.0,  // burst capacity
    ));

    // Set up retry policy
    let retry_policy = RetryPolicy::default_jitter(3)
        .with_rate_limiter(rate_limiter.clone());

    // Set up callbacks for observability
    let mut callbacks = CallbackManager::new();
    callbacks.add_handler(Arc::new(ConsoleCallbackHandler::new(true)));

    // Create TracedChatModel with all production features
    let llm = TracedChatModel::builder_from_arc(raw_llm)
        .service_name("{name}")
        .callback_manager(callbacks.clone())
        .retry_policy(retry_policy)
        .rate_limiter(rate_limiter)
        .build();

    // Set up cost tracking
    let cost_tracker = Arc::new(Mutex::new(
        CostTracker::with_defaults()
            .with_daily_budget(100.0)  // $100/day budget
    ));

    // Create RunnableConfig with metadata
    let config = RunnableConfig::new()
        .with_run_name("{name}")
        .with_tag("production")
        .with_callbacks(callbacks);
{observability_setup}
    // Example: Run a simple query
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("Hello! What can you help me with today?"),
    ];

    tracing::info!("Sending query to LLM...");
    let response = llm.generate(&messages, None, None, None, None).await?;

    if let Some(content) = response.content() {{
        println!("Assistant: {{}}", content);
    }}

    // Print cost summary
    let tracker = cost_tracker.lock().await;
    let report = tracker.report();
    println!("\\nCost Summary:");
    println!("  Total calls: {{}}", report.total_calls());
    println!("  Total cost: ${{:.4}}", report.total_cost());

    Ok(())
}}
"#,
        name = args.name,
        observability_imports = observability_imports,
        observability_setup = observability_setup,
    )
}

fn generate_rag_main(args: &NewArgs) -> String {
    format!(
        r#"//! {name} - A DashFlow RAG Pipeline
//!
//! This application demonstrates a production-ready RAG pipeline with:
//! - Chunked document ingestion
//! - Vector similarity search
//! - Contextual answer generation
//!
//! ## Usage
//!
//! ```bash
//! # Set your API keys
//! export OPENAI_API_KEY="your-key"
//!
//! # Run the RAG pipeline
//! cargo run -- --query "What is DashFlow?"
//! ```

use anyhow::Result;
use dashflow::prelude::*;
use dashflow_factories::{{create_llm, create_embeddings, LLMRequirements, EmbeddingRequirements}};
use dashflow_observability::cost::CostTracker;
use dashflow::core::{{
    callbacks::{{CallbackManager, ConsoleCallbackHandler}},
    config::RunnableConfig,
    language_models::traced::TracedChatModel,
    rate_limiters::InMemoryRateLimiter,
    retry::RetryPolicy,
}};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;
use tracing_subscriber::{{fmt, prelude::*, EnvFilter}};

#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    tracing::info!("Starting {name} RAG pipeline");

    // Create provider-agnostic LLM
    let raw_llm = create_llm(LLMRequirements::default()).await?;

    // Create provider-agnostic embeddings
    let embeddings = create_embeddings(EmbeddingRequirements::default()).await?;

    // Set up rate limiter
    let rate_limiter = Arc::new(InMemoryRateLimiter::new(
        10.0,
        Duration::from_millis(50),
        20.0,
    ));

    // Set up retry policy
    let retry_policy = RetryPolicy::default_jitter(3)
        .with_rate_limiter(rate_limiter.clone());

    // Set up callbacks
    let mut callbacks = CallbackManager::new();
    callbacks.add_handler(Arc::new(ConsoleCallbackHandler::new(true)));

    // Create TracedChatModel
    let llm = TracedChatModel::builder_from_arc(raw_llm)
        .service_name("{name}")
        .callback_manager(callbacks.clone())
        .retry_policy(retry_policy)
        .rate_limiter(rate_limiter)
        .build();

    // Set up cost tracking
    let cost_tracker = Arc::new(Mutex::new(
        CostTracker::with_defaults()
            .with_daily_budget(100.0)
    ));

    // Example: Process a query
    let query = std::env::args().nth(2).unwrap_or_else(|| "What is DashFlow?".to_string());

    tracing::info!("Processing query: {{}}", query);

    // Generate embeddings for query
    let query_embedding = embeddings.embed_query(&query).await?;
    tracing::info!("Query embedding dimension: {{}}", query_embedding.len());

    // Vector store integration: Deferred - requires vector store dependency and configuration
    // When implemented: let results = vector_store._similarity_search(&query_embedding, 5).await?;

    // Generate answer with context
    let messages = vec![
        Message::system("You are a helpful assistant. Answer based on the provided context."),
        Message::human(&format!(
            "Question: {{}}\\n\\nContext: (No documents loaded yet - add vector store integration)",
            query
        )),
    ];

    let response = llm.generate(&messages, None, None, None, None).await?;

    if let Some(content) = response.content() {{
        println!("\\nAnswer: {{}}", content);
    }}

    // Print cost summary
    let tracker = cost_tracker.lock().await;
    let report = tracker.report();
    println!("\\nCost Summary:");
    println!("  Total calls: {{}}", report.total_calls());
    println!("  Total cost: ${{:.4}}", report.total_cost());

    Ok(())
}}
"#,
        name = args.name,
    )
}

fn generate_comparison_main(args: &NewArgs) -> String {
    format!(
        r#"//! {name} - Multi-Model Comparison
//!
//! This application compares responses from multiple LLM providers.
//!
//! ## Usage
//!
//! ```bash
//! # Set API keys for providers you want to compare
//! export OPENAI_API_KEY="your-key"
//! export ANTHROPIC_API_KEY="your-key"
//!
//! cargo run
//! ```

use anyhow::Result;
use dashflow::prelude::*;
use dashflow::core::language_models::ChatModel;
use dashflow_openai::ChatOpenAI;
// Uncomment to add more providers:
// use dashflow_anthropic::ChatAnthropic;
// use dashflow_ollama::ChatOllama;
use std::sync::Arc;
use std::time::Instant;
use tracing_subscriber::{{fmt, prelude::*, EnvFilter}};

#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    tracing::info!("Starting {name}");

    // Configure models to compare
    let models: Vec<(&str, Arc<dyn ChatModel>)> = vec![
        ("OpenAI GPT-4o", Arc::new(ChatOpenAI::new("gpt-4o")?)),
        // Add more models:
        // ("Anthropic Claude", Arc::new(ChatAnthropic::new("claude-3-sonnet-20240229")?)),
        // ("Ollama Llama", Arc::new(ChatOllama::new("llama2")?)),
    ];

    let prompt = "Explain the concept of recursion in programming in exactly two sentences.";

    println!("Prompt: {{}}\\n", prompt);
    println!("{{}}\\n", "=".repeat(80));

    for (name, model) in &models {{
        println!("Model: {{}}", name);
        println!("{{}}", "-".repeat(40));

        let messages = vec![
            Message::system("You are a helpful assistant. Be concise."),
            Message::human(prompt),
        ];

        let start = Instant::now();
        let response = model.generate(&messages, None, None, None, None).await?;
        let elapsed = start.elapsed();

        if let Some(content) = response.content() {{
            println!("Response: {{}}\\n", content);
        }}
        println!("Latency: {{:.2?}}\\n", elapsed);
        println!("{{}}\\n", "=".repeat(80));
    }}

    Ok(())
}}
"#,
        name = args.name,
    )
}

fn generate_minimal_main(args: &NewArgs) -> String {
    format!(
        r#"//! {name} - Minimal DashFlow Application
//!
//! A minimal example using the DashFlowApp builder for quick setup.
//!
//! ## Usage
//!
//! ```bash
//! export OPENAI_API_KEY="your-key"
//! cargo run
//! ```

use anyhow::Result;
use dashflow::prelude::*;
use dashflow_factories::{{create_llm, LLMRequirements}};
use dashflow::core::language_models::traced::TracedChatModel;
use tracing_subscriber::{{fmt, prelude::*, EnvFilter}};

#[tokio::main]
async fn main() -> Result<()> {{
    // Initialize logging
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env().add_directive("info".parse()?))
        .init();

    // Quick setup: create provider-agnostic LLM
    let llm = create_llm(LLMRequirements::default()).await?;

    // Wrap with tracing
    let traced_llm = TracedChatModel::new(llm.as_ref().clone());

    // Use it
    let messages = vec![
        Message::human("Hello! What is 2 + 2?"),
    ];

    let response = traced_llm.generate(&messages, None, None, None, None).await?;

    if let Some(content) = response.content() {{
        println!("{{}}", content);
    }}

    Ok(())
}}
"#,
        name = args.name,
    )
}

fn generate_test_rs(args: &NewArgs) -> String {
    format!(
        r#"//! Integration tests for {name}

use anyhow::Result;

/// Test that the application can be built
#[tokio::test]
async fn test_app_builds() -> Result<()> {{
    // This test just verifies the app compiles and basic dependencies work
    Ok(())
}}

/// Test basic LLM factory
#[tokio::test]
#[ignore = "requires API key"]
async fn test_llm_factory() -> Result<()> {{
    use dashflow_factories::{{create_llm, LLMRequirements}};

    let llm = create_llm(LLMRequirements::default()).await?;
    assert!(!llm.model_name().is_empty());

    Ok(())
}}
"#,
        name = args.name,
    )
}

fn generate_readme(args: &NewArgs) -> String {
    let template_desc = match args.template {
        Template::Agent => "ReAct Agent with tool calling",
        Template::Rag => "RAG Pipeline with vector search",
        Template::Comparison => "Multi-model comparison",
        Template::Minimal => "Minimal DashFlow example",
    };

    let observability_section = if args.with_observability {
        r#"
## Observability

This project includes DashStream observability support.

### Start the Observability Stack

```bash
# Start Kafka, Prometheus, Grafana
docker-compose -f docker-compose.dashstream.yml up -d

# Run with observability
cargo run --features dashstream
```

### View Metrics

- Grafana Dashboard: http://localhost:3000
- Prometheus: http://localhost:9090
"#
    } else {
        r#"
## Adding Observability

To enable DashStream observability:

1. Add to Cargo.toml features:
   ```toml
   dashstream = ["dashflow/dashstream", "dep:dashflow-streaming"]
   ```

2. Run with feature flag:
   ```bash
   cargo run --features dashstream
   ```
"#
    };

    format!(
        r#"# {name}

{template_desc}

Built with [DashFlow](https://github.com/dashflow-ai/dashflow) - Production-grade LLM application framework.

## Quick Start

```bash
# Set your API key (auto-detects provider)
export OPENAI_API_KEY="your-key"
# OR
export ANTHROPIC_API_KEY="your-key"
# OR use local Ollama (no key needed)

# Run the application
cargo run
```

## Features

- **Provider-agnostic**: Works with OpenAI, Anthropic, Ollama, and more
- **Production-ready**: Built-in retry, rate limiting, and cost tracking
- **Observable**: Tracing, callbacks, and optional DashStream integration
- **Type-safe**: Full Rust type safety with async/await
{observability_section}
## Project Structure

```
{name}/
├── Cargo.toml      # Dependencies and features
├── src/
│   └── main.rs     # Application entry point
└── tests/
    └── integration.rs  # Integration tests
```

## Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OPENAI_API_KEY` | OpenAI API key | - |
| `ANTHROPIC_API_KEY` | Anthropic API key | - |
| `OLLAMA_HOST` | Ollama server URL | `http://localhost:11434` |
| `LLM_RATE_LIMIT` | Requests per second | 10.0 |
| `DAILY_BUDGET` | Daily cost limit (USD) | unlimited |

## License

MIT
"#,
        name = args.name,
        template_desc = template_desc,
        observability_section = observability_section,
    )
}

fn generate_gitignore() -> String {
    r#"# Build artifacts
/target/

# IDE
.idea/
.vscode/
*.swp
*.swo

# Environment
.env
.env.local

# Logs
*.log

# OS
.DS_Store
Thumbs.db
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_cargo_toml() {
        let args = NewArgs {
            name: "test-app".to_string(),
            template: Template::Agent,
            with_observability: false,
            with_tests: true,
            path: None,
            no_git: false,
            force: false,
        };

        let toml = generate_cargo_toml(&args);
        assert!(toml.contains("name = \"test-app\""));
        assert!(toml.contains("dashflow ="));
        assert!(toml.contains("dashflow-factories ="));
    }

    #[test]
    fn test_generate_cargo_toml_with_observability() {
        let args = NewArgs {
            name: "monitored-app".to_string(),
            template: Template::Agent,
            with_observability: true,
            with_tests: true,
            path: None,
            no_git: false,
            force: false,
        };

        let toml = generate_cargo_toml(&args);
        assert!(toml.contains("dashstream"));
        assert!(toml.contains("dashflow-streaming"));
    }

    #[test]
    fn test_generate_readme() {
        let args = NewArgs {
            name: "my-agent".to_string(),
            template: Template::Agent,
            with_observability: false,
            with_tests: true,
            path: None,
            no_git: false,
            force: false,
        };

        let readme = generate_readme(&args);
        assert!(readme.contains("# my-agent"));
        assert!(readme.contains("ReAct Agent"));
        assert!(readme.contains("OPENAI_API_KEY"));
    }
}
