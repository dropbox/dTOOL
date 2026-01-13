//! JWT parsing utilities for extracting claims from ID tokens
//!
//! Provides functionality to parse JWT tokens and extract useful claims
//! like email, plan type, and account ID.

use base64::Engine;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error type for ID token parsing
#[derive(Debug, Error)]
pub enum IdTokenError {
    /// The JWT format is invalid (doesn't have 3 parts)
    #[error("invalid ID token format")]
    InvalidFormat,
    /// Base64 decoding failed
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    /// JSON parsing failed
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Flat subset of useful claims extracted from an id_token JWT.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IdTokenInfo {
    /// Email from the JWT claims
    pub email: Option<String>,
    /// ChatGPT subscription plan type
    pub(crate) plan_type: Option<PlanType>,
    /// Organization/workspace identifier
    pub account_id: Option<String>,
    /// The raw JWT string
    pub raw_jwt: String,
}

impl IdTokenInfo {
    /// Get the ChatGPT plan type as a human-readable string
    pub fn get_plan_type(&self) -> Option<String> {
        self.plan_type.as_ref().map(|t| match t {
            PlanType::Known(plan) => format!("{plan:?}"),
            PlanType::Unknown(s) => s.clone(),
        })
    }
}

/// Plan type enum supporting both known and unknown values
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum PlanType {
    Known(KnownPlan),
    Unknown(String),
}

/// Known ChatGPT subscription plans
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KnownPlan {
    Free,
    Plus,
    Pro,
    Team,
    Business,
    Enterprise,
    Edu,
}

/// JWT payload claims structure
#[derive(Deserialize)]
struct IdClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<AuthClaims>,
}

