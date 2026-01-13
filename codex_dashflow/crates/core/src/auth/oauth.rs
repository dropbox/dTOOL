//! OAuth authentication module for Codex DashFlow
//!
//! Implements OAuth 2.0 with PKCE for ChatGPT account sign-in.
//! Based on OpenAI Codex login module patterns.

use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use chrono::Utc;
use tiny_http::{Header, Request, Response, Server, StatusCode};

use super::storage::AuthCredentialsStoreMode;
use super::{AuthDotJson, TokenData};

/// OAuth client ID for Codex
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";

/// Default OAuth issuer
pub const DEFAULT_ISSUER: &str = "https://auth.openai.com";

/// Default local callback server port
pub const DEFAULT_PORT: u16 = 1455;

/// PKCE codes for OAuth flow
#[derive(Debug, Clone)]
pub struct PkceCodes {
    pub code_verifier: String,
    pub code_challenge: String,
}

/// Generate PKCE codes for OAuth flow
pub fn generate_pkce() -> PkceCodes {
    let mut bytes = [0u8; 64];
    rand::rng().fill_bytes(&mut bytes);

    // Verifier: URL-safe base64 without padding (43..128 chars)
    let code_verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);

    // Challenge (S256): BASE64URL-ENCODE(SHA256(verifier)) without padding
    let digest = Sha256::digest(code_verifier.as_bytes());
    let code_challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);

    PkceCodes {
        code_verifier,
        code_challenge,
    }
}

/// Options for the login server
#[derive(Debug, Clone)]
pub struct ServerOptions {
    pub codex_home: PathBuf,
    pub client_id: String,
    pub issuer: String,
    pub port: u16,
    pub open_browser: bool,
    pub force_state: Option<String>,
    pub forced_chatgpt_workspace_id: Option<String>,
    pub cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
}

impl ServerOptions {
    pub fn new(
        codex_home: PathBuf,
        forced_chatgpt_workspace_id: Option<String>,
        cli_auth_credentials_store_mode: AuthCredentialsStoreMode,
    ) -> Self {
        Self {
            codex_home,
            client_id: CLIENT_ID.to_string(),
            issuer: DEFAULT_ISSUER.to_string(),
            port: DEFAULT_PORT,
            open_browser: true,
            force_state: None,
            forced_chatgpt_workspace_id,
            cli_auth_credentials_store_mode,
        }
    }
}

/// Handle for cancelling a running login server
#[derive(Clone, Debug)]
pub struct ShutdownHandle {
    shutdown_notify: Arc<tokio::sync::Notify>,
}

impl ShutdownHandle {
    pub fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}

/// Login server that handles OAuth callback
pub struct LoginServer {
    pub auth_url: String,
    pub actual_port: u16,
    server_handle: tokio::task::JoinHandle<io::Result<()>>,
    shutdown_handle: ShutdownHandle,
}

impl LoginServer {
    pub async fn block_until_done(self) -> io::Result<()> {
        self.server_handle
            .await
            .map_err(|err| io::Error::other(format!("login server thread panicked: {err:?}")))?
    }

    pub fn cancel(&self) {
        self.shutdown_handle.shutdown();
    }

    pub fn cancel_handle(&self) -> ShutdownHandle {
        self.shutdown_handle.clone()
    }
}

/// Run the OAuth login server
pub fn run_login_server(opts: ServerOptions) -> io::Result<LoginServer> {
    let pkce = generate_pkce();
    let state = opts.force_state.clone().unwrap_or_else(generate_state);

    let server = bind_server(opts.port)?;
    let actual_port = match server.server_addr().to_ip() {
        Some(addr) => addr.port(),
        None => {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "Unable to determine the server port",
            ));
        }
    };
    let server = Arc::new(server);

    let redirect_uri = format!("http://localhost:{actual_port}/auth/callback");
    let auth_url = build_authorize_url(
        &opts.issuer,
        &opts.client_id,
        &redirect_uri,
        &pkce,
        &state,
        opts.forced_chatgpt_workspace_id.as_deref(),
    );

    if opts.open_browser {
        let _ = webbrowser::open(&auth_url);
    }

    // Map blocking reads from server.recv() to an async channel.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Request>(16);
    let _server_handle = {
        let server = server.clone();
        thread::spawn(move || -> io::Result<()> {
            while let Ok(request) = server.recv() {
                tx.blocking_send(request).map_err(|e| {
                    eprintln!("Failed to send request to channel: {e}");
                    io::Error::other("Failed to send request to channel")
                })?;
            }
            Ok(())
        })
    };

    let shutdown_notify = Arc::new(tokio::sync::Notify::new());
    let server_handle = {
        let shutdown_notify = shutdown_notify.clone();
        let server = server;
        tokio::spawn(async move {
            let result = loop {
                tokio::select! {
                    _ = shutdown_notify.notified() => {
                        break Err(io::Error::other("Login was not completed"));
                    }
                    maybe_req = rx.recv() => {
                        let Some(req) = maybe_req else {
                            break Err(io::Error::other("Login was not completed"));
                        };

                        let url_raw = req.url().to_string();
                        let response =
                            process_request(&url_raw, &opts, &redirect_uri, &pkce, actual_port, &state).await;

                        let exit_result = match response {
                            HandledRequest::Response(response) => {
                                let _ = tokio::task::spawn_blocking(move || req.respond(response)).await;
                                None
                            }
                            HandledRequest::ResponseAndExit {
                                headers,
                                body,
                                result,
                            } => {
                                let _ = tokio::task::spawn_blocking(move || {
                                    send_response_with_disconnect(req, headers, body)
                                })
                                .await;
                                Some(result)
                            }
                            HandledRequest::RedirectWithHeader(header) => {
                                let redirect = Response::empty(302).with_header(header);
                                let _ = tokio::task::spawn_blocking(move || req.respond(redirect)).await;
                                None
                            }
                        };

                        if let Some(result) = exit_result {
                            break result;
                        }
                    }
                }
            };

            // Ensure that the server is unblocked so the thread dedicated to
            // running `server.recv()` in a loop exits cleanly.
            server.unblock();
            result
        })
    };

    Ok(LoginServer {
        auth_url,
        actual_port,
        server_handle,
        shutdown_handle: ShutdownHandle { shutdown_notify },
    })
}

