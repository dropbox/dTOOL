//! Agentic coding assistant
//!
//! This module implements the interactive chat mode using DashFlow's agent framework.
//! It provides a ReAct agent with tools for file operations and shell execution.

pub mod tools;

use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::Message;
use dashflow::core::tools::Tool;
use dashflow::prebuilt::{create_react_agent, AgentState};
use dashflow::stream::{StreamEvent, StreamMode};
use dashflow::CompiledGraph;
use futures::StreamExt;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, info_span};

use self::tools::{EditFileTool, ListFilesTool, ReadFileTool, ShellExecTool, WriteFileTool};
use crate::session::{load_or_create_session, save_session, Session};

/// System prompt for the coding assistant
const CODING_ASSISTANT_PROMPT: &str = r#"You are Codex DashFlow, an AI coding assistant powered by the DashFlow platform.

## Your Role
You help developers with software engineering tasks: writing code, fixing bugs, refactoring, explaining code, running tests, and managing files.

## Tool Usage Guidelines

### read_file
Use to examine file contents before making changes. Always read a file before editing it.
- Input: `{"path": "src/main.rs"}`
- Returns: File contents with line count

### write_file
Use to create new files or completely replace existing file contents.
- Input: `{"path": "src/new_file.rs", "content": "..."}`
- Creates parent directories automatically
- Prefer edit_file for modifying existing files

### edit_file
Use to make targeted changes to existing files. This is preferred over write_file for modifications.
- Input: `{"path": "src/main.rs", "old_text": "original code", "new_text": "replacement code"}`
- The old_text must match exactly and uniquely in the file
- Include enough context to ensure a unique match

### list_files
Use to explore directory structure and find files.
- Input: `{"path": "src", "recursive": true}`
- Use recursive:true to see nested structure
- Skips common non-essential directories (.git, node_modules, target)

### shell_exec
Use to run commands: tests, builds, git operations, installations.
- Input: `{"command": "cargo test", "timeout_secs": 120}`
- Default timeout is 60 seconds
- Output is truncated if too long

## Workflow Best Practices

1. **Understand First**: Read relevant files before proposing changes
2. **Plan Before Acting**: For complex tasks, outline your approach
3. **Make Minimal Changes**: Change only what's necessary
4. **Verify Your Work**: Run tests or builds to confirm changes work
5. **Explain Clearly**: Summarize what you did and why

## Safety Guidelines

- Never modify files without reading them first
- Be careful with destructive operations (file deletion, git force commands)
- Warn before making large-scale changes
- Don't expose secrets or sensitive data in outputs

## Response Format

When completing a task:
1. Briefly state what you did
2. List files modified
3. Note any issues or follow-up needed"#;

/// Default initial state for the coding agent.
#[must_use]
pub fn default_agent_state() -> AgentState {
    AgentState::new(Message::system(CODING_ASSISTANT_PROMPT))
}

/// Create the coding agent with all tools
pub fn create_coding_agent<M>(
    model: M,
    working_dir: Option<&Path>,
) -> Result<CompiledGraph<AgentState>>
where
    M: ChatModel + Clone + 'static,
{
    // Create tools with working directory context
    let cwd = working_dir
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let tools: Vec<Arc<dyn Tool>> = vec![
        Arc::new(ReadFileTool::new(cwd.clone())),
        Arc::new(WriteFileTool::new(cwd.clone())),
        Arc::new(EditFileTool::new(cwd.clone())),
        Arc::new(ListFilesTool::new(cwd.clone())),
        Arc::new(ShellExecTool::new(cwd)),
    ];

    // Create the agent using DashFlow's prebuilt pattern
    create_react_agent(model, tools).map_err(|e| anyhow::anyhow!("Failed to create agent: {}", e))
}

/// Run the interactive chat loop
pub async fn run_chat_loop<M>(model: M, working_dir: Option<&Path>) -> Result<()>
where
    M: ChatModel + Clone + 'static,
{
    run_chat_loop_with_session(model, working_dir, None, false).await
}

/// Run the interactive chat loop with optional session persistence.
pub async fn run_chat_loop_with_session<M>(
    model: M,
    working_dir: Option<&Path>,
    session_path: Option<&Path>,
    resume: bool,
) -> Result<()>
where
    M: ChatModel + Clone + 'static,
{
    run_chat_loop_with_session_impl(model, working_dir, session_path, resume, false).await
}

