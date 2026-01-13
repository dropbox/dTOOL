//! Codex DashFlow CLI
//!
//! CLI entry point for the Codex DashFlow agent.

use anyhow::Result;
use clap::Parser;
use colored::Colorize;

/// Pre-main process hardening.
/// This runs before main() to disable core dumps, ptrace, and sanitize environment.
#[ctor::ctor]
fn harden_process() {
    codex_dashflow_process_hardening::pre_main_hardening();
}
use codex_dashflow_cli::{
    build_agent_state, build_exec_approval_callback, build_mcp_client,
    build_runner_config_full_async, load_config, print_dry_run_config, print_exec_security_posture,
    read_prompt_from_stdin, resolve_config, resolve_session_id_with_max_age, resolve_system_prompt,
    run_capabilities_command, run_check_command, run_completions_command, run_doctor_command,
    run_features_command, run_init_command, run_introspect_command, run_login_command,
    run_logout_command, run_mcp_server_command, run_optimize_command, run_sessions_command,
    run_version_command, validate_prompt, Args, CheckExitCode, Command, DoctorExitCode,
    InitExitCode, LoginExitCode, LogoutExitCode, SessionsExitCode,
};
use codex_dashflow_core::{can_resume_session, resume_session, run_agent, Message};
use codex_dashflow_tui::{run_app, AppConfig};

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Handle subcommands first
    if let Some(command) = &args.command {
        return match command {
            Command::Optimize(optimize_args) => run_optimize_command(optimize_args),
            Command::McpServer(mcp_args) => run_mcp_server_command(mcp_args).await,
            Command::Completions(completions_args) => {
                run_completions_command(completions_args);
                Ok(())
            }
            Command::Version(version_args) => {
                let exit_code = run_version_command(version_args);
                if exit_code != 0 {
                    std::process::exit(exit_code);
                }
                Ok(())
            }
            Command::Doctor(doctor_args) => {
                let file_config = load_config(&args);
                let exit_code = run_doctor_command(doctor_args, &file_config).await;
                if exit_code != DoctorExitCode::Ok {
                    std::process::exit(exit_code.code());
                }
                Ok(())
            }
            Command::Init(init_args) => {
                let exit_code = run_init_command(init_args);
                if exit_code != InitExitCode::Success {
                    std::process::exit(exit_code.code());
                }
                Ok(())
            }
            Command::Login(login_args) => {
                let exit_code = run_login_command(login_args);
                if exit_code != LoginExitCode::Success {
                    std::process::exit(exit_code.code());
                }
                Ok(())
            }
            Command::Logout(logout_args) => {
                let exit_code = run_logout_command(logout_args);
                if exit_code != LogoutExitCode::Success {
                    std::process::exit(exit_code.code());
                }
                Ok(())
            }
            Command::Introspect(introspect_args) => {
                let exit_code = run_introspect_command(introspect_args);
                if exit_code != 0 {
                    std::process::exit(exit_code);
                }
                Ok(())
            }
            Command::Sessions(sessions_args) => {
                let file_config = load_config(&args);
                let exit_code = run_sessions_command(sessions_args, &file_config).await;
                if exit_code != SessionsExitCode::Success {
                    std::process::exit(exit_code.code());
                }
                Ok(())
            }
            Command::Capabilities(capabilities_args) => {
                let exit_code = run_capabilities_command(capabilities_args);
                if exit_code != 0 {
                    std::process::exit(exit_code);
                }
                Ok(())
            }
            Command::Features(features_args) => {
                let exit_code = run_features_command(features_args);
                if exit_code != 0 {
                    std::process::exit(exit_code);
                }
                Ok(())
            }
        };
    }

    // Audit #25: Early guard when --dashstream used without compiled feature
    // User should learn about this immediately, not at runtime
    #[cfg(not(feature = "dashstream"))]
    if args.dashstream {
        eprintln!(
            "{}",
            "Error: --dashstream flag used but 'dashstream' feature is not compiled.".red()
        );
        eprintln!(
            "{}",
            "Rebuild with: cargo build --features dashstream (requires protoc)".yellow()
        );
        std::process::exit(1);
    }

    // Load and resolve configuration for agent mode
    let file_config = load_config(&args);
    let resolved = resolve_config(&args, &file_config);

    // Handle --check: validate configuration and exit
    if resolved.check {
        let exit_code = run_check_command(
            &args,
            &file_config,
            args.config.as_deref(),
            resolved.quiet,
            resolved.json,
        );
        if exit_code != CheckExitCode::Valid {
            std::process::exit(exit_code.code());
        }
        return Ok(());
    }

    // Handle --dry-run: print resolved config and exit
    if resolved.dry_run {
        print_dry_run_config(&resolved, args.config.as_deref(), resolved.json);
        return Ok(());
    }

    // Determine the prompt: --stdin > --prompt-file > --exec
    let prompt = if resolved.stdin {
        // Read from stdin (highest priority for prompt)
        Some(
            read_prompt_from_stdin()
                .map_err(|e| anyhow::anyhow!("Failed to read prompt from stdin: {}", e))?,
        )
    } else if let Some(ref path) = resolved.prompt_file {
        // Read from prompt file (second priority)
        let content = std::fs::read_to_string(path).map_err(|e| {
            anyhow::anyhow!("Failed to read prompt file '{}': {}", path.display(), e)
        })?;
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Err(anyhow::anyhow!("Prompt file '{}' is empty", path.display()));
        }
        Some(trimmed.to_string())
    } else {
        // Use --exec prompt (lowest priority)
        resolved.exec_prompt.clone()
    };

    // Validate prompt if provided
    if let Some(ref p) = prompt {
        validate_prompt(p).map_err(|e| anyhow::anyhow!("Invalid prompt: {}", e))?;
    }

    if let Some(prompt) = &prompt {
        // Non-interactive (exec) mode
        // Print security posture banner to stderr (audit item #11)
        // This is displayed before tracing initialization for immediate visibility
        if !resolved.quiet {
            print_exec_security_posture(&resolved);

            // Initialize tracing after security banner
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::from_default_env()
                        .add_directive(tracing::Level::INFO.into()),
                )
                .with_writer(std::io::stderr)
                .init();

            // Also log to tracing for structured logging (audit item #97)
            let mcp_status = if resolved.mcp_servers.is_empty() {
                "none".to_string()
            } else {
                format!("{} server(s)", resolved.mcp_servers.len())
            };

            tracing::info!(
                prompt = %prompt,
                working_dir = %resolved.working_dir,
                max_turns = resolved.max_turns,
                mock = resolved.use_mock_llm,
                model = %resolved.model,
                sandbox = ?resolved.sandbox_mode,
                approval_mode = ?resolved.approval_mode,
                mcp = %mcp_status,
                "Running in exec mode"
            );
        }

        // Resolve system prompt (--system-prompt takes precedence over --system-prompt-file)
        let system_prompt = resolve_system_prompt(
            resolved.system_prompt.as_deref(),
            resolved.system_prompt_file.as_ref(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to read system prompt file: {}", e))?;

        // Build agent state
        let mut state = build_agent_state(&resolved);

        // Build and connect MCP client if configured (audit items #17, #22, #90)
        if let Some(mcp_client) = build_mcp_client(&resolved).await {
            state = state.with_mcp_client(mcp_client);
        }

        // Set approval callback based on approval mode (audit items #18, #61)
        // In exec mode, we can't prompt for approval, so use appropriate callback
        let approval_callback = build_exec_approval_callback(resolved.approval_mode);
        state = state.with_approval_callback(approval_callback);

        // Create runner config with streaming callback, training collection, and optimized prompts
        // --quiet disables verbose output and console streaming (quiet takes precedence)
        // Passes dashstream config for audit item #13 wiring
        // Uses checkpointing config for audit items #14, #15
        // Passes working_dir for project doc discovery (audit items #20, #23)
        // Uses async builder to support DashFlowStreamAdapter initialization
        let effective_verbose = resolved.verbose && !resolved.quiet;
        // --quiet also suppresses console streaming to stderr
        let effective_streaming = resolved.streaming_enabled && !resolved.quiet;
        let runner_config = build_runner_config_full_async(
            effective_verbose,
            resolved.collect_training,
            resolved.load_optimized_prompts,
            system_prompt,
            resolved.postgres.clone(),
            resolved.dashstream.as_ref(),
            resolved.checkpointing_enabled,
            resolved.checkpoint_path.as_ref(),
            Some(&resolved.working_dir),
            resolved.session_id.as_deref(),
            effective_streaming, // Audit item #14: Config-driven streaming, suppressed by --quiet
        )
        .await;

        // Session resume logic (audit item #19)
        // Determine session ID to use for exec mode
        // Priority: explicit --session > auto_resume (if enabled) > None (fresh session)
        let (session_to_resolve, max_age_for_resolve) = if resolved.session_id.is_some() {
            // User explicitly passed --session - no max age filtering
            (resolved.session_id.as_deref(), None)
        } else if resolved.auto_resume_enabled && resolved.checkpointing_enabled {
            // Auto-resume enabled: try to get latest session, respecting max age
            (Some("latest"), resolved.auto_resume_max_age_secs)
        } else {
            (None, None)
        };

        let resolved_session_id = resolve_session_id_with_max_age(
            session_to_resolve,
            &runner_config,
            max_age_for_resolve,
        )
        .await;

        // Warn user if auto-resume was enabled but no sessions exist
        let auto_resume_attempted = resolved.auto_resume_enabled
            && resolved.checkpointing_enabled
            && resolved.session_id.is_none();
        if auto_resume_attempted && resolved_session_id.is_none() && !resolved.quiet {
            eprintln!(
                "{}",
                "Note: auto_resume enabled but no sessions found. Starting fresh session.".yellow()
            );
        }

        // Check if --session was provided and a checkpoint exists to resume
        if let Some(ref session_id) = resolved_session_id {
            if can_resume_session(session_id, &runner_config).await {
                if !resolved.quiet {
                    tracing::info!(
                        session_id = %session_id,
                        "Resuming existing session from checkpoint"
                    );
                }
                match resume_session(session_id, &runner_config).await {
                    Ok(mut resumed_state) => {
                        // Transfer new config to resumed state (sandbox, working_dir, etc.)
                        // The resumed state from checkpoint doesn't have runtime fields (MCP client,
                        // approval callback) so we apply them from the freshly built state
                        resumed_state.sandbox_mode = state.sandbox_mode;
                        resumed_state.working_directory = state.working_directory.clone();
                        if let Some(mcp) = state.mcp_client() {
                            resumed_state = resumed_state.with_mcp_client(mcp);
                        }
                        // Apply approval callback from freshly configured state
                        let approval = build_exec_approval_callback(resolved.approval_mode);
                        resumed_state = resumed_state.with_approval_callback(approval);

                        // Audit #31: Refresh system prompt if configured
                        // If the user has load_optimized_prompts or a custom system_prompt,
                        // update the resumed state with the current config's prompt
                        if state.system_prompt.is_some() {
                            resumed_state.system_prompt = state.system_prompt.clone();
                        }

                        // Add the new user message to the resumed session
                        resumed_state.messages.push(Message::user(prompt));
                        state = resumed_state;
                    }
                    Err(e) => {
                        if !resolved.quiet {
                            tracing::warn!(
                                error = %e,
                                session_id = %session_id,
                                "Failed to resume session, starting fresh"
                            );
                        }
                        // Fall through to fresh session
                        state.messages.push(Message::user(prompt));
                    }
                }
            } else {
                // No checkpoint exists for this session ID - start fresh
                if !resolved.quiet {
                    tracing::info!(
                        session_id = %session_id,
                        "Starting new session (no checkpoint found)"
                    );
                }
                state.messages.push(Message::user(prompt));
            }
        } else {
            // No session ID provided, start fresh
            state.messages.push(Message::user(prompt));
        }

        // Run the agent
        match run_agent(state, &runner_config).await {
            Ok(result) => {
                // Print the agent's response to stdout
                if let Some(response) = result.state.last_response {
                    println!("{}", response);
                } else if !resolved.quiet {
                    eprintln!("No response from agent");
                }
            }
            Err(e) => {
                // Always output errors, even in quiet mode
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    } else {
        // Interactive TUI mode

        // Resolve system prompt (--system-prompt takes precedence over --system-prompt-file)
        let system_prompt = resolve_system_prompt(
            resolved.system_prompt.as_deref(),
            resolved.system_prompt_file.as_ref(),
        )
        .map_err(|e| anyhow::anyhow!("Failed to read system prompt file: {}", e))?;

        // Build runner config to resolve "latest" session ID
        // Uses the same checkpointing config that will be passed to TUI
        let tui_runner_config = if let Some(ref conn_str) = resolved.postgres {
            codex_dashflow_core::RunnerConfig::with_postgres_checkpointing(conn_str)
        } else if let Some(ref path) = resolved.checkpoint_path {
            codex_dashflow_core::RunnerConfig::with_file_checkpointing(path)
        } else if resolved.checkpointing_enabled {
            codex_dashflow_core::RunnerConfig::with_memory_checkpointing()
        } else {
            codex_dashflow_core::RunnerConfig::default()
        };

        // Determine session ID to use for TUI
        // Priority: explicit --session > auto_resume (if enabled) > None (fresh session)
        let (session_to_resolve, max_age_for_resolve) = if resolved.session_id.is_some() {
            // User explicitly passed --session - no max age filtering
            (resolved.session_id.as_deref(), None)
        } else if resolved.auto_resume_enabled && resolved.checkpointing_enabled {
            // Auto-resume enabled: try to get latest session, respecting max age
            (Some("latest"), resolved.auto_resume_max_age_secs)
        } else {
            (None, None)
        };

        // Resolve "latest" to actual session ID if checkpointing is configured
        let tui_session_id = resolve_session_id_with_max_age(
            session_to_resolve,
            &tui_runner_config,
            max_age_for_resolve,
        )
        .await;

        // Track if auto-resume was attempted but no sessions exist
        let auto_resume_attempted = resolved.auto_resume_enabled
            && resolved.checkpointing_enabled
            && resolved.session_id.is_none();
        let auto_resume_no_sessions = auto_resume_attempted && tui_session_id.is_none();
        // Track if auto-resume was successful (session was auto-resumed, not explicitly requested)
        let auto_resumed_session = auto_resume_attempted && tui_session_id.is_some();

        let app_config = AppConfig {
            session_id: tui_session_id,
            working_dir: resolved.working_dir,
            max_turns: resolved.max_turns,
            model: resolved.model,
            use_mock_llm: resolved.use_mock_llm,
            collect_training: resolved.collect_training,
            load_optimized_prompts: resolved.load_optimized_prompts,
            system_prompt,
            checkpointing_enabled: resolved.checkpointing_enabled,
            checkpoint_path: resolved.checkpoint_path,
            postgres_connection_string: resolved.postgres,
            auto_resume_no_sessions,
            auto_resumed_session,
            ..Default::default()
        };

        // Run the TUI
        run_app(app_config).await?;
    }

    Ok(())
}
