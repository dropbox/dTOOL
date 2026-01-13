//! Docker service management for integration tests

use std::process::Command;

use crate::{Result, TestError};

/// Docker Compose service manager
pub struct DockerServices {
    compose_file: String,
    project_name: Option<String>,
}

impl DockerServices {
    /// Create a new Docker services manager
    pub fn new(compose_file: impl Into<String>) -> Self {
        Self {
            compose_file: compose_file.into(),
            project_name: None,
        }
    }

    /// Set the project name
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }

    /// Start all services
    pub fn start(&self) -> Result<()> {
        tracing::info!("Starting docker services from {}", self.compose_file);

        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f").arg(&self.compose_file);

        if let Some(ref name) = self.project_name {
            cmd.arg("-p").arg(name);
        }

        cmd.arg("up").arg("-d");

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestError::DockerError(format!(
                "Failed to start docker services: {stderr}"
            )));
        }

        tracing::info!("Docker services started successfully");
        Ok(())
    }

    /// Stop all services
    pub fn stop(&self) -> Result<()> {
        tracing::info!("Stopping docker services");

        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f").arg(&self.compose_file);

        if let Some(ref name) = self.project_name {
            cmd.arg("-p").arg(name);
        }

        cmd.arg("down");

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestError::DockerError(format!(
                "Failed to stop docker services: {stderr}"
            )));
        }

        tracing::info!("Docker services stopped successfully");
        Ok(())
    }

    /// Check if docker compose is available
    #[must_use]
    pub fn is_docker_available() -> bool {
        Command::new("docker-compose")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    /// Get service logs
    pub fn logs(&self, service: Option<&str>) -> Result<String> {
        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f").arg(&self.compose_file);

        if let Some(ref name) = self.project_name {
            cmd.arg("-p").arg(name);
        }

        cmd.arg("logs");

        if let Some(svc) = service {
            cmd.arg(svc);
        }

        let output = cmd.output()?;

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Clean up volumes (WARNING: removes all data)
    pub fn clean_volumes(&self) -> Result<()> {
        tracing::warn!("Cleaning up docker volumes");

        let mut cmd = Command::new("docker-compose");
        cmd.arg("-f").arg(&self.compose_file);

        if let Some(ref name) = self.project_name {
            cmd.arg("-p").arg(name);
        }

        cmd.arg("down").arg("-v");

        let output = cmd.output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(TestError::DockerError(format!(
                "Failed to clean volumes: {stderr}"
            )));
        }

        Ok(())
    }
}

/// Setup function for integration tests
pub async fn setup_docker_services() -> Result<DockerServices> {
    let services =
        DockerServices::new("docker-compose.test.yml").with_project_name("dashflow-test");

    if !DockerServices::is_docker_available() {
        return Err(TestError::DockerError(
            "docker-compose is not available".to_string(),
        ));
    }

    services.start()?;

    // Wait for services to be healthy
    crate::health::check_all_docker_services().await?;

    Ok(services)
}

/// Teardown function for integration tests
pub async fn teardown_docker_services(services: &DockerServices) -> Result<()> {
    services.stop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_docker_available_matches_actual_docker_status() {
        // M-662: Actually verify the result matches Docker's real status
        let is_available = DockerServices::is_docker_available();

        // Cross-check: if is_docker_available returns true, verify Docker actually works
        // by running a simple version command
        if is_available {
            let check = std::process::Command::new("docker")
                .arg("--version")
                .output();

            assert!(
                check.is_ok_and(|o| o.status.success()),
                "is_docker_available() returned true but docker --version failed"
            );
        }

        // Note: if Docker is not available (returns false), that's valid for the test
        // environment - we just ensure the function doesn't panic or return incorrect true
        println!(
            "Docker availability check completed: available={}",
            is_available
        );
    }

    #[test]
    fn test_docker_services_new() {
        let services = DockerServices::new("test-compose.yml");
        assert_eq!(services.compose_file, "test-compose.yml");
        assert!(services.project_name.is_none());
    }

    #[test]
    fn test_docker_services_with_project_name() {
        let services = DockerServices::new("test-compose.yml").with_project_name("my-project");
        assert_eq!(services.compose_file, "test-compose.yml");
        assert_eq!(services.project_name, Some("my-project".to_string()));
    }
}
