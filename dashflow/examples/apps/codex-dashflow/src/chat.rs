//! Interactive chat mode with conversation memory
//!
//! Provides a REPL interface for code-related conversations with memory
//! across multiple turns.

use anyhow::Result;
use dashflow::core::language_models::ChatModel;
use dashflow::core::messages::{AIMessage, HumanMessage, Message};
use dashflow::generate;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::sync::Arc;
use tracing::info;

/// Default system prompt for code assistance
const SYSTEM_PROMPT: &str = r"You are Codex DashFlow, an expert AI programming assistant.

You help with:
- Writing, explaining, and debugging code
- Answering programming questions
- Suggesting refactoring improvements
- Generating tests and documentation

Be concise but thorough. When showing code, use markdown code blocks with the language specified.
If you don't know something, say so honestly.";

/// Configuration for chat mode
#[derive(Default)]
pub struct ChatConfig {
    /// Working directory context (optional)
    pub context_dir: Option<String>,
    /// System prompt override (optional)
    pub system_prompt: Option<String>,
}

/// Conversation history for memory across turns
struct ConversationHistory {
    messages: Vec<Message>,
    system_prompt: String,
}

impl ConversationHistory {
    fn new(system_prompt: &str) -> Self {
        Self {
            messages: Vec::new(),
            system_prompt: system_prompt.to_string(),
        }
    }

    fn add_user_message(&mut self, content: &str) {
        self.messages.push(HumanMessage::new(content).into());
    }

    fn add_assistant_message(&mut self, content: &str) {
        self.messages.push(AIMessage::new(content).into());
    }

    fn clear(&mut self) {
        self.messages.clear();
    }

    fn get_messages(&self) -> Vec<Message> {
        let mut result = vec![Message::system(self.system_prompt.as_str())];
        result.extend(self.messages.clone());
        result
    }

    fn turn_count(&self) -> usize {
        self.messages.len() / 2
    }
}

/// Run interactive chat mode
pub async fn run_chat(model: Arc<dyn ChatModel>, config: ChatConfig) -> Result<()> {
    // Create conversation history with system prompt
    let system_prompt = config
        .system_prompt
        .as_deref()
        .unwrap_or(SYSTEM_PROMPT);
    let mut history = ConversationHistory::new(system_prompt);

    // Print welcome message
    println!();
    println!("Codex DashFlow Chat Mode");
    println!("========================");
    println!("An AI-powered code assistant with conversation memory.");
    println!();
    println!("Commands:");
    println!("  /help   - Show this help message");
    println!("  /clear  - Clear conversation history");
    println!("  /exit   - Exit chat mode (or Ctrl+D)");
    println!();

    // Load context if provided
    if let Some(ref dir) = config.context_dir {
        if Path::new(dir).exists() {
            println!("Context directory: {}", dir);
            info!(context_dir = %dir, "Chat mode started with context");
        } else {
            println!("Warning: Context directory '{}' does not exist", dir);
        }
    }

    println!("Type your message and press Enter to chat.\n");

    // Run REPL
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        print!("You: ");
        stdout.flush()?;

        let mut input = String::new();
        match stdin.lock().read_line(&mut input) {
            Ok(0) => {
                // EOF (Ctrl+D)
                println!("\nGoodbye!");
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("Error reading input: {}", e);
                continue;
            }
        }

        let input = input.trim();

        // Handle empty input
        if input.is_empty() {
            continue;
        }

        // Handle commands
        if input.starts_with('/') {
            match input.to_lowercase().as_str() {
                "/help" => {
                    println!();
                    println!("Commands:");
                    println!("  /help   - Show this help message");
                    println!("  /clear  - Clear conversation history");
                    println!("  /exit   - Exit chat mode");
                    println!();
                    continue;
                }
                "/clear" => {
                    history.clear();
                    println!("Conversation history cleared.\n");
                    info!("Conversation history cleared");
                    continue;
                }
                "/exit" | "/quit" => {
                    println!("Goodbye!");
                    break;
                }
                _ => {
                    println!(
                        "Unknown command: {}. Type /help for available commands.\n",
                        input
                    );
                    continue;
                }
            }
        }

        // Add user message to history
        history.add_user_message(input);

        // Send to LLM and get response
        info!(
            input_len = input.len(),
            turns = history.turn_count(),
            "Processing chat input"
        );

        let messages = history.get_messages();
        match generate(Arc::clone(&model), &messages).await {
            Ok(result) => {
                let response = result
                    .generations
                    .first()
                    .map(|g| g.message.content().as_text())
                    .unwrap_or_default();

                // Add assistant response to history
                history.add_assistant_message(&response);

                println!("\nAssistant: {}\n", response);
                info!(
                    response_len = response.len(),
                    turns = history.turn_count(),
                    "Chat response generated"
                );
            }
            Err(e) => {
                // Remove the failed user message from history
                history.messages.pop();
                eprintln!("\nError: {}\n", e);
                info!(error = %e, "Chat error");
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_config_default() {
        let config = ChatConfig::default();
        assert!(config.context_dir.is_none());
        assert!(config.system_prompt.is_none());
    }

    #[test]
    fn test_conversation_history() {
        let mut history = ConversationHistory::new("Test system prompt");
        assert_eq!(history.turn_count(), 0);
        assert_eq!(history.get_messages().len(), 1); // Just system

        history.add_user_message("Hello");
        assert_eq!(history.turn_count(), 0); // Need both user and assistant

        history.add_assistant_message("Hi there!");
        assert_eq!(history.turn_count(), 1);
        assert_eq!(history.get_messages().len(), 3); // System + user + assistant

        history.clear();
        assert_eq!(history.turn_count(), 0);
        assert_eq!(history.get_messages().len(), 1); // Just system
    }
}
