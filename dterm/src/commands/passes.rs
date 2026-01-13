use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::clipboard::ClipboardProvider;
use crate::telemetry::{TelemetryEvent, TelemetrySink};

pub const TENGU_GUEST_PASSES_VIEW: &str = "tengu_guest_passes_view";
pub const TENGU_GUEST_PASSES_COPY_REFERRAL: &str = "tengu_guest_passes_copy_referral";
pub const TENGU_GUEST_PASSES_COPY_FAILED: &str = "tengu_guest_passes_copy_failed";

#[derive(Debug, Serialize, Deserialize, Clone)]
struct GuestPassesStore {
    count: u32,
    referral_url: String,
}

impl GuestPassesStore {
    fn default_with_new_referral() -> Self {
        let code = uuid::Uuid::new_v4().to_string();
        Self {
            count: 0,
            referral_url: format!("https://dterm.ai/ref/{code}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestPassesResult {
    pub count: u32,
    pub referral_url: String,
    pub copied: bool,
}

pub struct GuestPassesCommand {
    store_path: PathBuf,
}

impl GuestPassesCommand {
    pub fn new(store_path: PathBuf) -> Self {
        Self { store_path }
    }

    pub fn run(
        &self,
        copy_referral: bool,
        clipboard: &mut dyn ClipboardProvider,
        telemetry: &dyn TelemetrySink,
    ) -> Result<GuestPassesResult> {
        let store = load_or_init_store(&self.store_path)?;
        telemetry.emit(
            TelemetryEvent::new(TENGU_GUEST_PASSES_VIEW)
                .with_field("count", store.count.to_string()),
        );

        let mut copied = false;
        if copy_referral {
            match clipboard.set_text(&store.referral_url) {
                Ok(()) => {
                    copied = true;
                    telemetry.emit(
                        TelemetryEvent::new(TENGU_GUEST_PASSES_COPY_REFERRAL)
                            .with_field("referral_url", store.referral_url.clone()),
                    );
                }
                Err(err) => {
                    telemetry.emit(
                        TelemetryEvent::new(TENGU_GUEST_PASSES_COPY_FAILED)
                            .with_field("error", err.to_string()),
                    );
                    return Err(err);
                }
            }
        }

        Ok(GuestPassesResult {
            count: store.count,
            referral_url: store.referral_url,
            copied,
        })
    }
}

fn load_or_init_store(path: &Path) -> Result<GuestPassesStore> {
    if path.exists() {
        let contents = fs::read_to_string(path)?;
        let store: GuestPassesStore = toml::from_str(&contents)?;
        return Ok(store);
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let store = GuestPassesStore::default_with_new_referral();
    let contents = toml::to_string_pretty(&store)?;
    fs::write(path, contents)?;
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    use crate::clipboard::TestClipboard;
    use crate::telemetry::VecTelemetry;

    #[test]
    fn passes_command_reports_count_and_referral() {
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join("guest_passes.toml");
        let store = GuestPassesStore {
            count: 3,
            referral_url: "https://example.com/ref".to_string(),
        };
        fs::write(&store_path, toml::to_string_pretty(&store).unwrap()).unwrap();

        let command = GuestPassesCommand::new(store_path);
        let telemetry = VecTelemetry::new();
        let mut clipboard = TestClipboard::default();

        let result = command.run(false, &mut clipboard, &telemetry).unwrap();
        assert_eq!(result.count, 3);
        assert_eq!(result.referral_url, "https://example.com/ref");
        assert!(!result.copied);

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_GUEST_PASSES_VIEW));
    }

    #[test]
    fn passes_command_copies_referral() {
        let tmp = TempDir::new().unwrap();
        let store_path = tmp.path().join("guest_passes.toml");
        let store = GuestPassesStore {
            count: 1,
            referral_url: "https://example.com/ref".to_string(),
        };
        fs::write(&store_path, toml::to_string_pretty(&store).unwrap()).unwrap();

        let command = GuestPassesCommand::new(store_path);
        let telemetry = VecTelemetry::new();
        let mut clipboard = TestClipboard::default();

        let result = command.run(true, &mut clipboard, &telemetry).unwrap();
        assert!(result.copied);
        assert_eq!(
            clipboard.last_text.as_deref(),
            Some("https://example.com/ref")
        );

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_GUEST_PASSES_COPY_REFERRAL));
    }
}
