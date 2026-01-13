//! dTerm - A high-performance AI agent terminal written in Rust
//!
//! Copyright 2024-2025 Andrew Yates
//! Licensed under Apache License 2.0

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use dterm_core::grid::Grid;
use dterm_core::terminal::Terminal;

mod clipboard;
mod commands;
mod prompt;
mod pty;
mod telemetry;
mod tui;
mod updater;

/// dTerm - AI Agent Terminal
#[derive(Parser, Debug)]
#[command(name = "dterm")]
#[command(author = "Andrew Yates")]
#[command(version = "0.1.0")]
#[command(about = "A high-performance AI agent terminal", long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Run in print mode (non-interactive)
    #[arg(short, long, conflicts_with = "command")]
    print: Option<String>,

    /// Read input bytes from a file instead of --print
    #[arg(
        long,
        value_name = "PATH",
        conflicts_with = "print",
        conflicts_with = "command"
    )]
    input_file: Option<PathBuf>,

    /// Model to use
    #[arg(
        short,
        long,
        default_value = "claude-sonnet-4-20250514",
        conflicts_with = "command"
    )]
    model: String,

    /// Output format (text, json, stream-json)
    #[arg(long, default_value = "text", conflicts_with = "command")]
    output_format: String,

    /// Enable verbose output
    #[arg(short, long, conflicts_with = "command")]
    verbose: bool,

    /// Terminal rows for print mode
    #[arg(long, default_value_t = 24, conflicts_with = "command")]
    rows: u16,

    /// Terminal columns for print mode
    #[arg(long, default_value_t = 80, conflicts_with = "command")]
    cols: u16,
}

#[derive(Subcommand, Debug)]
enum Command {
    #[command(name = "passes", alias = "/passes")]
    Passes {
        /// Copy referral link to clipboard
        #[arg(long)]
        copy: bool,

        /// Override the guest passes store path
        #[arg(long, value_name = "PATH")]
        store_path: Option<PathBuf>,
    },
    #[command(name = "setup-github-actions", alias = "/setup-github-actions")]
    SetupGithubActions {
        /// Repository root (defaults to current directory)
        #[arg(long, value_name = "PATH")]
        repo_root: Option<PathBuf>,

        /// Workflow path override
        #[arg(long, value_name = "PATH")]
        workflow_path: Option<PathBuf>,

        /// GitHub Actions secret name(s)
        #[arg(long = "secret")]
        secrets: Vec<String>,

        /// Disable interactive prompts
        #[arg(long)]
        no_prompt: bool,
    },
    #[command(name = "install-github-app", alias = "/install-github-app")]
    InstallGithubApp {
        /// Repository root (defaults to current directory)
        #[arg(long, value_name = "PATH")]
        repo_root: Option<PathBuf>,

        /// Workflow path override
        #[arg(long, value_name = "PATH")]
        workflow_path: Option<PathBuf>,

        /// GitHub App client ID
        #[arg(long)]
        client_id: Option<String>,

        /// GitHub App client secret
        #[arg(long)]
        client_secret: Option<String>,

        /// OAuth redirect URI
        #[arg(long)]
        redirect_uri: Option<String>,

        /// Disable interactive prompts
        #[arg(long)]
        no_prompt: bool,
    },
    #[command(name = "native-update")]
    NativeUpdate {
        /// Target version to install
        #[arg(long)]
        target_version: String,

        /// Download URL for the new binary
        #[arg(long)]
        download_url: String,

        /// Install path for the binary
        #[arg(long, value_name = "PATH")]
        install_path: Option<PathBuf>,

        /// Data directory for lock files and staging
        #[arg(long, value_name = "PATH")]
        data_dir: Option<PathBuf>,
    },
}

fn main() {
    let args = Args::parse();

    if let Some(command) = args.command {
        if let Err(err) = handle_command(command) {
            eprintln!("Command error: {err}");
            std::process::exit(1);
        }
        return;
    }

    if args.verbose {
        eprintln!("dTerm v0.1.0");
        eprintln!("Model: {}", args.model);
    }

    // Print mode: process input and dump grid
    if let Some(input) = load_input(&args) {
        let mut terminal = Terminal::new(args.rows, args.cols);
        terminal.process(&input);
        dump_grid(terminal.grid());
        return;
    }

    // Interactive mode: run TUI
    run_interactive(args);
}

