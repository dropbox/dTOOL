//! Domain abstraction for terminal connections.
//!
//! A Domain represents a context for spawning terminal panes. Different domains
//! provide different connection types:
//!
//! - **Local**: Spawns processes on the local machine via PTY
//! - **SSH**: Connects to remote machines via SSH protocol
//! - **WSL**: Connects to Windows Subsystem for Linux instances
//! - **Serial**: Connects to serial port devices
//! - **Mux**: Connects to a remote multiplexer server
//!
//! ## Design
//!
//! Based on WezTerm's domain architecture (`mux/src/domain.rs`), with adaptations
//! for dterm's async-agnostic core library design.
//!
//! ```text
//! ┌──────────────────────────────────────────────────────────────┐
//! │                      Domain Trait                             │
//! │  spawn() → Pane                                               │
//! │  attach() / detach()                                          │
//! │  state() → DomainState                                        │
//! └──────────────────────────────────────────────────────────────┘
//!         ▲           ▲           ▲           ▲
//!         │           │           │           │
//! ┌───────┴───┐ ┌─────┴─────┐ ┌───┴────┐ ┌────┴────┐
//! │   Local   │ │    SSH    │ │  WSL   │ │ Serial  │
//! │  Domain   │ │  Domain   │ │ Domain │ │ Domain  │
//! └───────────┘ └───────────┘ └────────┘ └─────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use dterm_core::domain::{Domain, LocalDomain, DomainState};
//!
//! // Create a local domain
//! let domain = LocalDomain::new("local");
//!
//! // Spawn a pane
//! let config = SpawnConfig::default();
//! let pane = domain.spawn_pane(80, 24, config)?;
//!
//! // Read/write to the pane
//! pane.write(b"echo hello\n")?;
//! let output = pane.read()?;
//! ```
//!
//! ## References
//!
//! - WezTerm: `mux/src/domain.rs`, `wezterm-ssh/`
//! - Ghostty: Connection abstraction
//! - Alacritty: PTY abstraction

use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Global domain ID counter.
static DOMAIN_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DomainId(u64);

impl DomainId {
    /// Allocate a new unique domain ID.
    #[must_use]
    pub fn new() -> Self {
        Self(DOMAIN_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    #[must_use]
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for DomainId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for DomainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "domain:{}", self.0)
    }
}

/// Global pane ID counter.
static PANE_ID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Unique identifier for a pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PaneId(u64);

impl PaneId {
    /// Allocate a new unique pane ID.
    #[must_use]
    pub fn new() -> Self {
        Self(PANE_ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    #[must_use]
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl Default for PaneId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "pane:{}", self.0)
    }
}

/// Connection state of a domain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainState {
    /// Domain is not connected.
    Detached,
    /// Domain is connected and ready to spawn panes.
    Attached,
    /// Domain is in the process of connecting.
    Connecting,
    /// Domain connection failed.
    Failed,
}

impl Default for DomainState {
    fn default() -> Self {
        Self::Detached
    }
}

/// Type of domain connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainType {
    /// Local PTY connection.
    Local,
    /// SSH remote connection.
    Ssh,
    /// Windows Subsystem for Linux.
    Wsl,
    /// Serial port connection.
    Serial,
    /// Remote multiplexer connection.
    Mux,
    /// Custom/plugin domain.
    Custom,
}

/// Configuration for spawning a new pane.
#[derive(Debug, Clone, Default)]
pub struct SpawnConfig {
    /// Command to run (None = default shell).
    pub command: Option<String>,
    /// Arguments to the command.
    pub args: Vec<String>,
    /// Working directory.
    pub cwd: Option<PathBuf>,
    /// Environment variables to set.
    pub env: HashMap<String, String>,
    /// Environment variables to clear.
    pub env_remove: Vec<String>,
}

impl SpawnConfig {
    /// Create a new spawn config with default shell.
    #[must_use]
    pub fn default_shell() -> Self {
        Self::default()
    }

