// Allow clippy warnings for this module
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::clone_on_ref_ptr)]
#![allow(clippy::needless_pass_by_value, clippy::redundant_clone)]

//! Communication platform loaders.
//!
//! This module provides loaders for communication platforms including:
//! - Discord (JSON export)
//! - Facebook Messenger (JSON export)
//! - Telegram (JSON export)
//! - `WhatsApp` (text export)
//! - Slack (zip export and workspace export)
//! - iMessage (macOS `SQLite` database)
//!
//! © 2025 Dropbox (created by Andrew Yates <ayates@dropbox.com>)

use async_trait::async_trait;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::config_loader::env_vars::{env_string, HOME};
use crate::core::documents::{Blob, Document, DocumentLoader};
use crate::core::error::{Error, Result};
use serde_json::Value;

/// Loader for Discord chat exports (JSON format).
///
/// Parses Discord chat export JSON files and extracts messages with metadata.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::DiscordLoader;
/// use dashflow::core::documents::DocumentLoader;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = DiscordLoader::new("chat_export.json");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct DiscordLoader {
    file_path: PathBuf,
    user_id_filter: Option<String>,
}

impl DiscordLoader {
    /// Create a new Discord loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            user_id_filter: None,
        }
    }

    /// Filter messages to only include those from a specific user ID.
    #[must_use]
    pub fn with_user_filter(mut self, user_id: impl Into<String>) -> Self {
        self.user_id_filter = Some(user_id.into());
        self
    }
}

#[async_trait]
impl DocumentLoader for DiscordLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        // Parse as JSON
        let json: Value = serde_json::from_str(&content)?;

        let messages = if let Some(msgs) = json.get("messages").and_then(|m| m.as_array()) {
            msgs
        } else if let Some(arr) = json.as_array() {
            arr
        } else {
            return Err(crate::core::error::Error::InvalidInput(
                "Expected 'messages' array in Discord export".to_string(),
            ));
        };

        let mut documents = Vec::new();

        for msg in messages {
            // Filter by user if specified
            if let Some(ref user_filter) = self.user_id_filter {
                if let Some(author_id) = msg
                    .get("author")
                    .and_then(|a| a.get("id"))
                    .and_then(|id| id.as_str())
                {
                    if author_id != user_filter {
                        continue;
                    }
                }
            }

            let content = msg
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();

            if content.is_empty() {
                continue;
            }

            let mut doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "discord");

            // Add author information
            if let Some(author) = msg.get("author") {
                if let Some(username) = author.get("username").and_then(|u| u.as_str()) {
                    doc = doc.with_metadata("author", username.to_string());
                }
                if let Some(user_id) = author.get("id").and_then(|id| id.as_str()) {
                    doc = doc.with_metadata("user_id", user_id.to_string());
                }
            }

            // Add timestamp
            if let Some(timestamp) = msg.get("timestamp").and_then(|t| t.as_str()) {
                doc = doc.with_metadata("timestamp", timestamp.to_string());
            }

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loader for Facebook Messenger chat exports (JSON format).
///
/// Parses Facebook Messenger export JSON files.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, FacebookChatLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = FacebookChatLoader::new("message_1.json");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct FacebookChatLoader {
    file_path: PathBuf,
    user_filter: Option<String>,
}

impl FacebookChatLoader {
    /// Create a new Facebook chat loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
            user_filter: None,
        }
    }

    /// Filter messages to only include those from a specific user.
    #[must_use]
    pub fn with_user_filter(mut self, user: impl Into<String>) -> Self {
        self.user_filter = Some(user.into());
        self
    }
}

#[async_trait]
impl DocumentLoader for FacebookChatLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let json: Value = serde_json::from_str(&content)?;

        let messages = json
            .get("messages")
            .and_then(|m| m.as_array())
            .ok_or_else(|| {
                crate::core::error::Error::InvalidInput(
                    "Expected 'messages' array in Facebook export".to_string(),
                )
            })?;

        let mut documents = Vec::new();

        for msg in messages {
            // Filter by user if specified
            if let Some(ref user_filter) = self.user_filter {
                if let Some(sender) = msg.get("sender_name").and_then(|s| s.as_str()) {
                    if sender != user_filter {
                        continue;
                    }
                }
            }

            let content = msg
                .get("content")
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string();

            if content.is_empty() {
                continue;
            }

            let mut doc = Document::new(content)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "facebook_messenger");

            if let Some(sender) = msg.get("sender_name").and_then(|s| s.as_str()) {
                doc = doc.with_metadata("sender", sender.to_string());
            }

            if let Some(timestamp) = msg.get("timestamp_ms").and_then(serde_json::Value::as_i64) {
                doc = doc.with_metadata("timestamp_ms", timestamp);
            }

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loader for Telegram chat exports (JSON format).
///
/// Parses JSON export files from Telegram Desktop.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, TelegramLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TelegramLoader::new("result.json");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct TelegramLoader {
    file_path: PathBuf,
}

impl TelegramLoader {
    /// Create a new Telegram loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for TelegramLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let json: Value = serde_json::from_str(&content)?;

        let messages = json
            .get("messages")
            .and_then(|m| m.as_array())
            .ok_or_else(|| {
                crate::core::error::Error::InvalidInput(
                    "Expected 'messages' array in Telegram export".to_string(),
                )
            })?;

        let mut documents = Vec::new();