enum HandledRequest {
    Response(Response<std::io::Cursor<Vec<u8>>>),
    RedirectWithHeader(Header),
    ResponseAndExit {
        headers: Vec<Header>,
        body: Vec<u8>,
        result: io::Result<()>,
    },
}

async fn process_request(
    url_raw: &str,
    opts: &ServerOptions,
    redirect_uri: &str,
    pkce: &PkceCodes,
    actual_port: u16,
    state: &str,
) -> HandledRequest {
    let parsed_url = match url::Url::parse(&format!("http://localhost{url_raw}")) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("URL parse error: {e}");
            return HandledRequest::Response(
                Response::from_string("Bad Request").with_status_code(400),
            );
        }
    };
    let path = parsed_url.path().to_string();

    match path.as_str() {
        "/auth/callback" => {
            let params: std::collections::HashMap<String, String> =
                parsed_url.query_pairs().into_owned().collect();
            if params.get("state").map(String::as_str) != Some(state) {
                return HandledRequest::Response(
                    Response::from_string("State mismatch").with_status_code(400),
                );
            }
            let code = match params.get("code") {
                Some(c) if !c.is_empty() => c.clone(),
                _ => {
                    return HandledRequest::Response(
                        Response::from_string("Missing authorization code").with_status_code(400),
                    );
                }
            };

            match exchange_code_for_tokens(&opts.issuer, &opts.client_id, redirect_uri, pkce, &code)
                .await
            {
                Ok(tokens) => {
                    if let Err(message) = ensure_workspace_allowed(
                        opts.forced_chatgpt_workspace_id.as_deref(),
                        &tokens.id_token,
                    ) {
                        eprintln!("Workspace restriction error: {message}");
                        return login_error_response(&message);
                    }
                    // Obtain API key via token-exchange and persist
                    let api_key = obtain_api_key(&opts.issuer, &opts.client_id, &tokens.id_token)
                        .await
                        .ok();
                    if let Err(err) = persist_tokens_async(
                        &opts.codex_home,
                        api_key.clone(),
                        tokens.id_token.clone(),
                        tokens.access_token.clone(),
                        tokens.refresh_token.clone(),
                        opts.cli_auth_credentials_store_mode,
                    )
                    .await
                    {
                        eprintln!("Persist error: {err}");
                        return HandledRequest::Response(
                            Response::from_string(format!("Unable to persist auth file: {err}"))
                                .with_status_code(500),
                        );
                    }

                    let success_url = compose_success_url(
                        actual_port,
                        &opts.issuer,
                        &tokens.id_token,
                        &tokens.access_token,
                    );
                    match tiny_http::Header::from_bytes(&b"Location"[..], success_url.as_bytes()) {
                        Ok(header) => HandledRequest::RedirectWithHeader(header),
                        Err(_) => HandledRequest::Response(
                            Response::from_string("Internal Server Error").with_status_code(500),
                        ),
                    }
                }
                Err(err) => {
                    eprintln!("Token exchange error: {err}");
                    HandledRequest::Response(
                        Response::from_string(format!("Token exchange failed: {err}"))
                            .with_status_code(500),
                    )
                }
            }
        }
        "/success" => {
            let body = include_str!("assets/success.html");
            HandledRequest::ResponseAndExit {
                headers: match Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                ) {
                    Ok(header) => vec![header],
                    Err(_) => Vec::new(),
                },
                body: body.as_bytes().to_vec(),
                result: Ok(()),
            }
        }
        "/cancel" => HandledRequest::ResponseAndExit {
            headers: Vec::new(),
            body: b"Login cancelled".to_vec(),
            result: Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Login cancelled",
            )),
        },
        _ => HandledRequest::Response(Response::from_string("Not Found").with_status_code(404)),
    }
}

