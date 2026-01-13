// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Unified timeline commands for graph execution observation.
//!
//! M-38: Unifies `watch`, `replay`, and `visualize` commands under a single
//! "graph timeline" model with consistent flags and concepts.
//!
//! Subcommands:
//! - `live`: Watch live execution (equivalent to `watch`)
//! - `replay`: Replay historical execution (equivalent to `replay`)
//! - `view`: View static graph visualization (equivalent to `visualize view`)
//! - `export`: Export visualization to HTML (equivalent to `visualize export`)
//!
//! See `docs/design/CLI_TIMELINE_UX.md` for the full design spec.

use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

use super::{replay, visualize, watch};

/// Unified timeline interface for graph execution observation
///
/// This command provides a unified interface for:
/// - Watching live graph execution
/// - Replaying historical executions
/// - Viewing static graph visualizations
///
/// All subcommands share consistent flag naming (e.g., `--thread` not `--thread-id`).
#[derive(Args)]
pub struct TimelineArgs {
    #[command(subcommand)]
    pub command: TimelineCommand,
}

#[derive(Subcommand)]
pub enum TimelineCommand {
    /// Watch live graph execution (real-time TUI visualization)
    ///
    /// Streams events from Kafka and displays them in an interactive terminal UI.
    /// Use `--thread` to filter by a specific execution thread.
    Live(LiveArgs),

    /// Replay historical execution (time-travel debugging)
    ///
    /// Replays events from Kafka for a specific thread, with optional time filtering.
    /// Supports playback speed control and pausing on errors.
    Replay(ReplayArgs),

    /// View a static graph visualization in an interactive web UI
    ///
    /// Opens a Mermaid diagram or JSON graph file in a browser-based viewer.
    View(ViewArgs),

    /// Export a graph visualization to a standalone HTML file
    ///
    /// Creates a self-contained HTML file that can be shared and viewed without a server.
    Export(ExportArgs),
}

/// Arguments for `dashflow timeline live`
#[derive(Args)]
pub struct LiveArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    pub bootstrap_servers: String,

    /// Kafka topic to consume from
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    pub topic: String,

    /// Filter by thread ID (consistent with replay command)
    #[arg(long)]
    pub thread: Option<String>,

    /// Start from beginning of topic
    #[arg(short, long)]
    pub from_beginning: bool,

    /// Refresh rate in milliseconds
    #[arg(short, long, default_value = "100")]
    pub refresh_ms: u64,
}

/// Arguments for `dashflow timeline replay`
#[derive(Args)]
pub struct ReplayArgs {
    /// Kafka bootstrap servers (comma-separated)
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    pub bootstrap_servers: String,

    /// Kafka topic to consume from
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    pub topic: String,

    /// Thread ID to replay (unified flag name across all timeline commands)
    #[arg(long, required = true)]
    pub thread: String,

    /// Start from timestamp (RFC3339 format or Unix micros)
    #[arg(long)]
    pub from_timestamp: Option<String>,

    /// Stop at timestamp (RFC3339 format or Unix micros)
    #[arg(long)]
    pub to_timestamp: Option<String>,

    /// Start from checkpoint ID
    #[arg(long)]
    pub from_checkpoint: Option<String>,

    /// Playback speed multiplier (1.0 = real-time, 2.0 = 2x speed, 0 = instant)
    #[arg(long, default_value = "0")]
    pub speed: f64,

    /// Show only specific event types (comma-separated)
    #[arg(long)]
    pub events: Option<String>,

    /// Pause on errors
    #[arg(long)]
    pub pause_on_error: bool,
}

/// Arguments for `dashflow timeline view`
#[derive(Args)]
pub struct ViewArgs {
    /// Path to Mermaid (.mmd) or JSON graph file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Open browser automatically (use --no-open to disable)
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub open: bool,

    /// Port to serve on
    #[arg(short, long, default_value = "8765")]
    pub port: u16,
}