        for msg in messages {
            // Handle text messages
            let text = if let Some(t) = msg.get("text").and_then(|t| t.as_str()) {
                t.to_string()
            } else if let Some(text_arr) = msg.get("text").and_then(|t| t.as_array()) {
                // Handle text with entities (array of strings and objects)
                text_arr
                    .iter()
                    .filter_map(|item| {
                        if let Some(s) = item.as_str() {
                            Some(s.to_string())
                        } else if let Some(obj) = item.as_object() {
                            obj.get("text")
                                .and_then(|t| t.as_str())
                                .map(std::string::ToString::to_string)
                        } else {
                            None
                        }
                    })
                    .collect::<String>()
            } else {
                continue;
            };

            if text.is_empty() {
                continue;
            }

            let mut doc = Document::new(text)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "telegram");

            if let Some(msg_type) = msg.get("type").and_then(|t| t.as_str()) {
                doc = doc.with_metadata("message_type", msg_type.to_string());
            }

            if let Some(from) = msg.get("from").and_then(|f| f.as_str()) {
                doc = doc.with_metadata("from", from.to_string());
            }

            if let Some(date) = msg.get("date").and_then(|d| d.as_str()) {
                doc = doc.with_metadata("date", date.to_string());
            }

            documents.push(doc);
        }

        Ok(documents)
    }
}

/// Loader for `WhatsApp` chat exports (text format).
///
/// Parses `WhatsApp` chat export text files.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, WhatsAppChatLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = WhatsAppChatLoader::new("_chat.txt");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct WhatsAppChatLoader {
    file_path: PathBuf,
}

impl WhatsAppChatLoader {
    /// Create a new `WhatsApp` chat loader for the given file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            file_path: path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl DocumentLoader for WhatsAppChatLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let blob = Blob::from_path(&self.file_path);
        let content = blob.as_string()?;

        let mut documents = Vec::new();

        // WhatsApp format: [DD/MM/YYYY, HH:MM:SS] Sender: Message
        // or: DD/MM/YYYY, HH:MM - Sender: Message
        let re = regex::Regex::new(
            r"(?m)^[\[\[]?(\d{1,2}/\d{1,2}/\d{2,4}),?\s+(\d{1,2}:\d{2}(?::\d{2})?)[\]\]]?\s*-?\s*([^:]+):\s*(.+)$"
        ).expect("static regex pattern is valid");

        for cap in re.captures_iter(&content) {
            let date = cap.get(1).map_or("", |m| m.as_str());
            let time = cap.get(2).map_or("", |m| m.as_str());
            let sender = cap.get(3).map_or("", |m| m.as_str().trim());
            let message = cap.get(4).map_or("", |m| m.as_str());

            if message.is_empty() {
                continue;
            }

            let doc = Document::new(message)
                .with_metadata("source", self.file_path.display().to_string())
                .with_metadata("format", "whatsapp")
                .with_metadata("sender", sender.to_string())
                .with_metadata("date", date.to_string())
                .with_metadata("time", time.to_string());

            documents.push(doc);
        }

        if documents.is_empty() {
            return Err(crate::core::error::Error::InvalidInput(
                "No valid WhatsApp messages found in file".to_string(),
            ));
        }

        Ok(documents)
    }
}

/// Loader for Slack export chat logs.
///
/// Loads chat sessions from Slack export zip files. Processes JSON message files
/// and converts them to documents with sender and timestamp metadata.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, SlackChatLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SlackChatLoader::new("slack_export.zip");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct SlackChatLoader {
    zip_path: PathBuf,
}

impl SlackChatLoader {
    /// Create a new Slack chat loader for the given zip file path.
    pub fn new<P: AsRef<Path>>(path: P) -> Self {
        Self {
            zip_path: path.as_ref().to_path_buf(),
        }
    }

    fn load_single_chat_session(messages: Vec<serde_json::Value>) -> Result<Vec<Document>> {
        let mut documents = Vec::new();
        let mut previous_sender = None;
        let skip_pattern = regex::Regex::new(r"<@U\d+> has joined the channel")
            .map_err(|e| crate::core::error::Error::InvalidInput(format!("Regex error: {e}")))?;

        for message in messages {
            if !message.is_object() {
                continue;
            }

            let text = message
                .get("text")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let timestamp = message
                .get("ts")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let sender = message
                .get("user")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if sender.is_empty() {
                continue;
            }

            // Skip system messages about users joining
            if skip_pattern.is_match(&text) {
                continue;
            }

            // If same sender as previous message, append to last document
            if Some(&sender) == previous_sender.as_ref() && !documents.is_empty() {
                let last_doc: &mut Document =
                    documents.last_mut().expect("checked non-empty above");
                last_doc.page_content = format!("{}\n\n{}", last_doc.page_content, text);
                // Add timestamp to events array in metadata
                if let Some(events) = last_doc.metadata.get("events") {
                    if let Some(mut events_vec) = events.as_array().cloned() {
                        events_vec.push(serde_json::json!({"message_time": timestamp}));
                        last_doc
                            .metadata
                            .insert("events".to_string(), serde_json::Value::Array(events_vec));
                    }
                }
            } else {
                // New sender, create new document
                let doc = Document::new(text)
                    .with_metadata("sender", sender.clone())
                    .with_metadata("role", sender.clone())
                    .with_metadata("events", serde_json::json!([{"message_time": timestamp}]));
                documents.push(doc);
                previous_sender = Some(sender);
            }
        }

        Ok(documents)
    }
}