fn send_response_with_disconnect(
    req: Request,
    mut headers: Vec<Header>,
    body: Vec<u8>,
) -> io::Result<()> {
    let status = StatusCode(200);
    let mut writer = req.into_writer();
    let reason = status.default_reason_phrase();
    write!(writer, "HTTP/1.1 {} {}\r\n", status.0, reason)?;
    headers.retain(|h| !h.field.equiv("Connection"));
    if let Ok(close_header) = Header::from_bytes(&b"Connection"[..], &b"close"[..]) {
        headers.push(close_header);
    }

    let content_length_value = format!("{}", body.len());
    if let Ok(content_length_header) =
        Header::from_bytes(&b"Content-Length"[..], content_length_value.as_bytes())
    {
        headers.push(content_length_header);
    }

    for header in headers {
        write!(
            writer,
            "{}: {}\r\n",
            header.field.as_str(),
            header.value.as_str()
        )?;
    }

    writer.write_all(b"\r\n")?;
    writer.write_all(&body)?;
    writer.flush()
}

fn build_authorize_url(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
    state: &str,
    forced_chatgpt_workspace_id: Option<&str>,
) -> String {
    let mut query = vec![
        ("response_type".to_string(), "code".to_string()),
        ("client_id".to_string(), client_id.to_string()),
        ("redirect_uri".to_string(), redirect_uri.to_string()),
        (
            "scope".to_string(),
            "openid profile email offline_access".to_string(),
        ),
        (
            "code_challenge".to_string(),
            pkce.code_challenge.to_string(),
        ),
        ("code_challenge_method".to_string(), "S256".to_string()),
        ("id_token_add_organizations".to_string(), "true".to_string()),
        ("codex_cli_simplified_flow".to_string(), "true".to_string()),
        ("state".to_string(), state.to_string()),
        ("originator".to_string(), "codex-dashflow".to_string()),
    ];
    if let Some(workspace_id) = forced_chatgpt_workspace_id {
        query.push(("allowed_workspace_id".to_string(), workspace_id.to_string()));
    }
    let qs = query
        .into_iter()
        .map(|(k, v)| format!("{k}={}", urlencoding::encode(&v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{issuer}/oauth/authorize?{qs}")
}

fn generate_state() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn send_cancel_request(port: u16) -> io::Result<()> {
    let addr: SocketAddr = format!("127.0.0.1:{port}")
        .parse()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidInput, err))?;
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(2))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;

    stream.write_all(b"GET /cancel HTTP/1.1\r\n")?;
    stream.write_all(format!("Host: 127.0.0.1:{port}\r\n").as_bytes())?;
    stream.write_all(b"Connection: close\r\n\r\n")?;

    let mut buf = [0u8; 64];
    let _ = stream.read(&mut buf);
    Ok(())
}

fn bind_server(port: u16) -> io::Result<Server> {
    let bind_address = format!("127.0.0.1:{port}");
    let mut cancel_attempted = false;
    let mut attempts = 0;
    const MAX_ATTEMPTS: u32 = 10;
    const RETRY_DELAY: Duration = Duration::from_millis(200);

    loop {
        match Server::http(&bind_address) {
            Ok(server) => return Ok(server),
            Err(err) => {
                attempts += 1;
                let is_addr_in_use = err
                    .downcast_ref::<io::Error>()
                    .map(|io_err| io_err.kind() == io::ErrorKind::AddrInUse)
                    .unwrap_or(false);

                // If the address is in use, there is probably another instance of the login server
                // running. Attempt to cancel it and retry.
                if is_addr_in_use {
                    if !cancel_attempted {
                        cancel_attempted = true;
                        if let Err(cancel_err) = send_cancel_request(port) {
                            eprintln!("Failed to cancel previous login server: {cancel_err}");
                        }
                    }

                    thread::sleep(RETRY_DELAY);

                    if attempts >= MAX_ATTEMPTS {
                        return Err(io::Error::new(
                            io::ErrorKind::AddrInUse,
                            format!("Port {bind_address} is already in use"),
                        ));
                    }

                    continue;
                }

                return Err(io::Error::other(err));
            }
        }
    }
}

struct ExchangedTokens {
    id_token: String,
    access_token: String,
    refresh_token: String,
}