/// Arguments for `dashflow timeline export`
#[derive(Args)]
pub struct ExportArgs {
    /// Path to Mermaid (.mmd) or JSON graph file
    #[arg(required = true)]
    pub input: PathBuf,

    /// Output HTML file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Include dark mode support (use --no-dark-mode to disable)
    #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
    pub dark_mode: bool,
}

/// Run the timeline command
pub async fn run(args: TimelineArgs) -> Result<()> {
    match args.command {
        TimelineCommand::Live(live_args) => run_live(live_args).await,
        TimelineCommand::Replay(replay_args) => run_replay(replay_args).await,
        TimelineCommand::View(view_args) => run_view(view_args).await,
        TimelineCommand::Export(export_args) => run_export(export_args).await,
    }
}

/// Run live monitoring (delegates to watch)
async fn run_live(args: LiveArgs) -> Result<()> {
    // Convert to watch args
    let watch_args = watch::WatchArgs {
        bootstrap_servers: args.bootstrap_servers,
        topic: args.topic,
        thread: args.thread,
        from_beginning: args.from_beginning,
        refresh_ms: args.refresh_ms,
    };
    watch::run(watch_args).await
}

/// Run replay (delegates to replay)
async fn run_replay(args: ReplayArgs) -> Result<()> {
    // Convert to replay args (using --thread instead of --thread-id)
    let replay_args = replay::ReplayArgs {
        bootstrap_servers: args.bootstrap_servers,
        topic: args.topic,
        thread_id: args.thread, // Map unified --thread to internal thread_id
        from_timestamp: args.from_timestamp,
        to_timestamp: args.to_timestamp,
        from_checkpoint: args.from_checkpoint,
        speed: args.speed,
        events: args.events,
        pause_on_error: args.pause_on_error,
    };
    replay::run(replay_args).await
}

/// Run view (delegates to visualize view)
async fn run_view(args: ViewArgs) -> Result<()> {
    let visualize_args = visualize::VisualizeArgs {
        command: visualize::VisualizeCommand::View(visualize::ViewArgs {
            input: args.input,
            open: args.open,
            port: args.port,
        }),
    };
    visualize::run(visualize_args).await
}