/// OpenAI-specific auth claims nested in the JWT
#[derive(Deserialize)]
struct AuthClaims {
    #[serde(default)]
    chatgpt_plan_type: Option<PlanType>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

/// Parse a JWT ID token and extract useful claims.
///
/// JWT format: header.payload.signature (base64url encoded)
///
/// # Arguments
/// * `id_token` - The raw JWT string
///
/// # Returns
/// * `Ok(IdTokenInfo)` - Parsed token info
/// * `Err(IdTokenError)` - If parsing fails
pub fn parse_id_token(id_token: &str) -> Result<IdTokenInfo, IdTokenError> {
    // JWT format: header.payload.signature
    let mut parts = id_token.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(h), Some(p), Some(s)) if !h.is_empty() && !p.is_empty() && !s.is_empty() => (h, p, s),
        _ => return Err(IdTokenError::InvalidFormat),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload_b64)?;
    let claims: IdClaims = serde_json::from_slice(&payload_bytes)?;

    match claims.auth {
        Some(auth) => Ok(IdTokenInfo {
            email: claims.email,
            raw_jwt: id_token.to_string(),
            plan_type: auth.chatgpt_plan_type,
            account_id: auth.chatgpt_account_id,
        }),
        None => Ok(IdTokenInfo {
            email: claims.email,
            raw_jwt: id_token.to_string(),
            plan_type: None,
            account_id: None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    fn b64url_no_pad(bytes: &[u8]) -> String {
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    }

    fn make_jwt(payload: &serde_json::Value) -> String {
        #[derive(Serialize)]
        struct Header {
            alg: &'static str,
            typ: &'static str,
        }
        let header = Header {
            alg: "none",
            typ: "JWT",
        };
        let header_b64 = b64url_no_pad(&serde_json::to_vec(&header).unwrap());
        let payload_b64 = b64url_no_pad(&serde_json::to_vec(payload).unwrap());
        let signature_b64 = b64url_no_pad(b"sig");
        format!("{header_b64}.{payload_b64}.{signature_b64}")
    }

    #[test]
    fn test_parse_id_token_with_email_and_plan() {
        let payload = serde_json::json!({
            "email": "user@example.com",
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "pro"
            }
        });
        let jwt = make_jwt(&payload);

        let info = parse_id_token(&jwt).expect("should parse");
        assert_eq!(info.email.as_deref(), Some("user@example.com"));
        assert_eq!(info.get_plan_type().as_deref(), Some("Pro"));
    }

    #[test]
    fn test_parse_id_token_with_account_id() {
        let payload = serde_json::json!({
            "email": "user@company.com",
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "enterprise",
                "chatgpt_account_id": "org_abc123"
            }
        });
        let jwt = make_jwt(&payload);

        let info = parse_id_token(&jwt).expect("should parse");
        assert_eq!(info.account_id.as_deref(), Some("org_abc123"));
        assert_eq!(info.get_plan_type().as_deref(), Some("Enterprise"));
    }

    #[test]
    fn test_parse_id_token_missing_fields() {
        let payload = serde_json::json!({ "sub": "123" });
        let jwt = make_jwt(&payload);

        let info = parse_id_token(&jwt).expect("should parse");
        assert!(info.email.is_none());
        assert!(info.get_plan_type().is_none());
        assert!(info.account_id.is_none());
    }

    #[test]
    fn test_parse_id_token_unknown_plan_type() {
        let payload = serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_plan_type": "experimental_tier"
            }
        });
        let jwt = make_jwt(&payload);

        let info = parse_id_token(&jwt).expect("should parse");
        assert_eq!(info.get_plan_type().as_deref(), Some("experimental_tier"));
    }

    #[test]
    fn test_parse_id_token_all_known_plans() {
        for plan in [
            "free",
            "plus",
            "pro",
            "team",
            "business",
            "enterprise",
            "edu",
        ] {
            let payload = serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_plan_type": plan
                }
            });
            let jwt = make_jwt(&payload);
            let info = parse_id_token(&jwt).expect("should parse");
            assert!(info.get_plan_type().is_some());
        }
    }

    #[test]
    fn test_parse_id_token_invalid_format_empty() {
        let result = parse_id_token("");
        assert!(matches!(result, Err(IdTokenError::InvalidFormat)));
    }

    #[test]
    fn test_parse_id_token_invalid_format_one_part() {
        let result = parse_id_token("onlyonepart");
        assert!(matches!(result, Err(IdTokenError::InvalidFormat)));
    }

    #[test]
    fn test_parse_id_token_invalid_format_two_parts() {
        let result = parse_id_token("two.parts");
        assert!(matches!(result, Err(IdTokenError::InvalidFormat)));
    }

    #[test]
    fn test_parse_id_token_invalid_format_empty_parts() {
        let result = parse_id_token("..");
        assert!(matches!(result, Err(IdTokenError::InvalidFormat)));
    }

    #[test]
    fn test_parse_id_token_invalid_base64() {
        // Invalid base64 in payload
        let result = parse_id_token("header.!!!invalid!!!.sig");
        assert!(matches!(result, Err(IdTokenError::Base64(_))));
    }

    #[test]
    fn test_parse_id_token_invalid_json() {
        // Valid base64, but not valid JSON
        let invalid_json = b64url_no_pad(b"not json");
        let result = parse_id_token(&format!("header.{invalid_json}.sig"));
        assert!(matches!(result, Err(IdTokenError::Json(_))));
    }

    #[test]
    fn test_id_token_info_default() {
        let info = IdTokenInfo::default();
        assert!(info.email.is_none());
        assert!(info.get_plan_type().is_none());
        assert!(info.account_id.is_none());
        assert!(info.raw_jwt.is_empty());
    }

    #[test]
    fn test_id_token_info_serialization() {
        let info = IdTokenInfo {
            email: Some("test@example.com".to_string()),
            plan_type: None,
            account_id: None,
            raw_jwt: "fake.jwt.here".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: IdTokenInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info, parsed);
    }

    #[test]
    fn test_id_token_error_display() {
        let err = IdTokenError::InvalidFormat;
        assert_eq!(err.to_string(), "invalid ID token format");
    }
}