async fn exchange_code_for_tokens(
    issuer: &str,
    client_id: &str,
    redirect_uri: &str,
    pkce: &PkceCodes,
    code: &str,
) -> io::Result<ExchangedTokens> {
    #[derive(serde::Deserialize)]
    struct TokenResponse {
        id_token: String,
        access_token: String,
        refresh_token: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
            urlencoding::encode(code),
            urlencoding::encode(redirect_uri),
            urlencoding::encode(client_id),
            urlencoding::encode(&pkce.code_verifier)
        ))
        .send()
        .await
        .map_err(io::Error::other)?;

    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "token endpoint returned status {}",
            resp.status()
        )));
    }

    let tokens: TokenResponse = resp.json().await.map_err(io::Error::other)?;
    Ok(ExchangedTokens {
        id_token: tokens.id_token,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

async fn persist_tokens_async(
    codex_home: &Path,
    api_key: Option<String>,
    id_token: String,
    access_token: String,
    refresh_token: String,
    auth_credentials_store_mode: AuthCredentialsStoreMode,
) -> io::Result<()> {
    let codex_home = codex_home.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let parsed = parse_id_token(&id_token)?;
        let mut tokens = TokenData {
            access_token,
            refresh_token: Some(refresh_token),
            expires_at: parsed.expires_at,
            account_id: None,
            email: parsed.email.clone(),
        };
        if let Some(acc) = jwt_auth_claims(&id_token)
            .get("chatgpt_account_id")
            .and_then(|v| v.as_str())
        {
            tokens.account_id = Some(acc.to_string());
        }
        let auth = AuthDotJson {
            openai_api_key: api_key,
            tokens: Some(tokens),
            last_refresh: Some(Utc::now()),
        };
        save_auth(&codex_home, &auth, auth_credentials_store_mode)
    })
    .await
    .map_err(|e| io::Error::other(format!("persist task failed: {e}")))?
}

fn compose_success_url(port: u16, issuer: &str, id_token: &str, access_token: &str) -> String {
    let token_claims = jwt_auth_claims(id_token);
    let access_claims = jwt_auth_claims(access_token);

    let org_id = token_claims
        .get("organization_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let project_id = token_claims
        .get("project_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let completed_onboarding = token_claims
        .get("completed_platform_onboarding")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let is_org_owner = token_claims
        .get("is_org_owner")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let needs_setup = (!completed_onboarding) && is_org_owner;
    let plan_type = access_claims
        .get("chatgpt_plan_type")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let platform_url = if issuer == DEFAULT_ISSUER {
        "https://platform.openai.com"
    } else {
        "https://platform.api.openai.org"
    };

    let mut params = vec![
        ("id_token", id_token.to_string()),
        ("needs_setup", needs_setup.to_string()),
        ("org_id", org_id.to_string()),
        ("project_id", project_id.to_string()),
        ("plan_type", plan_type.to_string()),
        ("platform_url", platform_url.to_string()),
    ];
    let qs = params
        .drain(..)
        .map(|(k, v)| format!("{}={}", k, urlencoding::encode(&v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("http://localhost:{port}/success?{qs}")
}

fn jwt_auth_claims(jwt: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut parts = jwt.split('.');
    let (_h, payload_b64, _s) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => {
            eprintln!("Invalid JWT format while extracting claims");
            return serde_json::Map::new();
        }
    };
    match base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64) {
        Ok(bytes) => match serde_json::from_slice::<serde_json::Value>(&bytes) {
            Ok(mut v) => {
                if let Some(obj) = v
                    .get_mut("https://api.openai.com/auth")
                    .and_then(|x| x.as_object_mut())
                {
                    return obj.clone();
                }
                eprintln!("JWT payload missing expected 'https://api.openai.com/auth' object");
            }
            Err(e) => {
                eprintln!("Failed to parse JWT JSON payload: {e}");
            }
        },
        Err(e) => {
            eprintln!("Failed to base64url-decode JWT payload: {e}");
        }
    }
    serde_json::Map::new()
}

fn ensure_workspace_allowed(expected: Option<&str>, id_token: &str) -> Result<(), String> {
    let Some(expected) = expected else {
        return Ok(());
    };

    let claims = jwt_auth_claims(id_token);
    let Some(actual) = claims
        .get("chatgpt_account_id")
        .and_then(serde_json::Value::as_str)
    else {
        return Err("Login is restricted to a specific workspace, but the token did not include a chatgpt_account_id claim.".to_string());
    };

    if actual == expected {
        Ok(())
    } else {
        Err(format!("Login is restricted to workspace id {expected}."))
    }
}

fn login_error_response(message: &str) -> HandledRequest {
    let mut headers = Vec::new();
    if let Ok(header) = Header::from_bytes(&b"Content-Type"[..], &b"text/plain; charset=utf-8"[..])
    {
        headers.push(header);
    }
    HandledRequest::ResponseAndExit {
        headers,
        body: message.as_bytes().to_vec(),
        result: Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            message.to_string(),
        )),
    }
}

/// Refresh an expired access token using a refresh token
///
/// Returns new access, refresh, and ID tokens on success.
pub async fn refresh_tokens(
    issuer: &str,
    client_id: &str,
    refresh_token: &str,
) -> io::Result<RefreshedTokens> {
    #[derive(serde::Deserialize)]
    struct RefreshResponse {
        access_token: String,
        refresh_token: String,
        id_token: String,
    }

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type=refresh_token&client_id={}&refresh_token={}",
            urlencoding::encode(client_id),
            urlencoding::encode(refresh_token)
        ))
        .send()
        .await
        .map_err(io::Error::other)?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(io::Error::other(format!(
            "token refresh failed with status {status}: {body}"
        )));
    }

    let tokens: RefreshResponse = resp.json().await.map_err(io::Error::other)?;
    Ok(RefreshedTokens {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        id_token: tokens.id_token,
    })
}