#[async_trait]
impl DocumentLoader for SlackChatLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Clone data for spawn_blocking (avoid blocking async runtime with std::fs)
        let zip_path = self.zip_path.clone();

        // Perform all filesystem I/O and zip parsing in spawn_blocking
        tokio::task::spawn_blocking(move || {
            use std::io::Read;

            if !zip_path.exists() {
                return Err(crate::core::error::Error::InvalidInput(format!(
                    "File {} not found",
                    zip_path.display()
                )));
            }

            let file =
                std::fs::File::open(&zip_path).map_err(crate::core::error::Error::Io)?;
            let mut archive = zip::ZipArchive::new(file).map_err(|e| {
                crate::core::error::Error::InvalidInput(format!("Invalid zip file: {e}"))
            })?;

            let mut all_documents = Vec::new();

            for i in 0..archive.len() {
                let mut file = archive.by_index(i).map_err(|e| {
                    crate::core::error::Error::InvalidInput(format!(
                        "Error reading zip entry: {e}"
                    ))
                })?;

                let file_name = file.name().to_string();

                // Only process JSON files
                if !file_name.ends_with(".json") {
                    continue;
                }

                let mut contents = String::new();
                file.read_to_string(&mut contents)
                    .map_err(crate::core::error::Error::Io)?;

                let messages: Vec<serde_json::Value> =
                    serde_json::from_str(&contents).map_err(|e| {
                        crate::core::error::Error::InvalidInput(format!(
                            "Invalid JSON in {file_name}: {e}"
                        ))
                    })?;

                let mut docs = SlackChatLoader::load_single_chat_session(messages)?;

                // Add source file metadata
                for doc in &mut docs {
                    doc.metadata.insert(
                        "source".to_string(),
                        serde_json::Value::String(zip_path.display().to_string()),
                    );
                    doc.metadata.insert(
                        "format".to_string(),
                        serde_json::Value::String("slack".to_string()),
                    );
                    doc.metadata.insert(
                        "channel_file".to_string(),
                        serde_json::Value::String(file_name.clone()),
                    );
                }

                all_documents.extend(docs);
            }

            if all_documents.is_empty() {
                return Err(crate::core::error::Error::InvalidInput(
                    "No valid Slack messages found in zip file".to_string(),
                ));
            }

            Ok::<Vec<Document>, crate::core::error::Error>(all_documents)
        })
        .await
        .map_err(|e| crate::core::error::Error::other(format!("Task join failed: {e}")))?
    }
}

/// Loader for Telegram chat history exports.
///
/// Loads Telegram chat messages from JSON or ZIP exports. Supports the
/// "Machine-readable JSON" export format from Telegram Desktop.
///
/// To export from Telegram Desktop:
/// 1. Select a conversation
/// 2. Click the three dots in the top right
/// 3. Select "Export chat history"
/// 4. Choose "Machine-readable JSON"
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, TelegramChatLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = TelegramChatLoader::new("telegram_export.json");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct TelegramChatLoader {
    path: PathBuf,
}

