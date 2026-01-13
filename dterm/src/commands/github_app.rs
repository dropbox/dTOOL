use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::Deserialize;

use crate::prompt::Prompt;
use crate::telemetry::{TelemetryEvent, TelemetrySink};

pub const TENGU_INSTALL_GITHUB_APP_START: &str = "tengu_install_github_app_start";
pub const TENGU_INSTALL_GITHUB_APP_SUCCESS: &str = "tengu_install_github_app_success";
pub const TENGU_INSTALL_GITHUB_APP_FAIL: &str = "tengu_install_github_app_fail";

pub const DEFAULT_GITHUB_APP_SECRET: &str = "DTERM_GITHUB_APP_TOKEN";

#[derive(Debug, Clone)]
pub struct GithubAppInstallConfig {
    pub repo_root: PathBuf,
    pub workflow_path: Option<PathBuf>,
    pub oauth: OAuthConfig,
    pub prompt_for_credentials: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubAppInstallResult {
    pub workflow_path: PathBuf,
    pub access_token: String,
    pub required_secrets: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub auth_url: String,
    pub token_url: String,
}

impl OAuthConfig {
    pub fn new(client_id: String, client_secret: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_uri: "urn:ietf:wg:oauth:2.0:oob".to_string(),
            scopes: vec!["repo".to_string()],
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
        }
    }
}

#[async_trait]
pub trait OAuthClient: Send + Sync {
    async fn exchange_code(&self, config: &OAuthConfig, code: &str) -> Result<String>;
}

pub struct HttpOAuthClient {
    client: reqwest::Client,
}

impl HttpOAuthClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl OAuthClient for HttpOAuthClient {
    async fn exchange_code(&self, config: &OAuthConfig, code: &str) -> Result<String> {
        let params = [
            ("client_id", config.client_id.as_str()),
            ("client_secret", config.client_secret.as_str()),
            ("code", code),
            ("redirect_uri", config.redirect_uri.as_str()),
        ];

        let response = self
            .client
            .post(&config.token_url)
            .header("Accept", "application/json")
            .form(&params)
            .send()
            .await?
            .error_for_status()?;

        let body: OAuthTokenResponse = response.json().await?;
        if let Some(token) = body.access_token {
            return Ok(token);
        }
        if let Some(error) = body.error {
            if let Some(description) = body.error_description {
                return Err(anyhow!("oauth error: {error}: {description}"));
            }
            return Err(anyhow!("oauth error: {error}"));
        }
        Err(anyhow!("oauth error: missing access token"))
    }
}

#[derive(Debug, Deserialize)]
struct OAuthTokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

pub async fn run_wizard(
    mut config: GithubAppInstallConfig,
    prompt: &mut dyn Prompt,
    oauth_client: &dyn OAuthClient,
    telemetry: &dyn TelemetrySink,
) -> Result<GithubAppInstallResult> {
    telemetry.emit(TelemetryEvent::new(TENGU_INSTALL_GITHUB_APP_START));

    if config.prompt_for_credentials {
        if config.oauth.client_id.trim().is_empty() {
            config.oauth.client_id = prompt.input("GitHub App client ID:")?;
        }
        if config.oauth.client_secret.trim().is_empty() {
            config.oauth.client_secret = prompt.input("GitHub App client secret:")?;
        }
        if config.oauth.redirect_uri.trim().is_empty() {
            config.oauth.redirect_uri = prompt.input("Redirect URI (blank for default):")?;
            if config.oauth.redirect_uri.trim().is_empty() {
                config.oauth.redirect_uri = "urn:ietf:wg:oauth:2.0:oob".to_string();
            }
        }
    }

    let state = uuid::Uuid::new_v4().to_string();
    let auth_url = build_authorize_url(&config.oauth, &state)?;
    let code = prompt.input(&format!(
        "Open this URL to authorize the GitHub App, then paste the code:\n{auth_url}\nCode:"
    ))?;

    let access_token = match oauth_client.exchange_code(&config.oauth, &code).await {
        Ok(token) => token,
        Err(err) => {
            telemetry.emit(
                TelemetryEvent::new(TENGU_INSTALL_GITHUB_APP_FAIL)
                    .with_field("error", err.to_string()),
            );
            return Err(err);
        }
    };

    let workflow_path = config
        .workflow_path
        .unwrap_or_else(|| default_workflow_path(&config.repo_root));
    if let Some(parent) = workflow_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = generate_workflow(DEFAULT_GITHUB_APP_SECRET);
    if let Err(err) = fs::write(&workflow_path, contents) {
        telemetry.emit(
            TelemetryEvent::new(TENGU_INSTALL_GITHUB_APP_FAIL).with_field("error", err.to_string()),
        );
        return Err(err.into());
    }

    telemetry.emit(
        TelemetryEvent::new(TENGU_INSTALL_GITHUB_APP_SUCCESS)
            .with_field("workflow", workflow_path.display().to_string()),
    );

    Ok(GithubAppInstallResult {
        workflow_path,
        access_token,
        required_secrets: vec![DEFAULT_GITHUB_APP_SECRET.to_string()],
    })
}

