use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

use crate::telemetry::{TelemetryEvent, TelemetrySink};

pub const TENGU_NATIVE_AUTO_UPDATER_START: &str = "tengu_native_auto_updater_start";
pub const TENGU_NATIVE_AUTO_UPDATER_SUCCESS: &str = "tengu_native_auto_updater_success";
pub const TENGU_NATIVE_AUTO_UPDATER_FAIL: &str = "tengu_native_auto_updater_fail";

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum UpdateStatus {
    InProgress,
    Succeeded,
    Failed,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateLock {
    pub target_version: String,
    pub status: UpdateStatus,
    pub last_error: Option<String>,
    pub updated_at_epoch_ms: u64,
}

impl UpdateLock {
    fn new(target_version: String, status: UpdateStatus, last_error: Option<String>) -> Self {
        Self {
            target_version,
            status,
            last_error,
            updated_at_epoch_ms: now_epoch_ms(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NativeUpdateConfig {
    pub target_version: String,
    pub download_url: String,
    pub install_path: PathBuf,
    pub data_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeUpdateResult {
    pub installed_path: PathBuf,
    pub lock_path: PathBuf,
}

#[async_trait]
pub trait Downloader: Send + Sync {
    async fn download_to(&self, url: &str, dest: &Path) -> Result<()>;
}

pub struct ReqwestDownloader {
    client: reqwest::Client,
}

impl ReqwestDownloader {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Downloader for ReqwestDownloader {
    async fn download_to(&self, url: &str, dest: &Path) -> Result<()> {
        let response = self.client.get(url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;
        let mut file = tokio::fs::File::create(dest).await?;
        file.write_all(&bytes).await?;
        file.flush().await?;
        Ok(())
    }
}

pub async fn run_native_update(
    config: NativeUpdateConfig,
    downloader: &dyn Downloader,
    telemetry: &dyn TelemetrySink,
) -> Result<NativeUpdateResult> {
    telemetry.emit(
        TelemetryEvent::new(TENGU_NATIVE_AUTO_UPDATER_START)
            .with_field("target_version", config.target_version.clone()),
    );

    fs::create_dir_all(&config.data_dir)?;
    let lock_path = config.data_dir.join("update.lock");
    write_lock(
        &lock_path,
        UpdateLock::new(
            config.target_version.clone(),
            UpdateStatus::InProgress,
            None,
        ),
    )?;

    let staging_dir = create_staging_dir(&config.data_dir)?;
    let binary_name = install_filename(&config.install_path);
    let staged_binary = staging_dir.join(&binary_name);

    let update_result = async {
        downloader
            .download_to(&config.download_url, &staged_binary)
            .await?;
        ensure_downloaded(&staged_binary)?;
        copy_permissions(&config.install_path, &staged_binary)?;
        install_binary(&config.install_path, &staged_binary)?;
        Ok::<(), anyhow::Error>(())
    }
    .await;

    if let Err(err) = update_result {
        record_failure(&lock_path, &config.target_version, telemetry, &err)?;
        cleanup_staging_dir(&staging_dir);
        return Err(err);
    }

    write_lock(
        &lock_path,
        UpdateLock::new(config.target_version.clone(), UpdateStatus::Succeeded, None),
    )?;

    cleanup_staging_dir(&staging_dir);

    telemetry.emit(
        TelemetryEvent::new(TENGU_NATIVE_AUTO_UPDATER_SUCCESS)
            .with_field("target_version", config.target_version.clone()),
    );

    Ok(NativeUpdateResult {
        installed_path: config.install_path,
        lock_path,
    })
}

fn install_filename(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "dterm".to_string())
}

fn create_staging_dir(base: &Path) -> Result<PathBuf> {
    let dir = base.join("staging").join(uuid::Uuid::new_v4().to_string());
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn cleanup_staging_dir(dir: &Path) {
    let _ = fs::remove_dir_all(dir);
}

fn ensure_downloaded(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)?;
    if metadata.len() == 0 {
        return Err(anyhow!("downloaded binary is empty"));
    }
    Ok(())
}

fn copy_permissions(original: &Path, staged: &Path) -> Result<()> {
    if let Ok(metadata) = fs::metadata(original) {
        let perms = metadata.permissions();
        fs::set_permissions(staged, perms)?;
        return Ok(());
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::Permissions::from_mode(0o755);
        fs::set_permissions(staged, perms)?;
    }

    Ok(())
}

fn install_binary(install_path: &Path, staged: &Path) -> Result<()> {
    let backup_path = install_path.with_extension("bak");
    if install_path.exists() {
        if backup_path.exists() {
            fs::remove_file(&backup_path)?;
        }
        fs::rename(install_path, &backup_path)?;
    }

    fs::rename(staged, install_path).inspect_err(|_| {
        let _ = fs::rename(&backup_path, install_path);
    })?;

    Ok(())
}

fn write_lock(path: &Path, lock: UpdateLock) -> Result<()> {
    let contents = toml::to_string_pretty(&lock)?;
    fs::write(path, contents)?;
    Ok(())
}

fn record_failure(
    path: &Path,
    target_version: &str,
    telemetry: &dyn TelemetrySink,
    err: &anyhow::Error,
) -> Result<()> {
    let lock = UpdateLock::new(
        target_version.to_string(),
        UpdateStatus::Failed,
        Some(err.to_string()),
    );
    write_lock(path, lock)?;
    telemetry.emit(
        TelemetryEvent::new(TENGU_NATIVE_AUTO_UPDATER_FAIL)
            .with_field("error", err.to_string())
            .with_field("target_version", target_version.to_string()),
    );
    Ok(())
}

fn now_epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        // SAFETY: u64::MAX milliseconds is ~584 million years from UNIX_EPOCH.
        // Truncation is not a concern for any practical timestamp.
        .map(|duration| {
            #[allow(clippy::cast_possible_truncation)]
            let ms = duration.as_millis() as u64;
            ms
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::telemetry::VecTelemetry;

    struct TestDownloader {
        payload: Vec<u8>,
        fail: bool,
    }

    #[async_trait]
    impl Downloader for TestDownloader {
        async fn download_to(&self, _url: &str, dest: &Path) -> Result<()> {
            if self.fail {
                return Err(anyhow!("download failed"));
            }
            let mut file = tokio::fs::File::create(dest).await?;
            file.write_all(&self.payload).await?;
            file.flush().await?;
            Ok(())
        }
    }

    #[tokio::test]
    async fn native_update_writes_lock_and_installs_binary() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let install_path = tmp.path().join("dterm");
        fs::write(&install_path, "old").unwrap();

        let config = NativeUpdateConfig {
            target_version: "1.2.3".to_string(),
            download_url: "https://example.com/dterm".to_string(),
            install_path: install_path.clone(),
            data_dir: data_dir.clone(),
        };
        let downloader = TestDownloader {
            payload: b"new".to_vec(),
            fail: false,
        };
        let telemetry = VecTelemetry::new();

        let result = run_native_update(config, &downloader, &telemetry)
            .await
            .unwrap();
        let contents = fs::read_to_string(&result.installed_path).unwrap();
        assert_eq!(contents, "new");

        let lock_contents = fs::read_to_string(result.lock_path).unwrap();
        let lock: UpdateLock = toml::from_str(&lock_contents).unwrap();
        assert_eq!(lock.status, UpdateStatus::Succeeded);

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_NATIVE_AUTO_UPDATER_SUCCESS));
    }

    #[tokio::test]
    async fn native_update_records_failure() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("data");
        let install_path = tmp.path().join("dterm");
        fs::write(&install_path, "old").unwrap();

        let config = NativeUpdateConfig {
            target_version: "1.2.3".to_string(),
            download_url: "https://example.com/dterm".to_string(),
            install_path: install_path.clone(),
            data_dir: data_dir.clone(),
        };
        let downloader = TestDownloader {
            payload: Vec::new(),
            fail: true,
        };
        let telemetry = VecTelemetry::new();

        let result = run_native_update(config, &downloader, &telemetry).await;
        assert!(result.is_err());

        let lock_contents = fs::read_to_string(data_dir.join("update.lock")).unwrap();
        let lock: UpdateLock = toml::from_str(&lock_contents).unwrap();
        assert_eq!(lock.status, UpdateStatus::Failed);

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_NATIVE_AUTO_UPDATER_FAIL));
    }
}