/// Run export (delegates to visualize export)
async fn run_export(args: ExportArgs) -> Result<()> {
    let visualize_args = visualize::VisualizeArgs {
        command: visualize::VisualizeCommand::Export(visualize::ExportArgs {
            input: args.input,
            output: args.output,
            dark_mode: args.dark_mode,
        }),
    };
    visualize::run(visualize_args).await
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        timeline: TimelineArgs,
    }

    #[test]
    fn test_timeline_live_defaults() {
        let cli = Cli::parse_from(["test", "live"]);

        if let TimelineCommand::Live(args) = cli.timeline.command {
            assert_eq!(args.bootstrap_servers, "localhost:9092");
            assert_eq!(args.topic, "dashstream-events");
            assert!(args.thread.is_none());
            assert!(!args.from_beginning);
            assert_eq!(args.refresh_ms, 100);
        } else {
            panic!("Expected Live command");
        }
    }

    #[test]
    fn test_timeline_live_with_thread() {
        let cli = Cli::parse_from(["test", "live", "--thread", "my-thread"]);

        if let TimelineCommand::Live(args) = cli.timeline.command {
            assert_eq!(args.thread, Some("my-thread".to_string()));
        } else {
            panic!("Expected Live command");
        }
    }

    #[test]
    fn test_timeline_replay_requires_thread() {
        // Should fail without --thread
        let result = Cli::try_parse_from(["test", "replay"]);
        assert!(result.is_err());

        // Should succeed with --thread
        let cli = Cli::parse_from(["test", "replay", "--thread", "thread-123"]);

        if let TimelineCommand::Replay(args) = cli.timeline.command {
            assert_eq!(args.thread, "thread-123");
        } else {
            panic!("Expected Replay command");
        }
    }

    #[test]
    fn test_timeline_replay_full_options() {
        let cli = Cli::parse_from([
            "test",
            "replay",
            "--thread",
            "t1",
            "--from-timestamp",
            "2025-01-01T00:00:00Z",
            "--to-timestamp",
            "2025-01-01T01:00:00Z",
            "--speed",
            "2.0",
            "--events",
            "NODE_START,NODE_END",
            "--pause-on-error",
        ]);

        if let TimelineCommand::Replay(args) = cli.timeline.command {
            assert_eq!(args.thread, "t1");
            assert_eq!(
                args.from_timestamp,
                Some("2025-01-01T00:00:00Z".to_string())
            );
            assert_eq!(args.to_timestamp, Some("2025-01-01T01:00:00Z".to_string()));
            assert!((args.speed - 2.0).abs() < f64::EPSILON);
            assert_eq!(args.events, Some("NODE_START,NODE_END".to_string()));
            assert!(args.pause_on_error);
        } else {
            panic!("Expected Replay command");
        }
    }

    #[test]
    fn test_timeline_view_requires_input() {
        // Should fail without input
        let result = Cli::try_parse_from(["test", "view"]);
        assert!(result.is_err());

        // Should succeed with input
        let cli = Cli::parse_from(["test", "view", "graph.mmd"]);

        if let TimelineCommand::View(args) = cli.timeline.command {
            assert_eq!(args.input, PathBuf::from("graph.mmd"));
            assert!(args.open); // default
            assert_eq!(args.port, 8765); // default
        } else {
            panic!("Expected View command");
        }
    }

    #[test]
    fn test_timeline_view_with_options() {
        let cli = Cli::parse_from([
            "test",
            "view",
            "graph.json",
            "--open=false",
            "--port",
            "9000",
        ]);

        if let TimelineCommand::View(args) = cli.timeline.command {
            assert_eq!(args.input, PathBuf::from("graph.json"));
            assert!(!args.open);
            assert_eq!(args.port, 9000);
        } else {
            panic!("Expected View command");
        }
    }

    #[test]
    fn test_timeline_export_requires_input() {
        // Should fail without input
        let result = Cli::try_parse_from(["test", "export"]);
        assert!(result.is_err());

        // Should succeed with input
        let cli = Cli::parse_from(["test", "export", "graph.mmd"]);

        if let TimelineCommand::Export(args) = cli.timeline.command {
            assert_eq!(args.input, PathBuf::from("graph.mmd"));
            assert!(args.output.is_none());
            assert!(args.dark_mode); // default
        } else {
            panic!("Expected Export command");
        }
    }

    #[test]
    fn test_timeline_export_with_options() {
        let cli = Cli::parse_from([
            "test",
            "export",
            "graph.mmd",
            "--output",
            "output.html",
            "--dark-mode=false",
        ]);

        if let TimelineCommand::Export(args) = cli.timeline.command {
            assert_eq!(args.input, PathBuf::from("graph.mmd"));
            assert_eq!(args.output, Some(PathBuf::from("output.html")));
            assert!(!args.dark_mode);
        } else {
            panic!("Expected Export command");
        }
    }

    #[test]
    fn test_timeline_unified_thread_flag() {
        // Both live and replay use --thread (not --thread-id)
        let live = Cli::parse_from(["test", "live", "--thread", "my-thread"]);
        let replay = Cli::parse_from(["test", "replay", "--thread", "my-thread"]);

        if let TimelineCommand::Live(args) = live.timeline.command {
            assert_eq!(args.thread, Some("my-thread".to_string()));
        } else {
            panic!("Expected Live command");
        }

        if let TimelineCommand::Replay(args) = replay.timeline.command {
            assert_eq!(args.thread, "my-thread");
        } else {
            panic!("Expected Replay command");
        }
    }
}