impl TelegramChatLoader {
    /// Create a new Telegram chat loader for the given path.
    ///
    /// The path can be:
    /// - A JSON file (result.json)
    /// - A ZIP file containing JSON files
    /// - A directory containing JSON files
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Load a single chat session from a JSON file's messages array.
    fn load_single_chat_session(messages: Vec<serde_json::Value>) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        for message in messages {
            let text = match message.get("text") {
                Some(serde_json::Value::String(s)) => s.clone(),
                Some(serde_json::Value::Array(parts)) => {
                    // Handle text as array of parts (can contain text and entities)
                    parts
                        .iter()
                        .filter_map(|part| {
                            if let serde_json::Value::String(s) = part {
                                Some(s.as_str())
                            } else if let Some(text) = part.get("text").and_then(|t| t.as_str()) {
                                Some(text)
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("")
                }
                _ => continue,
            };

            if text.is_empty() {
                continue;
            }

            let timestamp = message
                .get("date")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let sender = message
                .get("from")
                .and_then(|v| v.as_str())
                .unwrap_or("Deleted Account")
                .to_string();

            let doc = Document::new(text)
                .with_metadata("sender", sender.clone())
                .with_metadata("events", serde_json::json!([{"message_time": timestamp}]));

            documents.push(doc);
        }

        Ok(documents)
    }
}

#[async_trait]
impl DocumentLoader for TelegramChatLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        use std::io::Read;

        if !self.path.exists() {
            return Err(crate::core::error::Error::InvalidInput(format!(
                "Path {} not found",
                self.path.display()
            )));
        }

        let mut all_documents = Vec::new();

        // Handle different path types
        if self.path.is_file() {
            if let Some(ext) = self.path.extension() {
                match ext.to_str() {
                    Some("json") => {
                        // Load single JSON file
                        let contents = tokio::fs::read_to_string(&self.path)
                            .await
                            .map_err(crate::core::error::Error::Io)?;

                        let data: serde_json::Value =
                            serde_json::from_str(&contents).map_err(|e| {
                                crate::core::error::Error::InvalidInput(format!(
                                    "Invalid JSON: {e}"
                                ))
                            })?;

                        let messages = data
                            .get("messages")
                            .and_then(|m| m.as_array())
                            .ok_or_else(|| {
                                crate::core::error::Error::InvalidInput(
                                    "JSON file must have a 'messages' array".to_string(),
                                )
                            })?
                            .clone();

                        let mut docs = Self::load_single_chat_session(messages)?;

                        // Add metadata
                        for doc in &mut docs {
                            doc.metadata.insert(
                                "source".to_string(),
                                serde_json::Value::String(self.path.display().to_string()),
                            );
                            doc.metadata.insert(
                                "format".to_string(),
                                serde_json::Value::String("telegram_json".to_string()),
                            );
                        }

                        all_documents.extend(docs);
                    }
                    Some("zip") => {
                        // Load ZIP file in spawn_blocking to avoid blocking async runtime
                        // with std::fs::File::open and zip archive I/O
                        let path = self.path.clone();
                        let docs = tokio::task::spawn_blocking(move || {
                            let file = std::fs::File::open(&path)
                                .map_err(crate::core::error::Error::Io)?;
                            let mut archive = zip::ZipArchive::new(file).map_err(|e| {
                                crate::core::error::Error::InvalidInput(format!(
                                    "Invalid zip file: {e}"
                                ))
                            })?;

                            let mut zip_documents = Vec::new();
                            let path_display = path.display().to_string();

                            for i in 0..archive.len() {
                                let mut file = archive.by_index(i).map_err(|e| {
                                    crate::core::error::Error::InvalidInput(format!(
                                        "Error reading zip entry: {e}"
                                    ))
                                })?;

                                let file_name = file.name().to_string();

                                // Only process JSON files
                                if !file_name.ends_with(".json") {
                                    continue;
                                }

                                let mut contents = String::new();
                                file.read_to_string(&mut contents)
                                    .map_err(crate::core::error::Error::Io)?;

                                let data: serde_json::Value =
                                    serde_json::from_str(&contents).map_err(|e| {
                                        crate::core::error::Error::InvalidInput(format!(
                                            "Invalid JSON in {file_name}: {e}"
                                        ))
                                    })?;

                                if let Some(messages) =
                                    data.get("messages").and_then(|m| m.as_array())
                                {
                                    let mut docs =
                                        TelegramChatLoader::load_single_chat_session(messages.clone())?;

                                    for doc in &mut docs {
                                        doc.metadata.insert(
                                            "source".to_string(),
                                            serde_json::Value::String(path_display.clone()),
                                        );
                                        doc.metadata.insert(
                                            "format".to_string(),
                                            serde_json::Value::String("telegram_json".to_string()),
                                        );
                                        doc.metadata.insert(
                                            "source_file".to_string(),
                                            serde_json::Value::String(file_name.clone()),
                                        );
                                    }

                                    zip_documents.extend(docs);
                                }
                            }
                            Ok::<_, crate::core::error::Error>(zip_documents)
                        })
                        .await
                        .map_err(|e| {
                            crate::core::error::Error::Other(format!(
                                "spawn_blocking panicked: {e}"
                            ))
                        })??;

                        all_documents.extend(docs);
                    }
                    _ => {
                        return Err(crate::core::error::Error::InvalidInput(format!(
                            "Unsupported file type: {ext:?}"
                        )));
                    }
                }
            }
        } else if self.path.is_dir() {
            // Load all JSON files in directory
            let mut entries = tokio::fs::read_dir(&self.path)
                .await
                .map_err(crate::core::error::Error::Io)?;

            while let Some(entry) = entries
                .next_entry()
                .await
                .map_err(crate::core::error::Error::Io)?
            {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    let contents = tokio::fs::read_to_string(&path)
                        .await
                        .map_err(crate::core::error::Error::Io)?;

                    if let Ok(data) = serde_json::from_str::<serde_json::Value>(&contents) {
                        if let Some(messages) = data.get("messages").and_then(|m| m.as_array()) {
                            let mut docs = Self::load_single_chat_session(messages.clone())?;

                            for doc in &mut docs {
                                doc.metadata.insert(
                                    "source".to_string(),
                                    serde_json::Value::String(path.display().to_string()),
                                );
                                doc.metadata.insert(
                                    "format".to_string(),
                                    serde_json::Value::String("telegram_json".to_string()),
                                );
                            }

                            all_documents.extend(docs);
                        }
                    }
                }
            }
        }

        if all_documents.is_empty() {
            return Err(crate::core::error::Error::InvalidInput(
                "No valid Telegram messages found".to_string(),
            ));
        }

        Ok(all_documents)
    }
}

/// Loader for iMessage chat history from macOS chat.db `SQLite` database.
///
/// Loads chat messages from the iMessage chat.db `SQLite` file. This loader
/// only works on macOS when you have iMessage enabled and can access the chat.db file.
///
/// The chat.db file is typically located at `~/Library/Messages/chat.db`. However,
/// your terminal may not have permission to access this file. To resolve this:
/// - Copy the file to a different location, or
/// - Change the permissions of the file, or
/// - Grant Full Disk Access to your terminal emulator in
///   System Settings > Security and Privacy > Full Disk Access.
///
/// # Example
///
/// ```no_run
/// use dashflow::core::document_loaders::{DocumentLoader, IMessageChatLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Use default path
/// let loader = IMessageChatLoader::new(None::<&str>)?;
/// let documents = loader.load().await?;
///
/// // Or specify custom path
/// let loader = IMessageChatLoader::new(Some("/path/to/chat.db"))?;
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct IMessageChatLoader {
    db_path: PathBuf,
}

impl IMessageChatLoader {
    /// Create a new iMessage chat loader.
    ///
    /// If path is None, uses the default macOS location: ~/Library/Messages/chat.db
    pub fn new(path: Option<impl Into<PathBuf>>) -> Result<Self> {
        let db_path = if let Some(p) = path {
            p.into()
        } else {
            let home = env_string(HOME).ok_or_else(|| {
                Error::InvalidInput("HOME environment variable not set".to_string())
            })?;
            PathBuf::from(home)
                .join("Library")
                .join("Messages")
                .join("chat.db")
        };

        if !db_path.exists() {
            return Err(Error::InvalidInput(format!(
                "File {} not found",
                db_path.display()
            )));
        }

        Ok(Self { db_path })
    }