pub fn build_authorize_url(config: &OAuthConfig, state: &str) -> Result<reqwest::Url> {
    let mut url = reqwest::Url::parse(&config.auth_url)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("client_id", &config.client_id);
        pairs.append_pair("redirect_uri", &config.redirect_uri);
        pairs.append_pair("state", state);
        if !config.scopes.is_empty() {
            pairs.append_pair("scope", &config.scopes.join(" "));
        }
    }
    Ok(url)
}

fn default_workflow_path(repo_root: &Path) -> PathBuf {
    repo_root
        .join(".github")
        .join("workflows")
        .join("dterm-github-app.yml")
}

fn generate_workflow(secret_name: &str) -> String {
    format!(
        "name: dterm-github-app\n\n\
on:\n\
  workflow_dispatch:\n\
  issue_comment:\n\
    types: [created]\n\n\
permissions:\n\
  contents: read\n\
  issues: write\n\
  pull-requests: write\n\n\
jobs:\n\
  dterm-app:\n\
    if: ${{{{ github.event_name == 'workflow_dispatch' || contains(github.event.comment.body, '/dterm') }}}}\n\
    runs-on: ubuntu-latest\n\
    steps:\n\
      - uses: actions/checkout@v4\n\
      - name: Install dterm\n\
        run: cargo install dterm --locked\n\
      - name: Run dterm\n\
        env:\n\
          {secret_name}: ${{{{ secrets.{secret_name} }}}}\n\
        run: dterm --help\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    use crate::prompt::{ScriptedPrompt, ScriptedResponse};
    use crate::telemetry::VecTelemetry;

    struct TestOAuthClient {
        expected_code: String,
        token: String,
    }

    #[async_trait]
    impl OAuthClient for TestOAuthClient {
        async fn exchange_code(&self, _config: &OAuthConfig, code: &str) -> Result<String> {
            if code != self.expected_code {
                return Err(anyhow!("unexpected code"));
            }
            Ok(self.token.clone())
        }
    }

    #[tokio::test]
    async fn wizard_creates_workflow_and_returns_token() {
        let tmp = TempDir::new().unwrap();
        let repo_root = tmp.path().to_path_buf();
        let config = GithubAppInstallConfig {
            repo_root,
            workflow_path: None,
            oauth: OAuthConfig::new(String::new(), String::new()),
            prompt_for_credentials: true,
        };

        let mut prompt = ScriptedPrompt::new(vec![
            ScriptedResponse::Input("client-id".to_string()),
            ScriptedResponse::Input("client-secret".to_string()),
            ScriptedResponse::Input("oauth-code".to_string()),
        ]);
        let telemetry = VecTelemetry::new();
        let oauth_client = TestOAuthClient {
            expected_code: "oauth-code".to_string(),
            token: "token123".to_string(),
        };

        let result = run_wizard(config, &mut prompt, &oauth_client, &telemetry)
            .await
            .unwrap();
        let contents = fs::read_to_string(result.workflow_path).unwrap();
        assert!(contents.contains(DEFAULT_GITHUB_APP_SECRET));
        assert_eq!(result.access_token, "token123");

        let events = telemetry.events();
        assert!(events
            .iter()
            .any(|event| event.name == TENGU_INSTALL_GITHUB_APP_SUCCESS));
    }

    #[test]
    fn build_authorize_url_contains_state() {
        let config = OAuthConfig {
            client_id: "client".to_string(),
            client_secret: "secret".to_string(),
            redirect_uri: "http://localhost".to_string(),
            scopes: vec!["repo".to_string()],
            auth_url: "https://github.com/login/oauth/authorize".to_string(),
            token_url: "https://github.com/login/oauth/access_token".to_string(),
        };
        let url = build_authorize_url(&config, "state123").unwrap();
        let query = url.query().unwrap_or_default();
        assert!(query.contains("state=state123"));
        assert!(query.contains("client_id=client"));
    }
}
