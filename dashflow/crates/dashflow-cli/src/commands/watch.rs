// Copyright 2026 Dropbox (created by Andrew Yates <ayates@dropbox.com>)
//! Live graph visualization TUI for DashStream events.
//!
//! This command provides a real-time terminal interface for watching
//! graph execution. It displays:
//! - Header: Graph name, thread ID, elapsed time, live indicator
//! - Graph: ASCII box visualization of nodes with status
//! - Timeline: Scrolling log of execution events
//! - State: Current graph state JSON

use crate::helpers::decode_payload;
use anyhow::{Context, Result};
use clap::Args;
use crossterm::{
    event::{Event as TermEvent, EventStream, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dashflow_streaming::kafka::KafkaSecurityConfig;
use dashflow_streaming::{dash_stream_message, Event as DashStreamEvent, EventType};
use futures::StreamExt;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use rdkafka::{
    consumer::{Consumer, StreamConsumer},
    Message,
};
use serde_json::Value as JsonValue;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Watch live graph execution with TUI visualization
///
/// NOTE: This command is deprecated in favor of `dashflow timeline live`.
/// The watch command will continue to work but new users should use timeline.
#[derive(Args)]
pub struct WatchArgs {
    /// Kafka bootstrap servers (comma-separated)
    /// M-631: Support KAFKA_BROKERS env var for consistency
    #[arg(short, long, env = "KAFKA_BROKERS", default_value = "localhost:9092")]
    pub bootstrap_servers: String,

    /// Kafka topic to consume from
    /// M-433: Default matches library default (dashstream-events)
    /// M-631: Support KAFKA_TOPIC env var for consistency
    #[arg(short, long, env = "KAFKA_TOPIC", default_value = "dashstream-events")]
    pub topic: String,

    /// Filter by thread ID
    #[arg(long)]
    pub thread: Option<String>,

    /// Start from beginning of topic
    #[arg(short, long)]
    pub from_beginning: bool,

    /// Refresh rate in milliseconds
    #[arg(short, long, default_value = "100")]
    pub refresh_ms: u64,
}

/// Node status for visualization
#[derive(Debug, Clone, PartialEq)]
enum NodeStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

impl NodeStatus {
    fn color(&self) -> Color {
        match self {
            NodeStatus::Pending => Color::DarkGray,
            NodeStatus::Running => Color::Yellow,
            NodeStatus::Completed => Color::Green,
            NodeStatus::Failed => Color::Red,
        }
    }

    fn symbol(&self) -> &'static str {
        match self {
            NodeStatus::Pending => "⏳",
            NodeStatus::Running => "⚡",
            NodeStatus::Completed => "✅",
            NodeStatus::Failed => "❌",
        }
    }
}

/// A node in the graph
#[derive(Debug, Clone)]
struct GraphNode {
    /// Node identifier - populated during parse but node name shown via separate lookup
    #[allow(dead_code)] // Architectural: Reserved for node name display feature
    name: String,
    status: NodeStatus,
    duration_ms: Option<u64>,
}

/// Timeline event for display
#[derive(Debug, Clone)]
struct TimelineEvent {
    /// Absolute timestamp for duration calculations
    #[allow(dead_code)] // Architectural: Reserved for inter-event duration calculations
    timestamp: Instant,
    elapsed_ms: u64,
    node: String,
    event_type: String,
    /// Additional event context for detailed display
    #[allow(dead_code)] // Architectural: Reserved for detailed event context display
    details: Option<String>,
}

/// State diff entry for highlighting
#[derive(Debug, Clone)]
struct StateDiffEntry {
    key: String,
    value: String,
    diff_type: DiffType,
}

/// Type of state change for diff highlighting
#[derive(Debug, Clone, PartialEq)]
enum DiffType {
    Unchanged,
    New,
    Changed,
    Removed,
}

impl DiffType {
    fn color(&self) -> Color {
        match self {
            DiffType::Unchanged => Color::White,
            DiffType::New => Color::Green,
            DiffType::Changed => Color::Yellow,
            DiffType::Removed => Color::Red,
        }
    }

    fn marker(&self) -> &'static str {
        match self {
            DiffType::Unchanged => "  ",
            DiffType::New => "+ ",
            DiffType::Changed => "~ ",
            DiffType::Removed => "- ",
        }
    }
}

/// Application state for the TUI
struct App {
    /// Graph name from events
    graph_name: String,
    /// Thread ID being watched
    thread_id: String,
    /// Start time for elapsed calculation
    start_time: Instant,
    /// Nodes in the graph (name -> node)
    nodes: HashMap<String, GraphNode>,
    /// Node execution order for layout
    node_order: Vec<String>,
    /// Timeline events (most recent last) - VecDeque for O(1) front removal (M-464)
    timeline: VecDeque<TimelineEvent>,
    /// Current state as JSON value
    current_state: JsonValue,
    /// Previous state for diff calculation
    previous_state: JsonValue,
    /// State diff entries for display
    state_diff: Vec<StateDiffEntry>,
    /// Whether we're live (receiving events)
    is_live: bool,
    /// Last event time for live detection
    last_event_time: Instant,
    /// M-465: Recent error messages for display
    error_messages: VecDeque<String>,
}

