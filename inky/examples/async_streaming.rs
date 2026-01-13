//! Async streaming example demonstrating AsyncApp and StreamingText.
#![allow(clippy::future_not_send)] // Example runs on current_thread runtime
//!
//! This example shows how to use inky's async capabilities for:
//! - Token-by-token text streaming (like LLM output)
//! - Concurrent message handling via channels
//! - Integration with tokio async runtime
//!
//! Requires the `async` feature:
//! ```bash
//! cargo run --example async_streaming --features async
//! ```
//!
//! Controls:
//! - Enter: Start/restart streaming
//! - q / Escape: Quit

use inky::components::StreamingText;
use inky::prelude::*;
use std::time::Duration;

/// Messages sent between async tasks and the UI.
#[derive(Clone)]
enum Msg {
    /// A chunk of text to append to the stream.
    StreamChunk(String),
    /// Streaming is complete.
    StreamDone,
}

/// Application state.
struct AppState {
    /// The streaming text component.
    stream: StreamingText,
    /// Whether streaming is currently active.
    is_streaming: bool,
    /// Status message.
    status: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            stream: StreamingText::new().parse_ansi(true).color(Color::White),
            is_streaming: false,
            status: "Press Enter to start streaming".into(),
        }
    }
}

/// Simulated LLM response - chunks of text to stream.
const SIMULATED_RESPONSE: &[&str] = &[
    "Hello! ",
    "I'm ",
    "a ",
    "simulated ",
    "AI ",
    "assistant.\n\n",
    "This ",
    "text ",
    "is ",
    "being ",
    "\x1b[1m", // bold
    "streamed ",
    "\x1b[0m", // reset
    "token-by-token, ",
    "just ",
    "like ",
    "a ",
    "real ",
    "LLM ",
    "response.\n\n",
    "inky ",
    "supports ",
    "\x1b[32m", // green
    "ANSI ",
    "colors ",
    "\x1b[0m",
    "and ",
    "\x1b[4m", // underline
    "styles",
    "\x1b[0m",
    " in ",
    "streaming ",
    "text!\n\n",
    "Features:\n",
    "- ",
    "\x1b[36m", // cyan
    "Thread-safe ",
    "appending\n",
    "\x1b[0m",
    "- ",
    "\x1b[33m", // yellow
    "ANSI ",
    "parsing\n",
    "\x1b[0m",
    "- ",
    "\x1b[35m", // magenta
    "Incremental ",
    "updates\n",
    "\x1b[0m",
];

#[cfg(feature = "async")]
fn main() -> Result<()> {
    // Build the tokio runtime manually to use current_thread flavor
    // (the async feature only enables tokio's "rt" feature, not "rt-multi-thread")
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to create runtime: {}", e))?;

    rt.block_on(async_main())
}

#[cfg(feature = "async")]
async fn async_main() -> Result<()> {
    let app = AsyncApp::new()
        .state(AppState::default())
        .message_type::<Msg>()
        .render(|ctx| {
            let state = ctx.state;

            BoxNode::new()
                .width(ctx.width())
                .height(ctx.height())
                .flex_direction(FlexDirection::Column)
                .child(
                    // Header
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .padding_xy(1.0, 0.0)
                        .border(BorderStyle::Rounded)
                        .child(
                            TextNode::new("Async Streaming Demo")
                                .color(Color::BrightCyan)
                                .bold(),
                        ),
                )
                .child(
                    // Streaming content area
                    BoxNode::new()
                        .flex_grow(1.0)
                        .width(Dimension::Percent(100.0))
                        .padding(1)
                        .border(BorderStyle::Single)
                        .child(state.stream.to_node()),
                )
                .child(
                    // Status bar
                    BoxNode::new()
                        .width(Dimension::Percent(100.0))
                        .padding_xy(1.0, 0.0)
                        .flex_direction(FlexDirection::Row)
                        .child(TextNode::new(&state.status).color(if state.is_streaming {
                            Color::BrightYellow
                        } else {
                            Color::BrightGreen
                        }))
                        .child(Spacer::new())
                        .child(TextNode::new("[Enter] Stream  [q] Quit").color(Color::BrightBlack)),
                )
                .into()
        })
        .on_key(|state, event| {
            match event.code {
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Enter if !state.is_streaming => {
                    // Clear previous content and mark as streaming
                    state.stream.clear();
                    state.is_streaming = true;
                    state.status = "Streaming...".into();
                }
                _ => {}
            }
            false
        })
        .on_message(|state, msg| {
            match msg {
                Msg::StreamChunk(chunk) => {
                    state.stream.append(&chunk);
                }
                Msg::StreamDone => {
                    state.is_streaming = false;
                    state.status = "Done! Press Enter to stream again".into();
                }
            }
            false
        });

    // Get handle for sending messages from async task
    let handle = app.async_handle();

    // Spawn streaming task
    tokio::spawn(async move {
        // Give the UI time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        loop {
            // Wait for streaming to be requested (polling approach for simplicity)
            tokio::time::sleep(Duration::from_millis(50)).await;

            // Simulate streaming response with delays between chunks
            for chunk in SIMULATED_RESPONSE {
                handle.send(Msg::StreamChunk((*chunk).into()));
                // Simulate network/inference delay
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            handle.send(Msg::StreamDone);

            // Wait before allowing another stream (debounce)
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });

    app.run_async().await?;
    Ok(())
}

#[cfg(not(feature = "async"))]
fn main() {
    eprintln!("This example requires the `async` feature.");
    eprintln!("Run with: cargo run --example async_streaming --features async");
    std::process::exit(1);
}