/// Tokens returned from a refresh operation
#[derive(Debug, Clone)]
pub struct RefreshedTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: String,
}

async fn obtain_api_key(issuer: &str, client_id: &str, id_token: &str) -> io::Result<String> {
    // Token exchange for an API key access token
    #[derive(serde::Deserialize)]
    struct ExchangeResp {
        access_token: String,
    }
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{issuer}/oauth/token"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!(
            "grant_type={}&client_id={}&requested_token={}&subject_token={}&subject_token_type={}",
            urlencoding::encode("urn:ietf:params:oauth:grant-type:token-exchange"),
            urlencoding::encode(client_id),
            urlencoding::encode("openai-api-key"),
            urlencoding::encode(id_token),
            urlencoding::encode("urn:ietf:params:oauth:token-type:id_token")
        ))
        .send()
        .await
        .map_err(io::Error::other)?;
    if !resp.status().is_success() {
        return Err(io::Error::other(format!(
            "api key exchange failed with status {}",
            resp.status()
        )));
    }
    let body: ExchangeResp = resp.json().await.map_err(io::Error::other)?;
    Ok(body.access_token)
}

/// Parsed ID token data
#[derive(Debug, Clone)]
pub struct ParsedIdToken {
    pub email: Option<String>,
    pub expires_at: Option<chrono::DateTime<Utc>>,
}

/// Parse an ID token to extract email and expiration
pub fn parse_id_token(id_token: &str) -> io::Result<ParsedIdToken> {
    let mut parts = id_token.split('.');
    let payload_b64 = parts
        .nth(1)
        .ok_or_else(|| io::Error::other("Invalid JWT: missing payload"))?;

    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| io::Error::other(format!("Failed to decode JWT payload: {e}")))?;

    let payload: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| io::Error::other(format!("Failed to parse JWT payload: {e}")))?;

    let email = payload
        .get("email")
        .and_then(|v| v.as_str())
        .map(String::from);

    let expires_at = payload
        .get("exp")
        .and_then(|v| v.as_i64())
        .and_then(|exp| chrono::DateTime::from_timestamp(exp, 0));

    Ok(ParsedIdToken { email, expires_at })
}