fn handle_command(command: Command) -> anyhow::Result<()> {
    use commands::github_actions::{run_setup, GithubActionsSetupConfig};
    use commands::github_app::{
        run_wizard, GithubAppInstallConfig, OAuthConfig, DEFAULT_GITHUB_APP_SECRET,
    };
    use commands::passes::GuestPassesCommand;
    use prompt::StdioPrompt;
    use telemetry::NoopTelemetry;
    use updater::{run_native_update, NativeUpdateConfig, ReqwestDownloader};

    let telemetry = NoopTelemetry;
    let mut prompt = StdioPrompt;

    match command {
        Command::Passes { copy, store_path } => {
            let store_path = store_path.unwrap_or_else(default_passes_path);
            let mut clipboard: Box<dyn clipboard::ClipboardProvider> = if copy {
                Box::new(clipboard::SystemClipboard::new()?)
            } else {
                Box::new(clipboard::NoopClipboard)
            };
            let command = GuestPassesCommand::new(store_path);
            let result = command.run(copy, clipboard.as_mut(), &telemetry)?;
            println!("Guest passes remaining: {}", result.count);
            println!("Referral link: {}", result.referral_url);
            if result.copied {
                println!("Referral link copied to clipboard.");
            }
        }
        Command::SetupGithubActions {
            repo_root,
            workflow_path,
            secrets,
            no_prompt,
        } => {
            let config = GithubActionsSetupConfig {
                repo_root: repo_root.unwrap_or_else(current_dir),
                workflow_path,
                secret_names: secrets,
                prompt_for_secrets: !no_prompt,
            };
            let result = run_setup(config, &mut prompt, &telemetry)?;
            println!("Workflow written to {}", result.workflow_path.display());
            println!("Add secrets: {}", result.secret_names.join(", "));
        }
        Command::InstallGithubApp {
            repo_root,
            workflow_path,
            client_id,
            client_secret,
            redirect_uri,
            no_prompt,
        } => {
            let mut oauth = OAuthConfig::new(
                client_id.unwrap_or_default(),
                client_secret.unwrap_or_default(),
            );
            if let Some(uri) = redirect_uri {
                oauth.redirect_uri = uri;
            }
            let config = GithubAppInstallConfig {
                repo_root: repo_root.unwrap_or_else(current_dir),
                workflow_path,
                oauth,
                prompt_for_credentials: !no_prompt,
            };
            let oauth_client = commands::github_app::HttpOAuthClient::new();
            let runtime = tokio::runtime::Runtime::new()?;
            let result =
                runtime.block_on(run_wizard(config, &mut prompt, &oauth_client, &telemetry))?;
            println!("Workflow written to {}", result.workflow_path.display());
            println!(
                "Store the access token as secret {}: {}",
                DEFAULT_GITHUB_APP_SECRET, result.access_token
            );
        }
        Command::NativeUpdate {
            target_version,
            download_url,
            install_path,
            data_dir,
        } => {
            let install_path = install_path.unwrap_or_else(current_exe_path);
            let data_dir = data_dir.unwrap_or_else(default_data_dir);
            let config = NativeUpdateConfig {
                target_version,
                download_url,
                install_path,
                data_dir,
            };
            let downloader = ReqwestDownloader::new();
            let runtime = tokio::runtime::Runtime::new()?;
            let result = runtime.block_on(run_native_update(config, &downloader, &telemetry))?;
            println!("Updated binary at {}", result.installed_path.display());
        }
    }

    Ok(())
}

fn current_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn default_data_dir() -> PathBuf {
    dirs::data_dir().unwrap_or_else(current_dir).join("dterm")
}

fn default_passes_path() -> PathBuf {
    default_data_dir().join("guest_passes.toml")
}

fn current_exe_path() -> PathBuf {
    std::env::current_exe().unwrap_or_else(|_| PathBuf::from("dterm"))
}

fn load_input(args: &Args) -> Option<Vec<u8>> {
    if let Some(prompt) = &args.print {
        return Some(prompt.as_bytes().to_vec());
    }

    let path = args.input_file.as_ref()?;
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(err) => {
            eprintln!("Failed to read {}: {}", path.display(), err);
            std::process::exit(1);
        }
    }
}

fn dump_grid(grid: &Grid) {
    for row in 0..grid.rows() {
        let mut line = String::with_capacity(grid.cols() as usize);
        for col in 0..grid.cols() {
            let cell = grid.cell(row, col);
            let ch = match cell {
                Some(cell) if cell.is_wide_continuation() => ' ',
                Some(cell) => cell.char(),
                None => ' ',
            };
            line.push(ch);
        }
        println!("{line}");
    }
}

fn run_interactive(args: Args) {
    // Get terminal size from environment or use defaults
    let (cols, rows) = crossterm::terminal::size().unwrap_or((args.cols, args.rows));

    // Create and run the TUI application
    match tui::App::new(rows, cols) {
        Ok(mut app) => {
            if let Err(e) = app.run() {
                eprintln!("TUI error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            eprintln!("Failed to start TUI: {}", e);
            std::process::exit(1);
        }
    }
}