impl App {
    fn new() -> Self {
        let now = Instant::now();
        Self {
            graph_name: "waiting...".to_string(),
            thread_id: "".to_string(),
            start_time: now,
            nodes: HashMap::new(),
            node_order: Vec::new(),
            timeline: VecDeque::new(),
            current_state: JsonValue::Object(serde_json::Map::new()),
            previous_state: JsonValue::Object(serde_json::Map::new()),
            state_diff: Vec::new(),
            is_live: false,
            last_event_time: now,
            error_messages: VecDeque::new(),
        }
    }

    /// M-465: Add an error message to the display queue
    fn add_error(&mut self, message: String) {
        self.error_messages.push_back(message);
        // Keep only the last 5 error messages
        while self.error_messages.len() > 5 {
            self.error_messages.pop_front();
        }
    }

    /// Process a DashStream event
    fn process_event(&mut self, event: &DashStreamEvent) {
        self.last_event_time = Instant::now();
        self.is_live = true;

        // Update thread ID from header
        if let Some(ref header) = event.header {
            if self.thread_id.is_empty() && !header.thread_id.is_empty() {
                self.thread_id = header.thread_id.clone();
                self.start_time = Instant::now();
            }
        }

        // Process node events
        let node_name = event.node_id.clone();
        if !node_name.is_empty() {
            // Ensure node exists
            if !self.nodes.contains_key(&node_name) {
                self.nodes.insert(
                    node_name.clone(),
                    GraphNode {
                        name: node_name.clone(),
                        status: NodeStatus::Pending,
                        duration_ms: None,
                    },
                );
                self.node_order.push(node_name.clone());
            }

            // Update node status based on event type
            let event_type = event.event_type();
            match event_type {
                EventType::NodeStart => {
                    if let Some(node) = self.nodes.get_mut(&node_name) {
                        node.status = NodeStatus::Running;
                    }
                }
                EventType::NodeEnd => {
                    if let Some(node) = self.nodes.get_mut(&node_name) {
                        node.status = NodeStatus::Completed;
                        // Use duration_us from Event (convert to ms)
                        if event.duration_us > 0 {
                            node.duration_ms = Some((event.duration_us / 1000) as u64);
                        }
                    }
                }
                EventType::NodeError => {
                    if let Some(node) = self.nodes.get_mut(&node_name) {
                        node.status = NodeStatus::Failed;
                    }
                }
                _ => {}
            }

            // Add timeline event
            let elapsed = self.start_time.elapsed().as_millis() as u64;
            self.timeline.push_back(TimelineEvent {
                timestamp: Instant::now(),
                elapsed_ms: elapsed,
                node: node_name.clone(),
                event_type: format!("{:?}", event_type),
                details: None,
            });

            // Keep timeline bounded - O(1) with VecDeque (M-464)
            if self.timeline.len() > 100 {
                self.timeline.pop_front();
            }
        }

        // Check for GraphStart to update graph name
        if event.event_type() == EventType::GraphStart {
            // Try to get graph name from attributes
            if let Some(attr) = event.attributes.get("graph_name") {
                if let Some(dashflow_streaming::attribute_value::Value::StringValue(name)) =
                    &attr.value
                {
                    self.graph_name = name.clone();
                }
            }
        }

        // Extract state from event attributes
        self.extract_state_from_event(event);
    }

