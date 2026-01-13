use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::prompt::Prompt;
use crate::telemetry::{TelemetryEvent, TelemetrySink};

pub const TENGU_SETUP_GITHUB_ACTIONS_START: &str = "tengu_setup_github_actions_start";
pub const TENGU_SETUP_GITHUB_ACTIONS_SUCCESS: &str = "tengu_setup_github_actions_success";
pub const TENGU_SETUP_GITHUB_ACTIONS_FAIL: &str = "tengu_setup_github_actions_fail";

#[derive(Debug, Clone)]
pub struct GithubActionsSetupConfig {
    pub repo_root: PathBuf,
    pub workflow_path: Option<PathBuf>,
    pub secret_names: Vec<String>,
    pub prompt_for_secrets: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubActionsSetupResult {
    pub workflow_path: PathBuf,
    pub secret_names: Vec<String>,
}

pub fn run_setup(
    mut config: GithubActionsSetupConfig,
    prompt: &mut dyn Prompt,
    telemetry: &dyn TelemetrySink,
) -> Result<GithubActionsSetupResult> {
    telemetry.emit(TelemetryEvent::new(TENGU_SETUP_GITHUB_ACTIONS_START));
    if config.secret_names.is_empty() && config.prompt_for_secrets {
        config.secret_names = prompt_secret_names(prompt)?;
    }

    if config.secret_names.is_empty() {
        config.secret_names.push("DTERM_API_KEY".to_string());
    }

    let workflow_path = config
        .workflow_path
        .unwrap_or_else(|| default_workflow_path(&config.repo_root));

    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = generate_workflow(&config.secret_names);
    if let Err(err) = fs::write(&workflow_path, contents) {
        telemetry.emit(
            TelemetryEvent::new(TENGU_SETUP_GITHUB_ACTIONS_FAIL)
                .with_field("error", err.to_string()),
        );
        return Err(err.into());
    }

    telemetry.emit(
        TelemetryEvent::new(TENGU_SETUP_GITHUB_ACTIONS_SUCCESS)
            .with_field("workflow", workflow_path.display().to_string()),
    );

    Ok(GithubActionsSetupResult {
        workflow_path,
        secret_names: config.secret_names,
    })
}

fn prompt_secret_names(prompt: &mut dyn Prompt) -> Result<Vec<String>> {
    let mut names = Vec::new();
    loop {
        let input = prompt.input("Add a GitHub Actions secret (blank to finish):")?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            break;
        }
        names.push(trimmed.to_string());
    }
    Ok(names)
}

fn default_workflow_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".github")
        .join("workflows")
        .join("dterm.yml")
}

fn generate_workflow(secret_names: &[String]) -> String {
    let mut env_lines = String::new();
    for name in secret_names {
        env_lines.push_str(&format!("          {name}: ${{{{ secrets.{name} }}}}\n"));
    }

    format!(
        "name: dterm\n\n\
on:\n\
  workflow_dispatch:\n\
  issue_comment:\n\
    types: [created]\n\n\
permissions:\n\
  contents: read\n\
  issues: write\n\
  pull-requests: write\n\n\
jobs:\n\
  dterm:\n\
    if: ${{{{ github.event_name == 'workflow_dispatch' || contains(github.event.comment.body, '/dterm') }}}}\n\
    runs-on: ubuntu-latest\n\
    steps:\n\
      - uses: actions/checkout@v4\n\
      - name: Install dterm\n\
        run: cargo install dterm --locked\n\
      - name: Run dterm\n\
        env:\n\
{env_lines}        run: dterm --help\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::prompt::{ScriptedPrompt, ScriptedResponse};
    use crate::telemetry::VecTelemetry;

    #[test]
    fn setup_writes_workflow_with_secrets() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().to_path_buf();
        let config = GithubActionsSetupConfig {
            repo_root: repo_root.clone(),
            workflow_path: None,
            secret_names: vec!["DTERM_API_KEY".to_string(), "DTERM_TEAM".to_string()],
            prompt_for_secrets: false,
        };
        let mut prompt = ScriptedPrompt::new(vec![]);
        let telemetry = VecTelemetry::new();

        let result = run_setup(config, &mut prompt, &telemetry).unwrap();
        let contents = fs::read_to_string(&result.workflow_path).unwrap();
        assert!(contents.contains("DTERM_API_KEY"));
        assert!(contents.contains("DTERM_TEAM"));

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_SETUP_GITHUB_ACTIONS_SUCCESS));
    }

    #[test]
    fn setup_prompts_for_secrets_when_empty() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().to_path_buf();
        let config = GithubActionsSetupConfig {
            repo_root,
            workflow_path: None,
            secret_names: Vec::new(),
            prompt_for_secrets: true,
        };
        let mut prompt = ScriptedPrompt::new(vec![
            ScriptedResponse::Input("DTERM_API_KEY".to_string()),
            ScriptedResponse::Input("".to_string()),
        ]);
        let telemetry = VecTelemetry::new();

        let result = run_setup(config, &mut prompt, &telemetry).unwrap();
        let contents = fs::read_to_string(result.workflow_path).unwrap();
        assert!(contents.contains("DTERM_API_KEY"));
    }
}