    /// Create a spawn config for a specific command.
    #[must_use]
    pub fn command(cmd: impl Into<String>) -> Self {
        Self {
            command: Some(cmd.into()),
            ..Default::default()
        }
    }

    /// Set the working directory.
    #[must_use]
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Add an argument.
    #[must_use]
    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Add multiple arguments.
    #[must_use]
    pub fn with_args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    /// Set an environment variable.
    #[must_use]
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }
}

/// Result type for domain operations.
pub type DomainResult<T> = Result<T, DomainError>;

/// Errors that can occur in domain operations.
#[derive(Debug)]
pub enum DomainError {
    /// Domain is not connected.
    NotAttached,
    /// Connection failed.
    ConnectionFailed(String),
    /// Spawn failed.
    SpawnFailed(String),
    /// I/O error.
    Io(std::io::Error),
    /// Pane not found.
    PaneNotFound(PaneId),
    /// Domain not found.
    DomainNotFound(DomainId),
    /// Operation not supported.
    NotSupported(String),
    /// Authentication failed.
    AuthenticationFailed(String),
    /// Timeout.
    Timeout,
    /// Other error.
    Other(String),
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAttached => write!(f, "domain is not attached"),
            Self::ConnectionFailed(msg) => write!(f, "connection failed: {msg}"),
            Self::SpawnFailed(msg) => write!(f, "spawn failed: {msg}"),
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::PaneNotFound(id) => write!(f, "pane not found: {:?}", id.raw()),
            Self::DomainNotFound(id) => write!(f, "domain not found: {:?}", id.raw()),
            Self::NotSupported(op) => write!(f, "operation not supported: {op}"),
            Self::AuthenticationFailed(msg) => write!(f, "authentication failed: {msg}"),
            Self::Timeout => write!(f, "operation timed out"),
            Self::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for DomainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for DomainError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

/// A pane represents an active terminal session within a domain.
///
/// Panes are the fundamental unit of terminal interaction. They handle:
/// - Input/output to the underlying process or connection
/// - Terminal size (rows/columns)
/// - Process lifecycle (running/exited)
pub trait Pane: Send + Sync {
    /// Get the unique pane ID.
    fn pane_id(&self) -> PaneId;

    /// Get the domain ID this pane belongs to.
    fn domain_id(&self) -> DomainId;

    /// Get the current terminal size.
    fn size(&self) -> (u16, u16); // (cols, rows)

    /// Resize the terminal.
    fn resize(&self, cols: u16, rows: u16) -> DomainResult<()>;

    /// Write data to the pane (input to the process).
    fn write(&self, data: &[u8]) -> DomainResult<usize>;

    /// Read available data from the pane (output from the process).
    ///
    /// Returns the data read, or an empty slice if no data is available.
    /// This is non-blocking.
    fn read(&self, buf: &mut [u8]) -> DomainResult<usize>;

    /// Check if the pane process is still running.
    fn is_alive(&self) -> bool;

    /// Get the exit status if the process has exited.
    fn exit_status(&self) -> Option<i32>;

    /// Kill the pane process.
    fn kill(&self) -> DomainResult<()>;

    /// Get the process ID (if applicable).
    fn pid(&self) -> Option<u32> {
        None
    }

    /// Get the pane title.
    fn title(&self) -> String {
        String::new()
    }

    /// Get the current working directory (if known).
    fn cwd(&self) -> Option<PathBuf> {
        None
    }

    /// Get the foreground process name (if known).
    fn foreground_process_name(&self) -> Option<String> {
        None
    }
}

/// A domain represents a context for spawning terminal panes.
///
/// Domains abstract over different connection types (local, SSH, WSL, etc.)
/// providing a uniform interface for creating and managing terminal sessions.
pub trait Domain: Send + Sync {
    /// Get the unique domain ID.
    fn domain_id(&self) -> DomainId;

    /// Get the domain name (short identifier).
    fn domain_name(&self) -> &str;

    /// Get a human-readable label for the domain.
    fn domain_label(&self) -> String {
        self.domain_name().to_string()
    }

    /// Get the domain type.
    fn domain_type(&self) -> DomainType;