/// Run the interactive chat loop with streaming output (best-effort).
pub async fn run_chat_loop_with_session_streaming<M>(
    model: M,
    working_dir: Option<&Path>,
    session_path: Option<&Path>,
    resume: bool,
) -> Result<()>
where
    M: ChatModel + Clone + 'static,
{
    run_chat_loop_with_session_impl(model, working_dir, session_path, resume, true).await
}

fn extract_last_assistant_text(state: &AgentState) -> Option<String> {
    state
        .messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Message::AI {
                content, tool_calls, ..
            } => {
                let text = content.as_text();
                // Return the text if it has content OR if there are no tool calls
                // (i.e., only return None if text is empty AND there are tool calls)
                if !text.trim().is_empty() || tool_calls.is_empty() {
                    Some(text)
                } else {
                    None
                }
            }
            _ => None,
        })
}

async fn run_chat_loop_with_session_impl<M>(
    model: M,
    working_dir: Option<&Path>,
    session_path: Option<&Path>,
    resume: bool,
    stream_output: bool,
) -> Result<()>
where
    M: ChatModel + Clone + 'static,
{
    let span = info_span!("codex_chat", working_dir = ?working_dir);
    let _guard = span.enter();

    info!("Starting interactive chat mode");

    // Create the agent
    let agent = create_coding_agent(model, working_dir)?;

    let mut state = default_agent_state();
    let mut session: Option<(std::path::PathBuf, Session)> = match session_path {
        Some(path) => {
            let path = path.to_path_buf();
            let loaded =
                load_or_create_session(&path, resume, default_agent_state(), working_dir).await?;
            state = loaded.state.clone();
            Some((path, loaded))
        }
        None => None,
    };

    println!("\n🤖 Codex DashFlow - AI Coding Assistant (Agentic Mode)");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("This agent can read/write files and execute shell commands.");
    println!("Type your coding questions or tasks. Type 'exit' or 'quit' to end.\n");

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        // Print prompt
        print!("You: ");
        stdout.flush()?;

        // Read user input
        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let input = input.trim();

        // Check for exit commands
        if input.is_empty() {
            continue;
        }
        if matches!(input.to_lowercase().as_str(), "exit" | "quit" | "q") {
            println!("\nGoodbye! 👋");
            if let Some((path, mut sess)) = session.take() {
                sess.update_state(state);
                save_session(&path, &sess).await?;
            }
            break;
        }

        // Add user message to state
        state.messages.push(Message::human(input));

        info!(input_len = input.len(), "User message received");

        // Run the agent
        println!("\n🔄 Thinking...\n");

        let prev_state = state.clone();
        match if stream_output {
            run_agent_streaming(&agent, state, Some("Assistant: "), false).await
        } else {
            agent.invoke(state).await.map(|r| r.final_state).map_err(|e| {
                anyhow::anyhow!("Agent execution failed: {e}")
            })
        } {
            Ok(result) => {
                // Update state with new messages
                state = result;

                // Print the assistant's response (best-effort: last AI message w/ content).
                if !stream_output {
                    if let Some(text) = extract_last_assistant_text(&state) {
                        println!("Assistant: {}\n", text);
                    }
                }

                if let Some((path, sess)) = session.as_mut() {
                    sess.update_state(state.clone());
                    save_session(path, sess).await?;
                }

                // Log execution info
                info!(
                    message_count = state.messages.len(),
                    "Agent execution completed"
                );
            }
            Err(e) => {
                eprintln!("Error: {}\n", e);
                state = prev_state;
                info!(error = %e, "Agent execution failed");
            }
        }
    }

    Ok(())
}