    /// Extract state from event attributes and compute diff
    fn extract_state_from_event(&mut self, event: &DashStreamEvent) {
        // Look for state in attributes
        let mut new_state = serde_json::Map::new();

        for (key, attr) in &event.attributes {
            if let Some(ref value) = attr.value {
                let json_val = match value {
                    dashflow_streaming::attribute_value::Value::StringValue(s) => {
                        JsonValue::String(s.clone())
                    }
                    dashflow_streaming::attribute_value::Value::IntValue(i) => {
                        JsonValue::Number((*i).into())
                    }
                    dashflow_streaming::attribute_value::Value::FloatValue(f) => {
                        serde_json::Number::from_f64(*f)
                            .map(JsonValue::Number)
                            .unwrap_or(JsonValue::Null)
                    }
                    dashflow_streaming::attribute_value::Value::BoolValue(b) => JsonValue::Bool(*b),
                    dashflow_streaming::attribute_value::Value::BytesValue(b) => {
                        // Try to parse bytes as JSON string first
                        if let Ok(s) = std::str::from_utf8(b) {
                            if let Ok(parsed) = serde_json::from_str(s) {
                                parsed
                            } else {
                                JsonValue::String(s.to_string())
                            }
                        } else {
                            JsonValue::String(format!("<{} bytes>", b.len()))
                        }
                    }
                    dashflow_streaming::attribute_value::Value::ArrayValue(arr) => {
                        let items: Vec<JsonValue> = arr
                            .values
                            .iter()
                            .filter_map(|v| {
                                v.value.as_ref().and_then(|val| match val {
                                    dashflow_streaming::attribute_value::Value::StringValue(s) => {
                                        Some(JsonValue::String(s.clone()))
                                    }
                                    dashflow_streaming::attribute_value::Value::IntValue(i) => {
                                        Some(JsonValue::Number((*i).into()))
                                    }
                                    dashflow_streaming::attribute_value::Value::BoolValue(b) => {
                                        Some(JsonValue::Bool(*b))
                                    }
                                    _ => None,
                                })
                            })
                            .collect();
                        JsonValue::Array(items)
                    }
                    dashflow_streaming::attribute_value::Value::MapValue(map) => {
                        let obj: serde_json::Map<String, JsonValue> = map
                            .values
                            .iter()
                            .filter_map(|(k, v)| {
                                v.value.as_ref().and_then(|val| match val {
                                    dashflow_streaming::attribute_value::Value::StringValue(s) => {
                                        Some((k.clone(), JsonValue::String(s.clone())))
                                    }
                                    dashflow_streaming::attribute_value::Value::IntValue(i) => {
                                        Some((k.clone(), JsonValue::Number((*i).into())))
                                    }
                                    dashflow_streaming::attribute_value::Value::BoolValue(b) => {
                                        Some((k.clone(), JsonValue::Bool(*b)))
                                    }
                                    _ => None,
                                })
                            })
                            .collect();
                        JsonValue::Object(obj)
                    }
                };
                new_state.insert(key.clone(), json_val);
            }
        }

        // Merge new state into current state (to accumulate state over time)
        if !new_state.is_empty() {
            // Save previous for diff
            self.previous_state = self.current_state.clone();

            // Merge new values into current state
            if let JsonValue::Object(ref mut current) = self.current_state {
                for (key, value) in new_state {
                    current.insert(key, value);
                }
            }

            // Compute diff
            self.compute_state_diff();
        }
    }

    /// Compute state diff between previous and current state
    fn compute_state_diff(&mut self) {
        self.state_diff.clear();

        let current_obj = match &self.current_state {
            JsonValue::Object(obj) => obj,
            _ => return,
        };

        let previous_obj = match &self.previous_state {
            JsonValue::Object(obj) => obj,
            _ => {
                // All current keys are new
                for (key, value) in current_obj {
                    self.state_diff.push(StateDiffEntry {
                        key: key.clone(),
                        value: format_json_value(value),
                        diff_type: DiffType::New,
                    });
                }
                return;
            }
        };

        // Track all keys
        let mut all_keys: HashSet<&String> = current_obj.keys().collect();
        all_keys.extend(previous_obj.keys());

        let mut keys: Vec<&String> = all_keys.into_iter().collect();
        keys.sort();

        for key in keys {
            let current_val = current_obj.get(key);
            let previous_val = previous_obj.get(key);

            match (current_val, previous_val) {
                (Some(curr), Some(prev)) => {
                    let diff_type = if curr == prev {
                        DiffType::Unchanged
                    } else {
                        DiffType::Changed
                    };
                    self.state_diff.push(StateDiffEntry {
                        key: key.clone(),
                        value: format_json_value(curr),
                        diff_type,
                    });
                }
                (Some(curr), None) => {
                    self.state_diff.push(StateDiffEntry {
                        key: key.clone(),
                        value: format_json_value(curr),
                        diff_type: DiffType::New,
                    });
                }
                (None, Some(prev)) => {
                    self.state_diff.push(StateDiffEntry {
                        key: key.clone(),
                        value: format_json_value(prev),
                        diff_type: DiffType::Removed,
                    });
                }
                (None, None) => {
                    // Logically unreachable: all_keys is built from both maps,
                    // so every key must exist in at least one. Skip if somehow reached.
                    continue;
                }
            }
        }
    }

    /// Check if we should show live indicator
    fn update_live_status(&mut self) {
        // Consider not live if no events in 2 seconds
        if self.last_event_time.elapsed() > Duration::from_secs(2) {
            self.is_live = false;
        }
    }

    /// Get elapsed time as formatted string
    fn elapsed_str(&self) -> String {
        let elapsed = self.start_time.elapsed();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();
        format!("{:02}:{:02}.{}", secs / 60, secs % 60, millis / 100)
    }
}

/// Format a JSON value for display (truncated if too long)
fn format_json_value(value: &JsonValue) -> String {
    let formatted = match value {
        JsonValue::String(s) => {
            if s.len() > 50 {
                format!("\"{}...\"", &s[..47])
            } else {
                format!("\"{}\"", s)
            }
        }
        JsonValue::Array(arr) => {
            if arr.len() <= 3 {
                format!("{:?}", arr)
            } else {
                format!("[{} items]", arr.len())
            }
        }
        JsonValue::Object(obj) => {
            if obj.len() <= 2 {
                serde_json::to_string(value).unwrap_or_else(|_| "{...}".to_string())
            } else {
                format!("{{{} keys}}", obj.len())
            }
        }
        _ => serde_json::to_string(value).unwrap_or_else(|_| "?".to_string()),
    };

    // Truncate if still too long
    if formatted.len() > 60 {
        format!("{}...", &formatted[..57])
    } else {
        formatted
    }
}