    /// Get the current connection state.
    fn state(&self) -> DomainState;

    /// Check if this domain can spawn new panes.
    ///
    /// Returns false for placeholder or disconnected domains.
    fn spawnable(&self) -> bool {
        self.state() == DomainState::Attached
    }

    /// Check if this domain supports detachment.
    ///
    /// Detachable domains preserve panes when disconnected.
    fn detachable(&self) -> bool;

    /// Attach to the domain (establish connection).
    ///
    /// For local domains, this is typically a no-op.
    /// For remote domains, this establishes the connection.
    fn attach(&self) -> DomainResult<()>;

    /// Detach from the domain.
    ///
    /// For detachable domains, panes continue running.
    /// For non-detachable domains, panes are terminated.
    fn detach(&self) -> DomainResult<()>;

    /// Spawn a new pane in this domain.
    fn spawn_pane(&self, cols: u16, rows: u16, config: SpawnConfig) -> DomainResult<Arc<dyn Pane>>;

    /// Get a pane by ID.
    fn get_pane(&self, id: PaneId) -> Option<Arc<dyn Pane>>;

    /// List all panes in this domain.
    fn list_panes(&self) -> Vec<Arc<dyn Pane>>;

    /// Remove a pane from the domain.
    fn remove_pane(&self, id: PaneId) -> Option<Arc<dyn Pane>>;
}

/// SSH-specific configuration.
#[derive(Debug, Clone)]
pub struct SshConfig {
    /// SSH host to connect to.
    pub host: String,
    /// SSH port (default: 22).
    pub port: u16,
    /// Username for authentication.
    pub username: Option<String>,
    /// Path to identity file (private key).
    pub identity_file: Option<PathBuf>,
    /// Use SSH agent for authentication.
    pub use_agent: bool,
    /// Connection timeout in seconds.
    pub connect_timeout_secs: u32,
    /// Keep-alive interval in seconds (0 = disabled).
    pub keepalive_secs: u32,
    /// Additional SSH options.
    pub options: HashMap<String, String>,
}

impl Default for SshConfig {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: 22,
            username: None,
            identity_file: None,
            use_agent: true,
            connect_timeout_secs: 30,
            keepalive_secs: 60,
            options: HashMap::new(),
        }
    }
}

impl SshConfig {
    /// Create a new SSH config for the given host.
    #[must_use]
    pub fn new(host: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            ..Default::default()
        }
    }

    /// Set the port.
    #[must_use]
    pub fn with_port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    /// Set the username.
    #[must_use]
    pub fn with_username(mut self, username: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self
    }

    /// Set the identity file path.
    #[must_use]
    pub fn with_identity_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.identity_file = Some(path.into());
        self
    }

    /// Disable SSH agent.
    #[must_use]
    pub fn without_agent(mut self) -> Self {
        self.use_agent = false;
        self
    }

    /// Build an SSH URI string (user@host:port).
    #[must_use]
    pub fn to_uri(&self) -> String {
        let mut uri = String::new();
        if let Some(ref user) = self.username {
            uri.push_str(user);
            uri.push('@');
        }
        uri.push_str(&self.host);
        if self.port != 22 {
            uri.push(':');
            uri.push_str(&self.port.to_string());
        }
        uri
    }
}

/// WSL-specific configuration.
#[derive(Debug, Clone)]
pub struct WslConfig {
    /// WSL distribution name (None = default).
    pub distribution: Option<String>,
    /// Default working directory.
    pub default_cwd: Option<PathBuf>,
}

impl Default for WslConfig {
    fn default() -> Self {
        Self {
            distribution: None,
            default_cwd: None,
        }
    }
}

impl WslConfig {
    /// Create a config for a specific WSL distribution.
    #[must_use]
    pub fn distribution(name: impl Into<String>) -> Self {
        Self {
            distribution: Some(name.into()),
            default_cwd: None,
        }
    }
}

