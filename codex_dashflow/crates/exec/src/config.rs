//! Configuration for exec mode

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

/// Output mode for exec results
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputMode {
    /// Human-readable output to stdout
    #[default]
    Human,
    /// JSON Lines output for programmatic consumption
    Json,
}

impl fmt::Display for OutputMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputMode::Human => write!(f, "human"),
            OutputMode::Json => write!(f, "json"),
        }
    }
}

impl FromStr for OutputMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "human" => Ok(OutputMode::Human),
            "json" | "jsonl" => Ok(OutputMode::Json),
            _ => Err(format!("Unknown output mode: {}. Use 'human' or 'json'", s)),
        }
    }
}

/// Configuration for non-interactive exec mode
#[derive(Debug, Clone)]
pub struct ExecConfig {
    /// The prompt to execute
    pub prompt: String,

    /// Output mode (human-readable or JSON)
    pub output_mode: OutputMode,

    /// Working directory for file operations
    pub working_dir: PathBuf,

    /// Maximum number of agent turns (0 = unlimited)
    pub max_turns: u32,

    /// Session ID (optional, will be generated if not provided)
    pub session_id: Option<String>,

    /// Use mock LLM for testing
    pub use_mock_llm: bool,

    /// Model to use
    pub model: Option<String>,

    /// Enable verbose output (show tool calls)
    pub verbose: bool,

    /// Enable checkpointing
    pub enable_checkpointing: bool,

    /// Path for checkpoint storage
    pub checkpoint_path: Option<PathBuf>,

    /// Enable training data collection
    pub collect_training: bool,

    /// Enable loading optimized prompts from PromptRegistry
    pub load_optimized_prompts: bool,

    /// Custom system prompt (overrides default and optimized prompts)
    pub system_prompt: Option<String>,

    /// Path to file containing system prompt (--system-prompt takes precedence)
    pub system_prompt_file: Option<PathBuf>,
}

impl ExecConfig {
    /// Create a new exec config with a prompt
    pub fn new(prompt: impl Into<String>) -> Self {
        Self {
            prompt: prompt.into(),
            output_mode: OutputMode::Human,
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            max_turns: 0,
            session_id: None,
            use_mock_llm: false,
            model: None,
            verbose: false,
            enable_checkpointing: false,
            checkpoint_path: None,
            collect_training: false,
            load_optimized_prompts: false,
            system_prompt: None,
            system_prompt_file: None,
        }
    }

    /// Set the output mode
    pub fn with_output_mode(mut self, mode: OutputMode) -> Self {
        self.output_mode = mode;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set maximum turns
    pub fn with_max_turns(mut self, max_turns: u32) -> Self {
        self.max_turns = max_turns;
        self
    }

    /// Set session ID
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Enable or disable mock LLM
    pub fn with_mock_llm(mut self, use_mock: bool) -> Self {
        self.use_mock_llm = use_mock;
        self
    }

    /// Set the model to use
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    /// Enable or disable verbose output
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Enable checkpointing with a file path
    pub fn with_checkpointing(mut self, path: impl Into<PathBuf>) -> Self {
        self.enable_checkpointing = true;
        self.checkpoint_path = Some(path.into());
        self
    }

    /// Enable memory-based checkpointing
    pub fn with_memory_checkpointing(mut self) -> Self {
        self.enable_checkpointing = true;
        self.checkpoint_path = None;
        self
    }

    /// Enable or disable training data collection
    pub fn with_collect_training(mut self, collect: bool) -> Self {
        self.collect_training = collect;
        self
    }

    /// Enable or disable loading optimized prompts from PromptRegistry
    pub fn with_load_optimized_prompts(mut self, load: bool) -> Self {
        self.load_optimized_prompts = load;
        self
    }

    /// Set a custom system prompt (overrides default and optimized prompts)
    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    /// Set a file path containing the system prompt
    /// Note: --system-prompt takes precedence over --system-prompt-file
    pub fn with_system_prompt_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.system_prompt_file = Some(path.into());
        self
    }