/// Event from Kafka consumer task
enum KafkaEvent {
    Message(Box<DashStreamEvent>),
    /// Error variant for Kafka error handling (connection drops, deserialization failures)
    #[allow(dead_code)] // Architectural: Reserved for Kafka connection error handling
    Error(String),
}

pub async fn run(args: WatchArgs) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create channel for Kafka events
    let (tx, mut rx) = mpsc::channel::<KafkaEvent>(100);

    // Start Kafka consumer task
    let bootstrap_servers = args.bootstrap_servers.clone();
    let topic = args.topic.clone();
    let from_beginning = args.from_beginning;
    let thread_filter = args.thread.clone();

    tokio::spawn(async move {
        if let Err(e) =
            kafka_consumer_task(tx, bootstrap_servers, topic, from_beginning, thread_filter).await
        {
            eprintln!("Kafka consumer error: {e}");
        }
    });

    // Run TUI event loop
    let result = run_tui(&mut terminal, &mut rx, args.refresh_ms).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

/// M-465: Maximum consecutive errors before the consumer exits.
/// Prevents infinite retry loops on persistent Kafka failures.
const MAX_CONSECUTIVE_ERRORS: u32 = 10;

async fn kafka_consumer_task(
    tx: mpsc::Sender<KafkaEvent>,
    bootstrap_servers: String,
    topic: String,
    from_beginning: bool,
    thread_filter: Option<String>,
) -> Result<()> {
    // M-413: Apply security config from environment
    let security_config = KafkaSecurityConfig::from_env();
    let mut client_config = security_config.create_client_config(&bootstrap_servers);
    client_config
        .set(
            "group.id",
            format!("dashstream-watch-{}", std::process::id()),
        )
        .set(
            "auto.offset.reset",
            if from_beginning { "earliest" } else { "latest" },
        )
        .set("enable.auto.commit", "false");
    let consumer: StreamConsumer = client_config
        .create()
        .context("Failed to create Kafka consumer")?;

    consumer
        .subscribe(&[&topic])
        .context("Failed to subscribe to topic")?;

    // M-465: Track consecutive errors to exit on persistent failures
    let mut consecutive_errors: u32 = 0;

    loop {
        match consumer.recv().await {
            Ok(message) => {
                // Reset error counter on successful message
                consecutive_errors = 0;

                if let Some(payload) = message.payload() {
                    if let Ok(msg) = decode_payload(payload) {
                        if let Some(dash_stream_message::Message::Event(event)) = msg.message {
                            // Apply thread filter (check header.thread_id)
                            if let Some(ref filter) = thread_filter {
                                let event_thread =
                                    event.header.as_ref().map(|h| h.thread_id.as_str());
                                if event_thread != Some(filter.as_str()) {
                                    continue;
                                }
                            }
                            let _ = tx.send(KafkaEvent::Message(Box::new(event))).await;
                        }
                    }
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                let error_msg = format!(
                    "Kafka error ({}/{}): {}",
                    consecutive_errors, MAX_CONSECUTIVE_ERRORS, e
                );
                let _ = tx.send(KafkaEvent::Error(error_msg)).await;

                // M-465: Exit after too many consecutive errors
                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    let _ = tx
                        .send(KafkaEvent::Error(format!(
                            "Exiting: {} consecutive Kafka errors (max: {})",
                            consecutive_errors, MAX_CONSECUTIVE_ERRORS
                        )))
                        .await;
                    return Err(anyhow::anyhow!(
                        "Kafka consumer exiting after {} consecutive errors",
                        consecutive_errors
                    ));
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

async fn run_tui(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    rx: &mut mpsc::Receiver<KafkaEvent>,
    refresh_ms: u64,
) -> Result<()> {
    let mut app = App::new();
    let tick_rate = Duration::from_millis(refresh_ms);
    let mut term_events = EventStream::new();
    let mut tick_interval = tokio::time::interval(tick_rate);

    loop {
        // Draw the UI
        terminal.draw(|f| ui(f, &app))?;

        // Use tokio::select! for concurrent event handling
        tokio::select! {
            // Handle terminal events (keyboard input)
            Some(Ok(term_event)) = term_events.next() => {
                if let TermEvent::Key(key) = term_event {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('r') => {
                                app = App::new();
                            }
                            KeyCode::Char('c') => {
                                // Clear state
                                app.current_state = JsonValue::Object(serde_json::Map::new());
                                app.previous_state = JsonValue::Object(serde_json::Map::new());
                                app.state_diff.clear();
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Handle Kafka events
            Some(event) = rx.recv() => {
                match event {
                    KafkaEvent::Message(e) => app.process_event(&e),
                    // M-465: Display errors to user
                    KafkaEvent::Error(msg) => app.add_error(msg),
                }
            }

            // Regular tick for UI updates (elapsed time counter)
            _ = tick_interval.tick() => {
                // Update live status
                app.update_live_status();
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    // Create layout: Header (3 lines), rest splits into Graph|Timeline|State
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
        ])
        .split(f.area());

    // Render header
    render_header(f, app, chunks[0]);

    // Split main content: Graph (top), Timeline+State (bottom)
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Graph
            Constraint::Percentage(50), // Timeline + State
        ])
        .split(chunks[1]);

    // Render graph
    render_graph(f, app, main_chunks[0]);

    // Split bottom: Timeline (left), State (right)
    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // Timeline
            Constraint::Percentage(50), // State
        ])
        .split(main_chunks[1]);

    // Render timeline
    render_timeline(f, app, bottom_chunks[0]);

    // Render state
    render_state(f, app, bottom_chunks[1]);
}

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let live_indicator = if app.is_live {
        Span::styled(
            " 🔴 LIVE ",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(" ⚫ ", Style::default().fg(Color::DarkGray))
    };

    let header_text = Line::from(vec![
        live_indicator,
        Span::styled(
            &app.graph_name,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  thread:"),
        Span::styled(
            if app.thread_id.is_empty() {
                "waiting"
            } else {
                &app.thread_id
            },
            Style::default().fg(Color::Yellow),
        ),
        Span::raw("  elapsed:"),
        Span::styled(app.elapsed_str(), Style::default().fg(Color::Green)),
        Span::raw("  [q]uit [r]eset [c]lear"),
    ]);

    let header = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title("DashFlow Watch"),
    );

    f.render_widget(header, area);
}

fn render_graph(f: &mut Frame, app: &App, area: Rect) {
    let mut lines = Vec::new();

    // M-465: Show error messages if any
    if !app.error_messages.is_empty() {
        for error in &app.error_messages {
            lines.push(Line::from(Span::styled(
                format!("⚠ {}", error),
                Style::default().fg(Color::Red),
            )));
        }
        lines.push(Line::from(Span::raw(""))); // Spacer
    }

    if app.nodes.is_empty() && app.error_messages.is_empty() {
        lines.push(Line::from(Span::styled(
            "Waiting for graph events...",
            Style::default().fg(Color::DarkGray),
        )));
    } else if app.nodes.is_empty() {
        // Just show the errors, already added above
    } else {
        // Render nodes as horizontal ASCII boxes
        let mut node_line: Vec<Span> = Vec::new();

        for (i, name) in app.node_order.iter().enumerate() {
            if let Some(node) = app.nodes.get(name) {
                let status_color = node.status.color();

                // Add node box top border
                node_line.push(Span::styled(
                    "┌──────────┐".to_string(),
                    Style::default().fg(status_color),
                ));

                if i < app.node_order.len() - 1 {
                    node_line.push(Span::raw("    "));
                }
            }
        }

        if !node_line.is_empty() {
            lines.push(Line::from(node_line));
        }

        // Node content line
        let mut content_line: Vec<Span> = Vec::new();
        for (i, name) in app.node_order.iter().enumerate() {
            if let Some(node) = app.nodes.get(name) {
                let status_color = node.status.color();
                let symbol = node.status.symbol();
                let display_name: String = if name.len() > 8 {
                    name[..8].to_string()
                } else {
                    format!("{:^8}", name)
                };

                content_line.push(Span::styled(
                    format!("│{} {:8}│", symbol, display_name),
                    Style::default().fg(status_color),
                ));

                if i < app.node_order.len() - 1 {
                    content_line.push(Span::styled(" ──▶ ", Style::default().fg(Color::White)));
                }
            }
        }

        if !content_line.is_empty() {
            lines.push(Line::from(content_line));
        }

        // Duration/status line
        let mut status_line: Vec<Span> = Vec::new();
        for (i, name) in app.node_order.iter().enumerate() {
            if let Some(node) = app.nodes.get(name) {
                let status_color = node.status.color();
                let duration_str = node
                    .duration_ms
                    .map(|d| format!("{:>6}ms", d))
                    .unwrap_or_else(|| format!("{:>8}", ""));

                status_line.push(Span::styled(
                    format!("│{:10}│", duration_str),
                    Style::default().fg(status_color),
                ));

                if i < app.node_order.len() - 1 {
                    status_line.push(Span::raw("    "));
                }
            }
        }

        if !status_line.is_empty() {
            lines.push(Line::from(status_line));
        }

        // Bottom border
        let mut bottom_line: Vec<Span> = Vec::new();
        for (i, name) in app.node_order.iter().enumerate() {
            if let Some(node) = app.nodes.get(name) {
                let status_color = node.status.color();
                bottom_line.push(Span::styled(
                    "└──────────┘".to_string(),
                    Style::default().fg(status_color),
                ));

                if i < app.node_order.len() - 1 {
                    bottom_line.push(Span::raw("    "));
                }
            }
        }

        if !bottom_line.is_empty() {
            lines.push(Line::from(bottom_line));
        }
    }

    let graph = Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Graph"));

    f.render_widget(graph, area);
}

fn render_timeline(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .timeline
        .iter()
        .rev()
        .take(20)
        .map(|e| {
            let time_str = format!("{:06.1}", e.elapsed_ms as f64 / 1000.0);
            let event_color = match e.event_type.as_str() {
                "NodeStart" => Color::Yellow,
                "NodeEnd" => Color::Green,
                "NodeError" => Color::Red,
                _ => Color::White,
            };

            let event_symbol = match e.event_type.as_str() {
                "NodeStart" => "▶",
                "NodeEnd" => "◀",
                "NodeError" => "✖",
                _ => "•",
            };

            ListItem::new(Line::from(vec![
                Span::styled(time_str, Style::default().fg(Color::DarkGray)),
                Span::raw("  "),
                Span::styled(&e.node, Style::default().fg(Color::Cyan)),
                Span::raw(" "),
                Span::styled(event_symbol, Style::default().fg(event_color)),
                Span::raw(" "),
                Span::styled(&e.event_type, Style::default().fg(event_color)),
            ]))
        })
        .collect();

    let timeline = List::new(items).block(Block::default().borders(Borders::ALL).title("Timeline"));

    f.render_widget(timeline, area);
}

fn render_state(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.state_diff.is_empty() {
        // Show current state as formatted JSON if no diff yet
        if let JsonValue::Object(obj) = &app.current_state {
            if obj.is_empty() {
                vec![ListItem::new(Line::from(Span::styled(
                    "Waiting for state...",
                    Style::default().fg(Color::DarkGray),
                )))]
            } else {
                obj.iter()
                    .map(|(k, v)| {
                        ListItem::new(Line::from(vec![
                            Span::styled("  ", Style::default()),
                            Span::styled(k, Style::default().fg(Color::Cyan)),
                            Span::raw(": "),
                            Span::styled(format_json_value(v), Style::default().fg(Color::White)),
                        ]))
                    })
                    .collect()
            }
        } else {
            vec![ListItem::new(Line::from(Span::styled(
                "{}",
                Style::default().fg(Color::DarkGray),
            )))]
        }
    } else {
        // Show diff with highlighting
        app.state_diff
            .iter()
            .map(|entry| {
                let marker = entry.diff_type.marker();
                let color = entry.diff_type.color();
                let style = Style::default().fg(color);

                // Add modifier for changed/new values
                let value_style = match entry.diff_type {
                    DiffType::New | DiffType::Changed => style.add_modifier(Modifier::BOLD),
                    DiffType::Removed => style.add_modifier(Modifier::DIM),
                    DiffType::Unchanged => style,
                };

                ListItem::new(Line::from(vec![
                    Span::styled(marker, style),
                    Span::styled(&entry.key, Style::default().fg(Color::Cyan)),
                    Span::raw(": "),
                    Span::styled(&entry.value, value_style),
                ]))
            })
            .collect()
    };

    let state = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("State (+ new, ~ changed, - removed)"),
    );

    f.render_widget(state, area);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic)]

    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(flatten)]
        watch: WatchArgs,
    }

    #[test]
    fn test_watch_args_defaults() {
        let cli = Cli::parse_from(["test"]);

        assert_eq!(cli.watch.bootstrap_servers, "localhost:9092");
        assert_eq!(cli.watch.topic, "dashstream-events");
        assert!(cli.watch.thread.is_none());
        assert!(!cli.watch.from_beginning);
        assert_eq!(cli.watch.refresh_ms, 100);
    }

    #[test]
    fn test_watch_args_with_thread_filter() {
        let cli = Cli::parse_from([
            "test",
            "--thread", "thread-123",
        ]);

        assert_eq!(cli.watch.thread, Some("thread-123".to_string()));
    }

    #[test]
    fn test_watch_args_kafka_config() {
        let cli = Cli::parse_from([
            "test",
            "--bootstrap-servers", "kafka1:9092,kafka2:9092",
            "--topic", "custom-events",
            "--from-beginning",
        ]);

        assert_eq!(cli.watch.bootstrap_servers, "kafka1:9092,kafka2:9092");
        assert_eq!(cli.watch.topic, "custom-events");
        assert!(cli.watch.from_beginning);
    }

    #[test]
    fn test_watch_args_refresh_rate() {
        let cli = Cli::parse_from([
            "test",
            "--refresh-ms", "50",
        ]);

        assert_eq!(cli.watch.refresh_ms, 50);
    }

    #[test]
    fn test_node_status_colors() {
        assert_eq!(NodeStatus::Pending.color(), Color::DarkGray);
        assert_eq!(NodeStatus::Running.color(), Color::Yellow);
        assert_eq!(NodeStatus::Completed.color(), Color::Green);
        assert_eq!(NodeStatus::Failed.color(), Color::Red);
    }

    #[test]
    fn test_node_status_symbols() {
        assert_eq!(NodeStatus::Pending.symbol(), "⏳");
        assert_eq!(NodeStatus::Running.symbol(), "⚡");
        assert_eq!(NodeStatus::Completed.symbol(), "✅");
        assert_eq!(NodeStatus::Failed.symbol(), "❌");
    }

    // ==========================================
    // M-446: Core Logic Tests (previously untested)
    // ==========================================

    #[test]
    fn test_diff_type_colors() {
        assert_eq!(DiffType::Unchanged.color(), Color::White);
        assert_eq!(DiffType::New.color(), Color::Green);
        assert_eq!(DiffType::Changed.color(), Color::Yellow);
        assert_eq!(DiffType::Removed.color(), Color::Red);
    }

    #[test]
    fn test_diff_type_markers() {
        assert_eq!(DiffType::Unchanged.marker(), "  ");
        assert_eq!(DiffType::New.marker(), "+ ");
        assert_eq!(DiffType::Changed.marker(), "~ ");
        assert_eq!(DiffType::Removed.marker(), "- ");
    }

    #[test]
    fn test_format_json_value_string() {
        // Short string
        let short = JsonValue::String("hello".to_string());
        assert_eq!(format_json_value(&short), "\"hello\"");

        // Long string gets truncated
        let long = JsonValue::String("a".repeat(60));
        let formatted = format_json_value(&long);
        assert!(formatted.ends_with("...\""));
        assert!(formatted.len() <= 52); // 47 chars + 5 for quotes and ellipsis
    }

    #[test]
    fn test_format_json_value_array() {
        // Small array shows contents
        let small: JsonValue = serde_json::from_str("[1, 2, 3]").unwrap();
        let formatted = format_json_value(&small);
        // Should show some representation of the array
        assert!(!formatted.is_empty());

        // Large array shows count
        let large: JsonValue = serde_json::from_str("[1, 2, 3, 4, 5]").unwrap();
        let formatted = format_json_value(&large);
        assert!(formatted.contains("5 items"));
    }

    #[test]
    fn test_format_json_value_object() {
        // Small object shows contents
        let small: JsonValue = serde_json::from_str(r#"{"a": 1}"#).unwrap();
        let formatted = format_json_value(&small);
        assert!(!formatted.is_empty());

        // Large object shows key count
        let large: JsonValue = serde_json::from_str(r#"{"a": 1, "b": 2, "c": 3, "d": 4}"#).unwrap();
        let formatted = format_json_value(&large);
        assert!(formatted.contains("4 keys"));
    }

    #[test]
    fn test_format_json_value_primitives() {
        assert_eq!(format_json_value(&JsonValue::Bool(true)), "true");
        assert_eq!(format_json_value(&JsonValue::Bool(false)), "false");
        assert_eq!(format_json_value(&JsonValue::Null), "null");
        assert_eq!(format_json_value(&JsonValue::Number(42.into())), "42");
    }

    #[test]
    fn test_app_new_initializes_empty() {
        let app = App::new();

        assert_eq!(app.graph_name, "waiting...");
        assert!(app.thread_id.is_empty());
        assert!(app.nodes.is_empty());
        assert!(app.node_order.is_empty());
        assert!(app.timeline.is_empty());
        assert!(!app.is_live);
        assert!(app.error_messages.is_empty());
    }

    #[test]
    fn test_app_add_error_limits_queue() {
        let mut app = App::new();

        // Add 7 errors
        for i in 0..7 {
            app.add_error(format!("Error {}", i));
        }

        // Should only keep last 5
        assert_eq!(app.error_messages.len(), 5);
        // First error should be "Error 2" (0 and 1 were removed)
        assert_eq!(app.error_messages.front().unwrap(), "Error 2");
        assert_eq!(app.error_messages.back().unwrap(), "Error 6");
    }

    #[test]
    fn test_app_elapsed_str_format() {
        let app = App::new();
        let elapsed = app.elapsed_str();

        // Format should be MM:SS.T (e.g., "00:00.0")
        assert!(elapsed.contains(':'));
        assert!(elapsed.contains('.'));
        // Initially very close to 0
        assert!(elapsed.starts_with("00:"));
    }

    #[test]
    fn test_app_update_live_status() {
        let mut app = App::new();

        // Initially not live
        assert!(!app.is_live);

        // After setting is_live true and calling update immediately
        app.is_live = true;
        app.last_event_time = std::time::Instant::now();
        app.update_live_status();
        assert!(app.is_live); // Should still be live (event was recent)
    }

    #[test]
    fn test_compute_state_diff_new_keys() {
        let mut app = App::new();

        // Set current state with new keys (previous is empty)
        app.current_state = serde_json::json!({
            "key1": "value1",
            "key2": 42
        });
        app.previous_state = JsonValue::Object(serde_json::Map::new());

        app.compute_state_diff();

        // All keys should be marked as New
        assert_eq!(app.state_diff.len(), 2);
        for entry in &app.state_diff {
            assert_eq!(entry.diff_type, DiffType::New);
        }
    }

    #[test]
    fn test_compute_state_diff_changed_values() {
        let mut app = App::new();

        app.previous_state = serde_json::json!({"counter": 1});
        app.current_state = serde_json::json!({"counter": 2});

        app.compute_state_diff();

        assert_eq!(app.state_diff.len(), 1);
        assert_eq!(app.state_diff[0].key, "counter");
        assert_eq!(app.state_diff[0].diff_type, DiffType::Changed);
    }

    #[test]
    fn test_compute_state_diff_removed_keys() {
        let mut app = App::new();

        app.previous_state = serde_json::json!({"old_key": "old_value"});
        app.current_state = JsonValue::Object(serde_json::Map::new());

        app.compute_state_diff();

        assert_eq!(app.state_diff.len(), 1);
        assert_eq!(app.state_diff[0].key, "old_key");
        assert_eq!(app.state_diff[0].diff_type, DiffType::Removed);
    }

    #[test]
    fn test_compute_state_diff_unchanged_values() {
        let mut app = App::new();

        app.previous_state = serde_json::json!({"stable": "value"});
        app.current_state = serde_json::json!({"stable": "value"});

        app.compute_state_diff();

        assert_eq!(app.state_diff.len(), 1);
        assert_eq!(app.state_diff[0].diff_type, DiffType::Unchanged);
    }

    #[test]
    fn test_compute_state_diff_mixed_changes() {
        let mut app = App::new();

        app.previous_state = serde_json::json!({
            "unchanged": "same",
            "modified": "old",
            "removed": "gone"
        });
        app.current_state = serde_json::json!({
            "unchanged": "same",
            "modified": "new",
            "added": "fresh"
        });

        app.compute_state_diff();

        // Should have 4 entries: unchanged, modified (changed), removed, added (new)
        assert_eq!(app.state_diff.len(), 4);

        let find_entry = |key: &str| app.state_diff.iter().find(|e| e.key == key);

        assert_eq!(find_entry("unchanged").unwrap().diff_type, DiffType::Unchanged);
        assert_eq!(find_entry("modified").unwrap().diff_type, DiffType::Changed);
        assert_eq!(find_entry("removed").unwrap().diff_type, DiffType::Removed);
        assert_eq!(find_entry("added").unwrap().diff_type, DiffType::New);
    }

    #[test]
    fn test_compute_state_diff_non_object_current() {
        let mut app = App::new();

        // If current state is not an object, diff should be empty
        app.current_state = JsonValue::Null;
        app.previous_state = serde_json::json!({"key": "value"});

        app.compute_state_diff();

        assert!(app.state_diff.is_empty());
    }

    #[test]
    fn test_compute_state_diff_non_object_previous() {
        let mut app = App::new();

        // If previous state is not an object, all current keys are New
        app.current_state = serde_json::json!({"key1": "value1", "key2": "value2"});
        app.previous_state = JsonValue::Null;

        app.compute_state_diff();

        assert_eq!(app.state_diff.len(), 2);
        for entry in &app.state_diff {
            assert_eq!(entry.diff_type, DiffType::New);
        }
    }

    #[test]
    fn test_timeline_bounded_at_100() {
        let mut app = App::new();

        // Simulate adding many timeline events
        for i in 0..150 {
            app.timeline.push_back(TimelineEvent {
                timestamp: std::time::Instant::now(),
                elapsed_ms: i as u64,
                node: format!("node_{}", i),
                event_type: "TestEvent".to_string(),
                details: None,
            });
            // Replicate the bounding logic from process_event
            if app.timeline.len() > 100 {
                app.timeline.pop_front();
            }
        }

        assert_eq!(app.timeline.len(), 100);
        // First event should be from iteration 50 (events 0-49 were removed)
        assert_eq!(app.timeline.front().unwrap().node, "node_50");
    }

    #[test]
    fn test_graph_node_default_values() {
        let node = GraphNode {
            name: "test_node".to_string(),
            status: NodeStatus::Pending,
            duration_ms: None,
        };

        assert_eq!(node.name, "test_node");
        assert_eq!(node.status, NodeStatus::Pending);
        assert!(node.duration_ms.is_none());
    }

    #[test]
    fn test_state_diff_entry_structure() {
        let entry = StateDiffEntry {
            key: "test_key".to_string(),
            value: "test_value".to_string(),
            diff_type: DiffType::New,
        };

        assert_eq!(entry.key, "test_key");
        assert_eq!(entry.value, "test_value");
        assert_eq!(entry.diff_type, DiffType::New);
    }

    #[test]
    fn test_timeline_event_structure() {
        let event = TimelineEvent {
            timestamp: std::time::Instant::now(),
            elapsed_ms: 1234,
            node: "my_node".to_string(),
            event_type: "NodeStart".to_string(),
            details: Some("extra info".to_string()),
        };

        assert_eq!(event.elapsed_ms, 1234);
        assert_eq!(event.node, "my_node");
        assert_eq!(event.event_type, "NodeStart");
        assert_eq!(event.details, Some("extra info".to_string()));
    }

    #[test]
    fn test_kafka_event_enum_variants() {
        // Test that KafkaEvent can hold both Message and Error
        let error_event = KafkaEvent::Error("Test error".to_string());
        match error_event {
            KafkaEvent::Error(msg) => assert_eq!(msg, "Test error"),
            KafkaEvent::Message(_) => panic!("Expected Error variant"),
        }

        // Message variant would require constructing a full DashStreamEvent
        // which is complex, so we just verify the enum exists and Error works
    }

    #[test]
    fn test_max_consecutive_errors_constant() {
        // Verify the constant is set to a reasonable value
        assert_eq!(MAX_CONSECUTIVE_ERRORS, 10);
    }
}