/// Serial port configuration.
#[derive(Debug, Clone)]
pub struct SerialConfig {
    /// Serial port path (e.g., /dev/ttyUSB0, COM1).
    pub port: String,
    /// Baud rate.
    pub baud_rate: u32,
    /// Data bits (5, 6, 7, 8).
    pub data_bits: u8,
    /// Stop bits (1, 2).
    pub stop_bits: u8,
    /// Parity (none, odd, even).
    pub parity: SerialParity,
    /// Flow control.
    pub flow_control: SerialFlowControl,
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            port: String::new(),
            baud_rate: 115200,
            data_bits: 8,
            stop_bits: 1,
            parity: SerialParity::None,
            flow_control: SerialFlowControl::None,
        }
    }
}

impl SerialConfig {
    /// Create a config for the given port.
    #[must_use]
    pub fn new(port: impl Into<String>) -> Self {
        Self {
            port: port.into(),
            ..Default::default()
        }
    }

    /// Set the baud rate.
    #[must_use]
    pub fn with_baud_rate(mut self, baud_rate: u32) -> Self {
        self.baud_rate = baud_rate;
        self
    }
}

/// Serial port parity setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerialParity {
    /// No parity.
    #[default]
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

/// Serial port flow control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SerialFlowControl {
    /// No flow control.
    #[default]
    None,
    /// Hardware flow control (RTS/CTS).
    Hardware,
    /// Software flow control (XON/XOFF).
    Software,
}

/// Domain registry for managing multiple domains.
#[derive(Default)]
pub struct DomainRegistry {
    domains: std::sync::RwLock<HashMap<DomainId, Arc<dyn Domain>>>,
    default_domain: std::sync::RwLock<Option<DomainId>>,
}