    /// Resolve the effective system prompt from config options.
    ///
    /// Precedence (highest to lowest):
    /// 1. system_prompt (direct string)
    /// 2. system_prompt_file (path to file)
    /// 3. None (use default or optimized prompts)
    ///
    /// Returns an error if the file cannot be read.
    pub fn resolve_system_prompt(&self) -> std::io::Result<Option<String>> {
        // Direct system_prompt takes highest precedence
        if let Some(ref prompt) = self.system_prompt {
            return Ok(Some(prompt.clone()));
        }

        // system_prompt_file is second priority
        if let Some(ref path) = self.system_prompt_file {
            let content = std::fs::read_to_string(path)?;
            return Ok(Some(content.trim().to_string()));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_mode_default() {
        let mode: OutputMode = Default::default();
        assert_eq!(mode, OutputMode::Human);
    }

    #[test]
    fn test_output_mode_parse_variants() {
        assert_eq!(OutputMode::from_str("human").unwrap(), OutputMode::Human);
        assert_eq!(OutputMode::from_str("Human").unwrap(), OutputMode::Human);
        assert_eq!(OutputMode::from_str("HUMAN").unwrap(), OutputMode::Human);
        assert_eq!(OutputMode::from_str("json").unwrap(), OutputMode::Json);
        assert_eq!(OutputMode::from_str("jsonl").unwrap(), OutputMode::Json);
        assert_eq!(OutputMode::from_str("JSONL").unwrap(), OutputMode::Json);
    }

    #[test]
    fn test_exec_config_builder() {
        let config = ExecConfig::new("test")
            .with_output_mode(OutputMode::Json)
            .with_working_dir("/tmp")
            .with_max_turns(10)
            .with_session_id("test-session")
            .with_mock_llm(true)
            .with_model("gpt-4")
            .with_verbose(true);

        assert_eq!(config.prompt, "test");
        assert_eq!(config.output_mode, OutputMode::Json);
        assert_eq!(config.working_dir, PathBuf::from("/tmp"));
        assert_eq!(config.max_turns, 10);
        assert_eq!(config.session_id, Some("test-session".to_string()));
        assert!(config.use_mock_llm);
        assert_eq!(config.model, Some("gpt-4".to_string()));
        assert!(config.verbose);
    }

    #[test]
    fn test_exec_config_checkpointing() {
        let config = ExecConfig::new("test").with_checkpointing("/tmp/checkpoints");

        assert!(config.enable_checkpointing);
        assert_eq!(
            config.checkpoint_path,
            Some(PathBuf::from("/tmp/checkpoints"))
        );
    }

    #[test]
    fn test_exec_config_memory_checkpointing() {
        let config = ExecConfig::new("test").with_memory_checkpointing();

        assert!(config.enable_checkpointing);
        assert!(config.checkpoint_path.is_none());
    }

    #[test]
    fn test_exec_config_load_optimized_prompts() {
        // Default is false
        let config = ExecConfig::new("test");
        assert!(!config.load_optimized_prompts);

        // Can enable with builder method
        let config = ExecConfig::new("test").with_load_optimized_prompts(true);
        assert!(config.load_optimized_prompts);

        // Can explicitly disable
        let config = ExecConfig::new("test").with_load_optimized_prompts(false);
        assert!(!config.load_optimized_prompts);
    }

    #[test]
    fn test_exec_config_system_prompt() {
        // Default is None
        let config = ExecConfig::new("test");
        assert!(config.system_prompt.is_none());

        // Can set with builder method
        let config = ExecConfig::new("test").with_system_prompt("You are a helpful assistant.");
        assert_eq!(
            config.system_prompt,
            Some("You are a helpful assistant.".to_string())
        );
    }

    #[test]
    fn test_exec_config_system_prompt_file() {
        // Default is None
        let config = ExecConfig::new("test");
        assert!(config.system_prompt_file.is_none());

        // Can set with builder method
        let config = ExecConfig::new("test").with_system_prompt_file("/path/to/system_prompt.txt");
        assert_eq!(
            config.system_prompt_file,
            Some(PathBuf::from("/path/to/system_prompt.txt"))
        );
    }

    #[test]
    fn test_exec_config_resolve_system_prompt_direct_precedence() {
        // Direct system_prompt takes precedence over system_prompt_file
        let config = ExecConfig::new("test")
            .with_system_prompt("Direct prompt")
            .with_system_prompt_file("/nonexistent/file.txt");

        let result = config.resolve_system_prompt().unwrap();
        assert_eq!(result, Some("Direct prompt".to_string()));
    }

    #[test]
    fn test_exec_config_resolve_system_prompt_from_file() {
        // Create a temporary file with a prompt
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_exec_prompt.txt");
        let prompt_content = "You are a specialized coding assistant.";

        std::fs::write(&temp_file, prompt_content).unwrap();

        let config = ExecConfig::new("test").with_system_prompt_file(&temp_file);
        let result = config.resolve_system_prompt().unwrap();

        assert_eq!(result, Some(prompt_content.to_string()));

        // Cleanup
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_exec_config_resolve_system_prompt_trims_whitespace() {
        // Create a temporary file with whitespace
        let temp_dir = std::env::temp_dir();
        let temp_file = temp_dir.join("test_exec_prompt_ws.txt");
        let prompt_content = "  \n  Trimmed content  \n\n";

        std::fs::write(&temp_file, prompt_content).unwrap();

        let config = ExecConfig::new("test").with_system_prompt_file(&temp_file);
        let result = config.resolve_system_prompt().unwrap();

        assert_eq!(result, Some("Trimmed content".to_string()));

        // Cleanup
        std::fs::remove_file(&temp_file).ok();
    }

    #[test]
    fn test_exec_config_resolve_system_prompt_none_when_neither() {
        let config = ExecConfig::new("test");
        let result = config.resolve_system_prompt().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_exec_config_resolve_system_prompt_file_not_found() {
        let config = ExecConfig::new("test").with_system_prompt_file("/nonexistent/file.txt");
        let result = config.resolve_system_prompt();
        assert!(result.is_err());
    }
}