async fn run_agent_streaming(
    agent: &CompiledGraph<AgentState>,
    initial_state: AgentState,
    assistant_prefix: Option<&str>,
    tool_events_to_stderr: bool,
) -> Result<AgentState> {
    let mut stream = Box::pin(agent.stream(initial_state, StreamMode::Custom));
    let mut final_state: Option<AgentState> = None;

    let mut assistant_started = false;
    let mut assistant_line_open = false;

    while let Some(event_result) = stream.next().await {
        match event_result? {
            StreamEvent::Custom { data, .. } => {
                let event_type = data.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match event_type {
                    "llm_delta" => {
                        if let Some(delta) = data.get("delta").and_then(|v| v.as_str()) {
                            if !assistant_started {
                                if let Some(prefix) = assistant_prefix {
                                    print!("{prefix}");
                                }
                                assistant_started = true;
                            }
                            print!("{delta}");
                            assistant_line_open = true;
                            io::stdout().flush()?;
                        }
                    }
                    "tool_call_start" => {
                        if assistant_line_open {
                            println!();
                            assistant_line_open = false;
                        }
                        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                        let args = data
                            .get("args")
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "{}".to_string());
                        if tool_events_to_stderr {
                            eprintln!("tool_call: {name} {args}");
                        } else {
                            println!("🔧 tool_call: {name} {args}");
                        }
                    }
                    "tool_call_end" => {
                        if assistant_line_open {
                            println!();
                            assistant_line_open = false;
                        }
                        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("tool");
                        let status = data
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        if let Some(preview) = data.get("result_preview").and_then(|v| v.as_str()) {
                            if tool_events_to_stderr {
                                eprintln!("tool_result ({status}): {name}: {preview}");
                            } else {
                                println!("🔧 tool_result ({status}): {name}: {preview}");
                            }
                        } else if tool_events_to_stderr {
                            eprintln!("tool_result ({status}): {name}");
                        } else {
                            println!("🔧 tool_result ({status}): {name}");
                        }
                    }
                    _ => {}
                }
            }
            StreamEvent::Done { state, .. } => {
                final_state = Some(state);
                break;
            }
            _ => {}
        }
    }

    if assistant_line_open {
        println!();
    }

    let state = final_state.ok_or_else(|| anyhow::anyhow!("Agent stream ended without Done"))?;

    if !assistant_started {
        // If no streaming deltas were emitted, fall back to printing the final response.
        if let Some(text) = extract_last_assistant_text(&state) {
            if let Some(prefix) = assistant_prefix {
                println!("{prefix}{text}");
            } else {
                println!("{text}");
            }
        }
    }

    Ok(state)
}

/// Run a single query (non-interactive mode)
pub async fn run_single_query<M>(
    model: M,
    query: &str,
    working_dir: Option<&Path>,
) -> Result<String>
where
    M: ChatModel + Clone + 'static,
{
    let (response, _final_state) =
        run_single_query_with_state(model, default_agent_state(), query, working_dir).await?;
    Ok(response)
}

/// Run a single query starting from an existing state (for session resume).
pub async fn run_single_query_with_state<M>(
    model: M,
    mut state: AgentState,
    query: &str,
    working_dir: Option<&Path>,
) -> Result<(String, AgentState)>
where
    M: ChatModel + Clone + 'static,
{
    let span = info_span!("codex_query", query_len = query.len());
    let _guard = span.enter();

    info!("Executing single query");

    // Create the agent
    let agent = create_coding_agent(model, working_dir)?;

    state.messages.push(Message::human(query));

    // Run the agent
    let result = agent
        .invoke(state)
        .await
        .map_err(|e| anyhow::anyhow!("Agent execution failed: {}", e))?;

    // Extract final response
    let final_state = result.final_state;
    let response = extract_last_assistant_text(&final_state)
        .unwrap_or_else(|| "No response generated".to_string());

    info!(response_len = response.len(), "Query completed");

    Ok((response, final_state))
}

/// Run a single query with streaming output (best-effort).
pub async fn run_single_query_streaming_with_state<M>(
    model: M,
    mut state: AgentState,
    query: &str,
    working_dir: Option<&Path>,
) -> Result<(String, AgentState)>
where
    M: ChatModel + Clone + 'static,
{
    let span = info_span!("codex_query_stream", query_len = query.len());
    let _guard = span.enter();

    info!("Executing single query (streaming)");

    let agent = create_coding_agent(model, working_dir)?;
    state.messages.push(Message::human(query));

    let final_state = run_agent_streaming(&agent, state, None, true).await?;
    let response = extract_last_assistant_text(&final_state)
        .unwrap_or_else(|| "No response generated".to_string());

    Ok((response, final_state))
}

/// Run a single query with streaming output (best-effort), returning only the response.
pub async fn run_single_query_streaming<M>(
    model: M,
    query: &str,
    working_dir: Option<&Path>,
) -> Result<String>
where
    M: ChatModel + Clone + 'static,
{
    let (response, _final_state) =
        run_single_query_streaming_with_state(model, default_agent_state(), query, working_dir)
            .await?;
    Ok(response)
}