impl DomainRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a domain.
    pub fn register(&self, domain: Arc<dyn Domain>) {
        let id = domain.domain_id();
        let mut domains = self.domains.write().unwrap();
        let mut default = self.default_domain.write().unwrap();
        domains.insert(id, domain);
        if default.is_none() {
            *default = Some(id);
        }
    }

    /// Unregister a domain.
    pub fn unregister(&self, id: DomainId) -> Option<Arc<dyn Domain>> {
        let mut domains = self.domains.write().unwrap();
        let mut default = self.default_domain.write().unwrap();
        let domain = domains.remove(&id);
        if *default == Some(id) {
            *default = domains.keys().next().copied();
        }
        domain
    }

    /// Get a domain by ID.
    pub fn get(&self, id: DomainId) -> Option<Arc<dyn Domain>> {
        self.domains.read().unwrap().get(&id).cloned()
    }

    /// Get the default domain.
    pub fn default_domain(&self) -> Option<Arc<dyn Domain>> {
        let default = self.default_domain.read().unwrap();
        default.and_then(|id| self.get(id))
    }

    /// Set the default domain.
    pub fn set_default(&self, id: DomainId) {
        let mut default = self.default_domain.write().unwrap();
        *default = Some(id);
    }

    /// List all registered domains.
    pub fn list(&self) -> Vec<Arc<dyn Domain>> {
        self.domains.read().unwrap().values().cloned().collect()
    }

    /// Get a domain by name.
    pub fn get_by_name(&self, name: &str) -> Option<Arc<dyn Domain>> {
        self.domains
            .read()
            .unwrap()
            .values()
            .find(|d| d.domain_name() == name)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_id_unique() {
        let id1 = DomainId::new();
        let id2 = DomainId::new();
        let id3 = DomainId::new();
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn pane_id_unique() {
        let id1 = PaneId::new();
        let id2 = PaneId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn domain_state_default() {
        assert_eq!(DomainState::default(), DomainState::Detached);
    }

    #[test]
    fn spawn_config_builder() {
        let config = SpawnConfig::command("bash")
            .with_cwd("/home/user")
            .with_arg("-c")
            .with_arg("echo hello")
            .with_env("TERM", "xterm-256color");

        assert_eq!(config.command, Some("bash".to_string()));
        assert_eq!(config.cwd, Some(PathBuf::from("/home/user")));
        assert_eq!(config.args, vec!["-c", "echo hello"]);
        assert_eq!(config.env.get("TERM"), Some(&"xterm-256color".to_string()));
    }

    #[test]
    fn ssh_config_builder() {
        let config = SshConfig::new("example.com")
            .with_port(2222)
            .with_username("user")
            .with_identity_file("/home/user/.ssh/id_rsa");

        assert_eq!(config.host, "example.com");
        assert_eq!(config.port, 2222);
        assert_eq!(config.username, Some("user".to_string()));
        assert_eq!(
            config.identity_file,
            Some(PathBuf::from("/home/user/.ssh/id_rsa"))
        );
    }

    #[test]
    fn ssh_config_uri() {
        let config1 = SshConfig::new("example.com");
        assert_eq!(config1.to_uri(), "example.com");

        let config2 = SshConfig::new("example.com").with_username("user");
        assert_eq!(config2.to_uri(), "user@example.com");

        let config3 = SshConfig::new("example.com")
            .with_username("user")
            .with_port(2222);
        assert_eq!(config3.to_uri(), "user@example.com:2222");
    }

    #[test]
    fn serial_config_builder() {
        let config = SerialConfig::new("/dev/ttyUSB0").with_baud_rate(9600);

        assert_eq!(config.port, "/dev/ttyUSB0");
        assert_eq!(config.baud_rate, 9600);
    }

    #[test]
    fn domain_error_display() {
        let err = DomainError::NotAttached;
        assert_eq!(err.to_string(), "domain is not attached");

        let err = DomainError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "connection failed: timeout");
    }

    #[test]
    fn domain_registry_operations() {
        use std::sync::Arc;

        struct MockDomain {
            id: DomainId,
            name: String,
        }

        impl Domain for MockDomain {
            fn domain_id(&self) -> DomainId {
                self.id
            }
            fn domain_name(&self) -> &str {
                &self.name
            }
            fn domain_type(&self) -> DomainType {
                DomainType::Local
            }
            fn state(&self) -> DomainState {
                DomainState::Attached
            }
            fn detachable(&self) -> bool {
                false
            }
            fn attach(&self) -> DomainResult<()> {
                Ok(())
            }
            fn detach(&self) -> DomainResult<()> {
                Ok(())
            }
            fn spawn_pane(
                &self,
                _cols: u16,
                _rows: u16,
                _config: SpawnConfig,
            ) -> DomainResult<Arc<dyn Pane>> {
                Err(DomainError::NotSupported("mock".to_string()))
            }
            fn get_pane(&self, _id: PaneId) -> Option<Arc<dyn Pane>> {
                None
            }
            fn list_panes(&self) -> Vec<Arc<dyn Pane>> {
                vec![]
            }
            fn remove_pane(&self, _id: PaneId) -> Option<Arc<dyn Pane>> {
                None
            }
        }

        let registry = DomainRegistry::new();

        // Register domains
        let domain1 = Arc::new(MockDomain {
            id: DomainId::new(),
            name: "local".to_string(),
        });
        let domain2 = Arc::new(MockDomain {
            id: DomainId::new(),
            name: "ssh".to_string(),
        });

        registry.register(domain1.clone());
        registry.register(domain2.clone());

        // Check list
        assert_eq!(registry.list().len(), 2);

        // Check get
        assert!(registry.get(domain1.domain_id()).is_some());
        assert!(registry.get(domain2.domain_id()).is_some());

        // Check get by name
        assert!(registry.get_by_name("local").is_some());
        assert!(registry.get_by_name("ssh").is_some());
        assert!(registry.get_by_name("nonexistent").is_none());

        // Check default (first registered)
        let default = registry.default_domain().unwrap();
        assert_eq!(default.domain_id(), domain1.domain_id());

        // Change default
        registry.set_default(domain2.domain_id());
        let default = registry.default_domain().unwrap();
        assert_eq!(default.domain_id(), domain2.domain_id());

        // Unregister
        registry.unregister(domain1.domain_id());
        assert!(registry.get(domain1.domain_id()).is_none());
        assert_eq!(registry.list().len(), 1);
    }
}