    /// Parse the attributedBody field of the message table for text content.
    ///
    /// The attributedBody field is a binary blob that contains the message content
    /// after the byte string b"NSString":
    ///
    /// ```text
    ///                       5 bytes      1-3 bytes    `len` bytes
    /// ... | b"NSString" |   preamble   |   `len`   |    contents    | ...
    /// ```
    ///
    /// The 5 preamble bytes are always `\x01\x94\x84\x01+`
    ///
    /// The size of `len` is either 1 byte or 3 bytes:
    /// - If the first byte in `len` is `\x81` then `len` is 3 bytes long.
    ///   The message length is the 2 bytes after, in little endian.
    /// - Otherwise, the size of `len` is 1 byte, and the message length is that byte.
    fn parse_attributed_body(attributed_body: &[u8]) -> Result<String> {
        // Find b"NSString" in the attributed body
        let ns_string = b"NSString";
        let pos = attributed_body
            .windows(ns_string.len())
            .position(|window| window == ns_string)
            .ok_or_else(|| {
                Error::InvalidInput("NSString not found in attributedBody".to_string())
            })?;

        // Skip "NSString" (8 bytes) and preamble (5 bytes)
        let content_start = pos + ns_string.len() + 5;
        if content_start >= attributed_body.len() {
            return Err(Error::InvalidInput("attributedBody too short".to_string()));
        }

        let content = &attributed_body[content_start..];
        if content.is_empty() {
            return Err(Error::InvalidInput("Empty content".to_string()));
        }

        let (length, start) = if content[0] == 129 {
            // 0x81 - length is 3 bytes, little endian in next 2 bytes
            if content.len() < 3 {
                return Err(Error::InvalidInput(
                    "Insufficient bytes for length".to_string(),
                ));
            }
            let len = u16::from_le_bytes([content[1], content[2]]) as usize;
            (len, 3)
        } else {
            // Length is just the first byte
            (content[0] as usize, 1)
        };

        let end = start + length;
        if end > content.len() {
            return Err(Error::InvalidInput(format!(
                "Content length {} exceeds available bytes {}",
                end,
                content.len()
            )));
        }

        String::from_utf8(content[start..end].to_vec())
            .map_err(|e| Error::InvalidInput(format!("Invalid UTF-8: {e}")))
    }

    /// Convert nanoseconds since 2001-01-01 to Unix timestamp in seconds.
    ///
    /// iMessage stores timestamps as nanoseconds since 2001-01-01 00:00:00 UTC.
    fn nanoseconds_from_2001_to_unix_seconds(nanoseconds: i64) -> i64 {
        // Convert nanoseconds to seconds
        let timestamp_in_seconds = nanoseconds / 1_000_000_000;

        // Reference date: January 1, 2001, 00:00:00 UTC in Unix time
        // Unix epoch is 1970-01-01, 2001-01-01 is 31 years later
        let reference_date_seconds = 978307200; // 2001-01-01 00:00:00 UTC

        // Calculate actual Unix timestamp
        reference_date_seconds + timestamp_in_seconds
    }

    /// Get the SQL query for fetching messages, with conditional join logic.
    ///
    /// Messages sent pre macOS 12 require a join through the `chat_handle_join` table.
    /// However, this table doesn't exist if the database was created with macOS 12 or above.
    fn get_session_query(use_chat_handle_table: bool) -> String {
        let joins = if use_chat_handle_table {
            r"
            JOIN chat_handle_join ON
                 chat_message_join.chat_id = chat_handle_join.chat_id
            JOIN handle ON
                 handle.ROWID = chat_handle_join.handle_id"
        } else {
            r"
            JOIN handle ON message.handle_id = handle.ROWID
        "
        };

        format!(
            r"
            SELECT  message.date,
                    handle.id,
                    message.text,
                    message.is_from_me,
                    message.attributedBody
            FROM message
            JOIN chat_message_join ON
                 message.ROWID = chat_message_join.message_id
            {joins}
            WHERE chat_message_join.chat_id = ?
            ORDER BY message.date ASC;
        "
        )
    }

    /// Load a single chat session from the iMessage database.
    fn load_single_chat_session(
        conn: &rusqlite::Connection,
        use_chat_handle_table: bool,
        chat_id: i64,
        source_path: &Path,
    ) -> Result<Vec<Document>> {
        let query = Self::get_session_query(use_chat_handle_table);
        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| Error::InvalidInput(format!("Failed to prepare query: {e}")))?;

        let messages = stmt
            .query_map([chat_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,             // date
                    row.get::<_, String>(1)?,          // handle.id (sender)
                    row.get::<_, Option<String>>(2)?,  // text
                    row.get::<_, i32>(3)?,             // is_from_me
                    row.get::<_, Option<Vec<u8>>>(4)?, // attributedBody
                ))
            })
            .map_err(|e| Error::InvalidInput(format!("Failed to query messages: {e}")))?;

        let mut documents = Vec::new();

        for message in messages {
            let (date, sender, text, is_from_me, attributed_body) = message
                .map_err(|e| Error::InvalidInput(format!("Failed to read message row: {e}")))?;

            // Get content from either text field or attributedBody
            let content = if let Some(text) = text {
                text
            } else if let Some(body) = attributed_body {
                match Self::parse_attributed_body(&body) {
                    Ok(text) => text,
                    Err(_) => continue, // Skip messages we can't parse
                }
            } else {
                continue; // Skip messages with no content
            };

            if content.is_empty() {
                continue;
            }

            // Convert timestamp
            let unix_timestamp = Self::nanoseconds_from_2001_to_unix_seconds(date);

            let doc = Document::new(content)
                .with_metadata("sender", sender.clone())
                .with_metadata("is_from_me", is_from_me != 0)
                .with_metadata("message_time", date)
                .with_metadata("message_time_unix", unix_timestamp)
                .with_metadata("chat_id", chat_id)
                .with_metadata("source", source_path.display().to_string())
                .with_metadata("format", "imessage");

            documents.push(doc);
        }

        Ok(documents)
    }
}