/// Save authentication data to storage
pub fn save_auth(
    codex_home: &Path,
    auth: &AuthDotJson,
    mode: AuthCredentialsStoreMode,
) -> io::Result<()> {
    let storage = super::storage::create_auth_storage(codex_home.to_path_buf(), mode);
    storage.save(auth)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =============================================
    // Constants tests
    // =============================================

    #[test]
    fn test_client_id_constant() {
        // Verify CLIENT_ID has expected format (compile-time const check)
        assert!(CLIENT_ID.len() > 4);
        assert!(CLIENT_ID.starts_with("app_"));
    }

    #[test]
    fn test_default_issuer_constant() {
        assert_eq!(DEFAULT_ISSUER, "https://auth.openai.com");
        assert!(DEFAULT_ISSUER.starts_with("https://"));
    }

    #[test]
    fn test_default_port_constant() {
        assert_eq!(DEFAULT_PORT, 1455);
        // Verify it's a non-privileged port (compile-time documentation)
        const _: () = assert!(DEFAULT_PORT > 1024);
    }

    // =============================================
    // PkceCodes tests
    // =============================================

    #[test]
    fn test_generate_pkce() {
        let pkce = generate_pkce();
        // Verifier should be 86 chars (64 bytes base64 encoded without padding)
        assert_eq!(pkce.code_verifier.len(), 86);
        // Challenge should be 43 chars (32 bytes SHA256 base64 encoded without padding)
        assert_eq!(pkce.code_challenge.len(), 43);
    }

    #[test]
    fn test_generate_pkce_unique_codes() {
        let pkce1 = generate_pkce();
        let pkce2 = generate_pkce();
        // Each call should generate unique codes
        assert_ne!(pkce1.code_verifier, pkce2.code_verifier);
        assert_ne!(pkce1.code_challenge, pkce2.code_challenge);
    }

    #[test]
    fn test_pkce_codes_clone() {
        let pkce = generate_pkce();
        let pkce_clone = pkce.clone();
        assert_eq!(pkce.code_verifier, pkce_clone.code_verifier);
        assert_eq!(pkce.code_challenge, pkce_clone.code_challenge);
    }

    #[test]
    fn test_pkce_codes_debug() {
        let pkce = PkceCodes {
            code_verifier: "test_verifier".to_string(),
            code_challenge: "test_challenge".to_string(),
        };
        let debug = format!("{:?}", pkce);
        assert!(debug.contains("PkceCodes"));
        assert!(debug.contains("test_verifier"));
        assert!(debug.contains("test_challenge"));
    }

    #[test]
    fn test_pkce_verifier_is_url_safe() {
        let pkce = generate_pkce();
        // URL-safe base64 should not contain + or /
        assert!(!pkce.code_verifier.contains('+'));
        assert!(!pkce.code_verifier.contains('/'));
        assert!(!pkce.code_challenge.contains('+'));
        assert!(!pkce.code_challenge.contains('/'));
    }

    // =============================================
    // ServerOptions tests
    // =============================================

    #[test]
    fn test_server_options_new() {
        let opts = ServerOptions::new(
            PathBuf::from("/home/user/.codex"),
            None,
            AuthCredentialsStoreMode::File,
        );
        assert_eq!(opts.codex_home, PathBuf::from("/home/user/.codex"));
        assert_eq!(opts.client_id, CLIENT_ID);
        assert_eq!(opts.issuer, DEFAULT_ISSUER);
        assert_eq!(opts.port, DEFAULT_PORT);
        assert!(opts.open_browser);
        assert!(opts.force_state.is_none());
        assert!(opts.forced_chatgpt_workspace_id.is_none());
    }

    #[test]
    fn test_server_options_with_workspace_id() {
        let opts = ServerOptions::new(
            PathBuf::from("/home/user/.codex"),
            Some("workspace123".to_string()),
            AuthCredentialsStoreMode::File,
        );
        assert_eq!(
            opts.forced_chatgpt_workspace_id,
            Some("workspace123".to_string())
        );
    }

    #[test]
    fn test_server_options_clone() {
        let opts = ServerOptions::new(
            PathBuf::from("/home/user/.codex"),
            Some("ws".to_string()),
            AuthCredentialsStoreMode::File,
        );
        let opts_clone = opts.clone();
        assert_eq!(opts.codex_home, opts_clone.codex_home);
        assert_eq!(opts.client_id, opts_clone.client_id);
        assert_eq!(
            opts.forced_chatgpt_workspace_id,
            opts_clone.forced_chatgpt_workspace_id
        );
    }

    #[test]
    fn test_server_options_debug() {
        let opts = ServerOptions::new(PathBuf::from("/test"), None, AuthCredentialsStoreMode::File);
        let debug = format!("{:?}", opts);
        assert!(debug.contains("ServerOptions"));
        assert!(debug.contains("codex_home"));
    }

    // =============================================
    // generate_state tests
    // =============================================

    #[test]
    fn test_generate_state() {
        let state1 = generate_state();
        let state2 = generate_state();
        // States should be different
        assert_ne!(state1, state2);
        // State should be 43 chars (32 bytes base64 encoded without padding)
        assert_eq!(state1.len(), 43);
    }

    #[test]
    fn test_generate_state_url_safe() {
        let state = generate_state();
        // URL-safe base64 should not contain + or /
        assert!(!state.contains('+'));
        assert!(!state.contains('/'));
    }

    // =============================================
    // build_authorize_url tests
    // =============================================

    #[test]
    fn test_build_authorize_url() {
        let pkce = PkceCodes {
            code_verifier: "verifier".to_string(),
            code_challenge: "challenge".to_string(),
        };
        let url = build_authorize_url(
            "https://auth.example.com",
            "client123",
            "http://localhost:1455/auth/callback",
            &pkce,
            "state123",
            None,
        );
        assert!(url.contains("https://auth.example.com/oauth/authorize"));
        assert!(url.contains("client_id=client123"));
        assert!(url.contains("code_challenge=challenge"));
        assert!(url.contains("state=state123"));
    }

    #[test]
    fn test_build_authorize_url_with_workspace() {
        let pkce = PkceCodes {
            code_verifier: "verifier".to_string(),
            code_challenge: "challenge".to_string(),
        };
        let url = build_authorize_url(
            "https://auth.example.com",
            "client123",
            "http://localhost:1455/auth/callback",
            &pkce,
            "state123",
            Some("workspace456"),
        );
        assert!(url.contains("allowed_workspace_id=workspace456"));
    }

    #[test]
    fn test_build_authorize_url_contains_required_params() {
        let pkce = PkceCodes {
            code_verifier: "verifier".to_string(),
            code_challenge: "challenge".to_string(),
        };
        let url = build_authorize_url(
            DEFAULT_ISSUER,
            CLIENT_ID,
            "http://localhost:1455/auth/callback",
            &pkce,
            "teststate",
            None,
        );
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("originator=codex-dashflow"));
    }

    #[test]
    fn test_build_authorize_url_encodes_special_chars() {
        let pkce = PkceCodes {
            code_verifier: "verifier".to_string(),
            code_challenge: "challenge".to_string(),
        };
        let url = build_authorize_url(
            "https://auth.example.com",
            "client 123", // space in client_id
            "http://localhost:1455/auth/callback",
            &pkce,
            "state 123", // space in state
            None,
        );
        // Should URL-encode the spaces
        assert!(url.contains("client%20123"));
        assert!(url.contains("state%20123"));
    }

    // =============================================
    // jwt_auth_claims tests
    // =============================================

    #[test]
    fn test_jwt_auth_claims_invalid_format() {
        // JWT with only 1 part
        let claims = jwt_auth_claims("invalid");
        assert!(claims.is_empty());
    }

    #[test]
    fn test_jwt_auth_claims_empty_parts() {
        // JWT with empty parts
        let claims = jwt_auth_claims("..");
        assert!(claims.is_empty());
    }

    #[test]
    fn test_jwt_auth_claims_missing_auth_claim() {
        // Valid JWT structure but missing the openai auth claim
        // header.payload.signature where payload is {"foo": "bar"}
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"foo": "bar"}"#.as_bytes());
        let jwt = format!("header.{}.signature", payload);
        let claims = jwt_auth_claims(&jwt);
        assert!(claims.is_empty());
    }

    #[test]
    fn test_jwt_auth_claims_valid() {
        // Create a valid JWT payload with the expected structure
        let payload_json = r#"{"https://api.openai.com/auth": {"organization_id": "org123", "project_id": "proj456"}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let claims = jwt_auth_claims(&jwt);
        assert_eq!(
            claims.get("organization_id").and_then(|v| v.as_str()),
            Some("org123")
        );
        assert_eq!(
            claims.get("project_id").and_then(|v| v.as_str()),
            Some("proj456")
        );
    }

    #[test]
    fn test_jwt_auth_claims_invalid_base64() {
        // Invalid base64 in payload
        let jwt = "header.!!!invalid_base64!!!.signature";
        let claims = jwt_auth_claims(jwt);
        assert!(claims.is_empty());
    }

    #[test]
    fn test_jwt_auth_claims_invalid_json() {
        // Valid base64 but invalid JSON
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("not json".as_bytes());
        let jwt = format!("header.{}.signature", payload);
        let claims = jwt_auth_claims(&jwt);
        assert!(claims.is_empty());
    }

    // =============================================
    // parse_id_token tests
    // =============================================

    #[test]
    fn test_parse_id_token_missing_payload() {
        let result = parse_id_token("header_only");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_id_token_invalid_base64() {
        let result = parse_id_token("header.!!!invalid!!!.signature");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_id_token_invalid_json() {
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("not json".as_bytes());
        let jwt = format!("header.{}.signature", payload);
        let result = parse_id_token(&jwt);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_id_token_valid_with_email() {
        let payload_json = r#"{"email": "user@example.com", "exp": 1700000000}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = parse_id_token(&jwt).unwrap();
        assert_eq!(result.email, Some("user@example.com".to_string()));
        assert!(result.expires_at.is_some());
    }

    #[test]
    fn test_parse_id_token_without_email() {
        let payload_json = r#"{"sub": "user123", "exp": 1700000000}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = parse_id_token(&jwt).unwrap();
        assert!(result.email.is_none());
        assert!(result.expires_at.is_some());
    }

    #[test]
    fn test_parse_id_token_without_exp() {
        let payload_json = r#"{"email": "user@example.com"}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = parse_id_token(&jwt).unwrap();
        assert_eq!(result.email, Some("user@example.com".to_string()));
        assert!(result.expires_at.is_none());
    }

    #[test]
    fn test_parsed_id_token_clone() {
        let token = ParsedIdToken {
            email: Some("user@example.com".to_string()),
            expires_at: None,
        };
        let token_clone = token.clone();
        assert_eq!(token.email, token_clone.email);
    }

    #[test]
    fn test_parsed_id_token_debug() {
        let token = ParsedIdToken {
            email: Some("test@test.com".to_string()),
            expires_at: None,
        };
        let debug = format!("{:?}", token);
        assert!(debug.contains("ParsedIdToken"));
        assert!(debug.contains("test@test.com"));
    }

    // =============================================
    // ensure_workspace_allowed tests
    // =============================================

    #[test]
    fn test_ensure_workspace_allowed_no_restriction() {
        // When expected is None, any workspace is allowed
        let result = ensure_workspace_allowed(None, "any_token");
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_workspace_allowed_missing_claim() {
        // When expected is set but token doesn't have the claim
        let payload_json = r#"{"https://api.openai.com/auth": {}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = ensure_workspace_allowed(Some("expected_ws"), &jwt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("chatgpt_account_id"));
    }

    #[test]
    fn test_ensure_workspace_allowed_mismatch() {
        let payload_json = r#"{"https://api.openai.com/auth": {"chatgpt_account_id": "wrong_ws"}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = ensure_workspace_allowed(Some("expected_ws"), &jwt);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expected_ws"));
    }

    #[test]
    fn test_ensure_workspace_allowed_match() {
        let payload_json =
            r#"{"https://api.openai.com/auth": {"chatgpt_account_id": "correct_ws"}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let jwt = format!("header.{}.signature", payload);

        let result = ensure_workspace_allowed(Some("correct_ws"), &jwt);
        assert!(result.is_ok());
    }

    // =============================================
    // RefreshedTokens tests
    // =============================================

    #[test]
    fn test_refreshed_tokens_clone() {
        let tokens = RefreshedTokens {
            access_token: "access".to_string(),
            refresh_token: "refresh".to_string(),
            id_token: "id".to_string(),
        };
        let tokens_clone = tokens.clone();
        assert_eq!(tokens.access_token, tokens_clone.access_token);
        assert_eq!(tokens.refresh_token, tokens_clone.refresh_token);
        assert_eq!(tokens.id_token, tokens_clone.id_token);
    }

    #[test]
    fn test_refreshed_tokens_debug() {
        let tokens = RefreshedTokens {
            access_token: "acc123".to_string(),
            refresh_token: "ref456".to_string(),
            id_token: "id789".to_string(),
        };
        let debug = format!("{:?}", tokens);
        assert!(debug.contains("RefreshedTokens"));
        assert!(debug.contains("acc123"));
    }

    // =============================================
    // ShutdownHandle tests
    // =============================================

    #[test]
    fn test_shutdown_handle_clone() {
        let notify = Arc::new(tokio::sync::Notify::new());
        let handle = ShutdownHandle {
            shutdown_notify: notify.clone(),
        };
        let handle_clone = handle.clone();
        // Both should reference the same Notify
        assert!(Arc::ptr_eq(
            &handle.shutdown_notify,
            &handle_clone.shutdown_notify
        ));
    }

    #[test]
    fn test_shutdown_handle_debug() {
        let notify = Arc::new(tokio::sync::Notify::new());
        let handle = ShutdownHandle {
            shutdown_notify: notify,
        };
        let debug = format!("{:?}", handle);
        assert!(debug.contains("ShutdownHandle"));
    }

    // =============================================
    // compose_success_url tests
    // =============================================

    #[test]
    fn test_compose_success_url_default_issuer() {
        // Create a minimal JWT with the required claims
        let payload_json = r#"{"https://api.openai.com/auth": {"organization_id": "org1", "project_id": "proj1"}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let id_token = format!("header.{}.signature", payload);
        let access_token = format!("header.{}.signature", payload);

        let url = compose_success_url(1455, DEFAULT_ISSUER, &id_token, &access_token);
        assert!(url.starts_with("http://localhost:1455/success?"));
        assert!(url.contains("platform_url=https%3A%2F%2Fplatform.openai.com"));
    }

    #[test]
    fn test_compose_success_url_alternate_issuer() {
        let payload_json = r#"{"https://api.openai.com/auth": {}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let id_token = format!("header.{}.signature", payload);
        let access_token = format!("header.{}.signature", payload);

        let url = compose_success_url(8080, "https://other.auth.com", &id_token, &access_token);
        assert!(url.starts_with("http://localhost:8080/success?"));
        assert!(url.contains("platform_url=https%3A%2F%2Fplatform.api.openai.org"));
    }

    #[test]
    fn test_compose_success_url_with_needs_setup() {
        // Token where completed_platform_onboarding=false and is_org_owner=true
        let payload_json = r#"{"https://api.openai.com/auth": {"completed_platform_onboarding": false, "is_org_owner": true}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let id_token = format!("header.{}.signature", payload);
        let access_token = format!("header.{}.signature", payload);

        let url = compose_success_url(1455, DEFAULT_ISSUER, &id_token, &access_token);
        assert!(url.contains("needs_setup=true"));
    }

    #[test]
    fn test_compose_success_url_no_needs_setup() {
        // Token where completed_platform_onboarding=true
        let payload_json =
            r#"{"https://api.openai.com/auth": {"completed_platform_onboarding": true}}"#;
        let payload =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        let id_token = format!("header.{}.signature", payload);
        let access_token = format!("header.{}.signature", payload);

        let url = compose_success_url(1455, DEFAULT_ISSUER, &id_token, &access_token);
        assert!(url.contains("needs_setup=false"));
    }

    // =============================================
    // login_error_response tests
    // =============================================

    #[test]
    fn test_login_error_response() {
        let response = login_error_response("Test error message");
        match response {
            HandledRequest::ResponseAndExit {
                headers,
                body,
                result,
            } => {
                // Should have content-type header or body content
                assert!(!headers.is_empty() || !body.is_empty());
                // Body should contain the error message
                let body_str = String::from_utf8_lossy(&body);
                assert!(body_str.contains("Test error message"));
                // Result should be an error
                assert!(result.is_err());
            }
            _ => panic!("Expected ResponseAndExit"),
        }
    }
}