#[async_trait]
impl DocumentLoader for IMessageChatLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        // Open SQLite connection
        let conn = rusqlite::Connection::open(&self.db_path).map_err(|e| {
            Error::InvalidInput(format!(
                "Could not open iMessage DB file {}. \
                Make sure your terminal emulator has disk access to this file. \
                You can either copy the DB file to an accessible location \
                or grant full disk access for your terminal emulator in \
                System Settings > Security and Privacy > Full Disk Access. \
                Error: {}",
                self.db_path.display(),
                e
            ))
        })?;

        // Check if chat_handle_join table exists (for pre-macOS 12 compatibility)
        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='chat_handle_join';",
            )
            .map_err(|e| Error::InvalidInput(format!("Failed to query sqlite_master: {e}")))?;

        let has_chat_handle_join = stmt
            .exists([])
            .map_err(|e| Error::InvalidInput(format!("Failed to check table existence: {e}")))?;

        // Fetch the list of chat IDs sorted by time (most recent first)
        let mut stmt = conn
            .prepare(
                r"SELECT chat_id
                FROM message
                JOIN chat_message_join ON message.ROWID = chat_message_join.message_id
                GROUP BY chat_id
                ORDER BY MAX(date) DESC;",
            )
            .map_err(|e| Error::InvalidInput(format!("Failed to prepare chat_id query: {e}")))?;

        let chat_ids: Vec<i64> = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| Error::InvalidInput(format!("Failed to query chat IDs: {e}")))?
            .collect::<std::result::Result<Vec<i64>, _>>()
            .map_err(|e| Error::InvalidInput(format!("Failed to collect chat IDs: {e}")))?;

        let mut all_documents = Vec::new();

        for chat_id in chat_ids {
            let docs = Self::load_single_chat_session(
                &conn,
                has_chat_handle_join,
                chat_id,
                &self.db_path,
            )?;
            all_documents.extend(docs);
        }

        if all_documents.is_empty() {
            return Err(Error::InvalidInput(
                "No valid iMessage chats found in database".to_string(),
            ));
        }

        Ok(all_documents)
    }
}

/// Loader for Slack workspace export data.
///
/// Loads messages from Slack workspace export JSON files. Slack exports are
/// organized by channel with one JSON file per day.
///
/// # Example
///
/// ```rust,no_run
/// use dashflow::core::document_loaders::{DocumentLoader, SlackExportLoader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let loader = SlackExportLoader::new("/path/to/slack/export")
///     .with_channel("general");
/// let documents = loader.load().await?;
/// # Ok(())
/// # }
/// ```
pub struct SlackExportLoader {
    export_path: PathBuf,
    channel: Option<String>,
}

impl SlackExportLoader {
    /// Create a new Slack export loader for the given export directory.
    pub fn new(export_path: impl Into<PathBuf>) -> Self {
        Self {
            export_path: export_path.into(),
            channel: None,
        }
    }

    /// Load only messages from a specific channel.
    #[must_use]
    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }
}

#[async_trait]
impl DocumentLoader for SlackExportLoader {
    async fn load(&self) -> Result<Vec<Document>> {
        let mut documents = Vec::new();

        // Determine which directories to process
        let channels = if let Some(ref channel) = self.channel {
            vec![self.export_path.join(channel)]
        } else {
            // Load all channels
            fs::read_dir(&self.export_path)?
                .filter_map(std::result::Result::ok)
                .filter(|e| e.path().is_dir())
                .map(|e| e.path())
                .collect()
        };

        // Process each channel directory
        for channel_dir in channels {
            let channel_name = channel_dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            // Skip non-channel directories
            if channel_name.starts_with('.') {
                continue;
            }

            // Process each JSON file in the channel
            for entry in fs::read_dir(&channel_dir)?.filter_map(std::result::Result::ok) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }

                let content = fs::read_to_string(&path)?;
                let messages: Vec<serde_json::Value> = serde_json::from_str(&content)?;

                // Process each message
                for msg in messages {
                    if let Some(text) = msg.get("text").and_then(|v| v.as_str()) {
                        let user = msg
                            .get("user")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");

                        let timestamp = msg.get("ts").and_then(|v| v.as_str()).unwrap_or("");

                        let doc = Document::new(text.to_string())
                            .with_metadata("source", channel_name.clone())
                            .with_metadata("channel", channel_name.clone())
                            .with_metadata("user", user.to_string())
                            .with_metadata("timestamp", timestamp.to_string())
                            .with_metadata("format", "slack");

                        documents.push(doc);
                    }
                }
            }
        }

        Ok(documents)
    }
}

// NOTE: MicrosoftTeamsLoader was removed (placeholder implementation).
// See git history for implementation notes if it needs to be added.

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    // ==================== DiscordLoader Tests ====================

    #[tokio::test]
    async fn test_discord_loader_basic() {
        let discord_json = r#"{
            "messages": [
                {
                    "content": "Hello everyone!",
                    "author": {"id": "123", "username": "alice"},
                    "timestamp": "2024-01-01T12:00:00Z"
                },
                {
                    "content": "Hi Alice!",
                    "author": {"id": "456", "username": "bob"},
                    "timestamp": "2024-01-01T12:01:00Z"
                }
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(discord_json.as_bytes()).unwrap();

        let loader = DiscordLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Hello everyone!");
        assert_eq!(docs[0].metadata.get("author").unwrap(), "alice");
        assert_eq!(docs[0].metadata.get("user_id").unwrap(), "123");
        assert_eq!(docs[1].page_content, "Hi Alice!");
        assert_eq!(docs[1].metadata.get("author").unwrap(), "bob");
    }

    #[tokio::test]
    async fn test_discord_loader_with_user_filter() {
        let discord_json = r#"{
            "messages": [
                {"content": "Message from Alice", "author": {"id": "123", "username": "alice"}},
                {"content": "Message from Bob", "author": {"id": "456", "username": "bob"}},
                {"content": "Another from Alice", "author": {"id": "123", "username": "alice"}}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(discord_json.as_bytes()).unwrap();

        let loader = DiscordLoader::new(file.path()).with_user_filter("123".to_string());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs
            .iter()
            .all(|d| d.metadata.get("user_id").unwrap() == "123"));
    }

    #[tokio::test]
    async fn test_discord_loader_array_format() {
        // Discord can also export as a plain array of messages
        let discord_json = r#"[
            {"content": "Message 1", "author": {"id": "123", "username": "user1"}},
            {"content": "Message 2", "author": {"id": "456", "username": "user2"}}
        ]"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(discord_json.as_bytes()).unwrap();

        let loader = DiscordLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_discord_loader_skips_empty_content() {
        let discord_json = r#"{
            "messages": [
                {"content": "", "author": {"id": "123", "username": "user1"}},
                {"content": "Valid message", "author": {"id": "456", "username": "user2"}}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(discord_json.as_bytes()).unwrap();

        let loader = DiscordLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Valid message");
    }

    // ==================== FacebookChatLoader Tests ====================

    #[tokio::test]
    async fn test_facebook_loader_basic() {
        let fb_json = r#"{
            "messages": [
                {"content": "Hey there!", "sender_name": "Alice", "timestamp_ms": 1704067200000},
                {"content": "Hello!", "sender_name": "Bob", "timestamp_ms": 1704067260000}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(fb_json.as_bytes()).unwrap();

        let loader = FacebookChatLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Hey there!");
        assert_eq!(docs[0].metadata.get("sender").unwrap(), "Alice");
        assert_eq!(
            docs[0].metadata.get("format").unwrap(),
            "facebook_messenger"
        );
    }

    #[tokio::test]
    async fn test_facebook_loader_with_user_filter() {
        let fb_json = r#"{
            "messages": [
                {"content": "From Alice", "sender_name": "Alice"},
                {"content": "From Bob", "sender_name": "Bob"},
                {"content": "Also Alice", "sender_name": "Alice"}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(fb_json.as_bytes()).unwrap();

        let loader = FacebookChatLoader::new(file.path()).with_user_filter("Alice".to_string());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs
            .iter()
            .all(|d| d.metadata.get("sender").unwrap() == "Alice"));
    }

    // ==================== TelegramLoader Tests ====================

    #[tokio::test]
    async fn test_telegram_loader_basic() {
        let tg_json = r#"{
            "messages": [
                {"type": "message", "text": "Hello from Telegram!", "from": "Alice", "date": "2024-01-01T12:00:00"},
                {"type": "message", "text": "Hi Alice!", "from": "Bob", "date": "2024-01-01T12:01:00"}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tg_json.as_bytes()).unwrap();

        let loader = TelegramLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Hello from Telegram!");
        assert_eq!(docs[0].metadata.get("from").unwrap(), "Alice");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "telegram");
    }

    #[tokio::test]
    async fn test_telegram_loader_with_text_entities() {
        // Telegram can have text as an array of strings and objects with entities
        let tg_json = r#"{
            "messages": [
                {"type": "message", "text": ["Hello ", {"type": "bold", "text": "world"}, "!"], "from": "User"}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tg_json.as_bytes()).unwrap();

        let loader = TelegramLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Hello world!");
    }

    #[tokio::test]
    async fn test_telegram_loader_skips_empty_messages() {
        let tg_json = r#"{
            "messages": [
                {"type": "message", "text": "", "from": "User"},
                {"type": "message", "text": "Valid message", "from": "User"}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tg_json.as_bytes()).unwrap();

        let loader = TelegramLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Valid message");
    }

    // ==================== WhatsAppChatLoader Tests ====================

    #[tokio::test]
    async fn test_whatsapp_loader_basic() {
        let wa_text = "[01/01/2024, 12:00:00] Alice: Hello from WhatsApp!
[01/01/2024, 12:01:00] Bob: Hi Alice!
[01/01/2024, 12:02:00] Alice: How are you?";

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(wa_text.as_bytes()).unwrap();

        let loader = WhatsAppChatLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "Hello from WhatsApp!");
        assert_eq!(docs[0].metadata.get("sender").unwrap(), "Alice");
        assert_eq!(docs[0].metadata.get("date").unwrap(), "01/01/2024");
        assert_eq!(docs[0].metadata.get("format").unwrap(), "whatsapp");
    }

    #[tokio::test]
    async fn test_whatsapp_loader_alternative_format() {
        // WhatsApp can also use format without brackets: DD/MM/YYYY, HH:MM - Sender: Message
        let wa_text = "01/01/2024, 12:00 - Alice: Hello!
01/01/2024, 12:01 - Bob: Hi there!";

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(wa_text.as_bytes()).unwrap();

        let loader = WhatsAppChatLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_whatsapp_loader_empty_file_error() {
        let wa_text = "This is not a valid WhatsApp export format";

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(wa_text.as_bytes()).unwrap();

        let loader = WhatsAppChatLoader::new(file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    // ==================== SlackChatLoader Tests ====================

    #[tokio::test]
    async fn test_slack_load_single_chat_session() {
        let messages = vec![
            serde_json::json!({"text": "Hello team!", "user": "U123", "ts": "1704067200.000"}),
            serde_json::json!({"text": "Hi!", "user": "U456", "ts": "1704067260.000"}),
        ];

        let docs = SlackChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 2);
        assert_eq!(docs[0].page_content, "Hello team!");
        assert_eq!(docs[0].metadata.get("sender").unwrap(), "U123");
    }

    #[tokio::test]
    async fn test_slack_merges_consecutive_messages_from_same_sender() {
        let messages = vec![
            serde_json::json!({"text": "First message", "user": "U123", "ts": "1704067200.000"}),
            serde_json::json!({"text": "Second message", "user": "U123", "ts": "1704067201.000"}),
            serde_json::json!({"text": "Different sender", "user": "U456", "ts": "1704067260.000"}),
        ];

        let docs = SlackChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 2);
        assert!(docs[0].page_content.contains("First message"));
        assert!(docs[0].page_content.contains("Second message"));
        assert_eq!(docs[1].page_content, "Different sender");
    }

    #[tokio::test]
    async fn test_slack_skips_join_messages() {
        let messages = vec![
            serde_json::json!({"text": "<@U123> has joined the channel", "user": "U123", "ts": "1704067200.000"}),
            serde_json::json!({"text": "Regular message", "user": "U456", "ts": "1704067260.000"}),
        ];

        let docs = SlackChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Regular message");
    }

    #[tokio::test]
    async fn test_slack_skips_messages_without_user() {
        let messages = vec![
            serde_json::json!({"text": "No user field", "ts": "1704067200.000"}),
            serde_json::json!({"text": "Has user", "user": "U456", "ts": "1704067260.000"}),
        ];

        let docs = SlackChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 1);
        assert_eq!(docs[0].page_content, "Has user");
    }

    // ==================== TelegramChatLoader Tests ====================

    #[tokio::test]
    async fn test_telegram_chat_loader_load_single_session() {
        let messages = vec![
            serde_json::json!({"type": "message", "text": "Hello!", "from": "Alice", "date": "2024-01-01T12:00:00"}),
            serde_json::json!({"type": "message", "text": "Hi!", "from": "Bob", "date": "2024-01-01T12:01:00"}),
        ];

        let docs = TelegramChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 2);
    }

    #[tokio::test]
    async fn test_telegram_chat_loader_preserves_individual_messages() {
        // TelegramChatLoader does NOT merge messages from same sender
        // Each message is a separate document
        let messages = vec![
            serde_json::json!({"type": "message", "text": "First", "from": "Alice", "date": "2024-01-01T12:00:00"}),
            serde_json::json!({"type": "message", "text": "Second", "from": "Alice", "date": "2024-01-01T12:00:01"}),
            serde_json::json!({"type": "message", "text": "From Bob", "from": "Bob", "date": "2024-01-01T12:01:00"}),
        ];

        let docs = TelegramChatLoader::load_single_chat_session(messages).unwrap();

        assert_eq!(docs.len(), 3);
        assert_eq!(docs[0].page_content, "First");
        assert_eq!(docs[1].page_content, "Second");
        assert_eq!(docs[2].page_content, "From Bob");
    }

    // ==================== SlackExportLoader Tests ====================

    #[tokio::test]
    async fn test_slack_export_loader_creation() {
        let loader = SlackExportLoader::new("/path/to/export");
        assert!(loader.channel.is_none());

        let loader_with_channel = SlackExportLoader::new("/path/to/export").with_channel("general");
        assert_eq!(loader_with_channel.channel.as_deref(), Some("general"));
    }

    // ==================== Error Handling Tests ====================

    #[tokio::test]
    async fn test_discord_loader_invalid_json() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"not valid json").unwrap();

        let loader = DiscordLoader::new(file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_facebook_loader_missing_messages_array() {
        let fb_json = r#"{"other_field": "value"}"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(fb_json.as_bytes()).unwrap();

        let loader = FacebookChatLoader::new(file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_telegram_loader_missing_messages_array() {
        let tg_json = r#"{"other_field": "value"}"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(tg_json.as_bytes()).unwrap();

        let loader = TelegramLoader::new(file.path());
        let result = loader.load().await;

        assert!(result.is_err());
    }

    // ==================== Metadata Tests ====================

    #[tokio::test]
    async fn test_discord_loader_metadata_complete() {
        let discord_json = r#"{
            "messages": [
                {
                    "content": "Test message",
                    "author": {"id": "123", "username": "testuser"},
                    "timestamp": "2024-01-01T12:00:00Z"
                }
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(discord_json.as_bytes()).unwrap();

        let loader = DiscordLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert_eq!(docs.len(), 1);
        assert!(docs[0].metadata.contains_key("source"));
        assert!(docs[0].metadata.contains_key("format"));
        assert!(docs[0].metadata.contains_key("author"));
        assert!(docs[0].metadata.contains_key("user_id"));
        assert!(docs[0].metadata.contains_key("timestamp"));
    }

    #[tokio::test]
    async fn test_facebook_loader_timestamp_metadata() {
        let fb_json = r#"{
            "messages": [
                {"content": "Test", "sender_name": "User", "timestamp_ms": 1704067200000}
            ]
        }"#;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(fb_json.as_bytes()).unwrap();

        let loader = FacebookChatLoader::new(file.path());
        let docs = loader.load().await.unwrap();

        assert!(docs[0].metadata.contains_key("timestamp_ms"));
    }
}
