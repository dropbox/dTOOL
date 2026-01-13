//! Authentication and authorization module
//!
//! Implements HIPAA §164.312(a)(1) Access Controls:
//! - Unique user identification (JWT with `user_id`)
//! - Role-based access control (RBAC)
//! - Automatic logoff after inactivity
//! - Encryption of authentication credentials

use crate::error::{Error, Result};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// User roles for role-based access control
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    /// AI agents can execute WASM code
    Agent,
    /// Administrators can manage policies and access all functions
    Administrator,
    /// Auditors have read-only access to audit logs
    Auditor,
}

impl Role {
    /// Check if this role can execute WASM code
    #[must_use]
    pub fn can_execute(&self) -> bool {
        matches!(self, Role::Agent | Role::Administrator)
    }

    /// Check if this role can access audit logs
    #[must_use]
    pub fn can_audit(&self) -> bool {
        matches!(self, Role::Administrator | Role::Auditor)
    }

    /// Check if this role can manage policies
    #[must_use]
    pub fn can_administrate(&self) -> bool {
        matches!(self, Role::Administrator)
    }
}

/// JWT claims structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User ID (HIPAA: Unique user identification)
    pub sub: String,

    /// User role for RBAC
    pub role: Role,

    /// Issued at (Unix timestamp)
    pub iat: i64,

    /// Expiry time (Unix timestamp)
    /// HIPAA: Automatic logoff
    pub exp: i64,

    /// Session ID for tracing
    pub session_id: String,
}

impl Claims {
    /// Create new claims for a user
    #[must_use]
    pub fn new(user_id: String, role: Role, session_id: String, expiry_minutes: i64) -> Self {
        let now = Utc::now();
        let expiry = now + Duration::minutes(expiry_minutes);

        Self {
            sub: user_id,
            role,
            iat: now.timestamp(),
            exp: expiry.timestamp(),
            session_id,
        }
    }

    /// Check if the token has expired
    #[must_use]
    pub fn is_expired(&self) -> bool {
        Utc::now().timestamp() > self.exp
    }
}

/// Authentication context
///
/// Handles JWT token generation and validation
#[derive(Clone)]
pub struct AuthContext {
    /// JWT secret key
    secret: String,

    /// Token expiry duration (minutes)
    expiry_minutes: i64,

    /// JWT algorithm
    algorithm: Algorithm,
}

impl AuthContext {
    /// Create new authentication context
    ///
    /// # Arguments
    /// * `secret` - JWT secret key (must be at least 32 characters)
    /// * `expiry_minutes` - Token expiry duration in minutes
    pub fn new(secret: String, expiry_minutes: i64) -> Result<Self> {
        if secret.len() < 32 {
            return Err(Error::Configuration(
                "JWT secret must be at least 32 characters".to_string(),
            ));
        }

        Ok(Self {
            secret,
            expiry_minutes,
            algorithm: Algorithm::HS256,
        })
    }

    /// Generate JWT token for a user
    ///
    /// # HIPAA Compliance
    /// - §164.312(d): Unique user identification via JWT sub claim
    /// - §164.312(a)(2)(iii): Automatic logoff via exp claim
    pub fn generate_token(
        &self,
        user_id: String,
        role: Role,
        session_id: String,
    ) -> Result<String> {
        let claims = Claims::new(user_id, role, session_id, self.expiry_minutes);

        let header = Header::new(self.algorithm);
        let encoding_key = EncodingKey::from_secret(self.secret.as_bytes());

        encode(&header, &claims, &encoding_key).map_err(Error::from)
    }

    /// Verify and decode JWT token
    ///
    /// Returns the claims if token is valid and not expired
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let decoding_key = DecodingKey::from_secret(self.secret.as_bytes());
        let mut validation = Validation::new(self.algorithm);
        validation.validate_exp = true;
        validation.leeway = 60; // 60 seconds leeway for clock skew

        let token_data = decode::<Claims>(token, &decoding_key, &validation)?;

        // Double-check expiry (defense in depth)
        if token_data.claims.is_expired() {
            return Err(Error::Authentication("Token expired".to_string()));
        }

        Ok(token_data.claims)
    }

    /// Verify token and check if user has required role
    ///
    /// # HIPAA Compliance
    /// - §164.312(a)(1): Access control enforcement
    pub fn verify_access(
        &self,
        token: &str,
        required_capability: impl Fn(&Role) -> bool,
    ) -> Result<Claims> {
        let claims = self.verify_token(token)?;

        if !required_capability(&claims.role) {
            return Err(Error::Authorization(format!(
                "User {} with role {:?} does not have required permissions",
                claims.sub, claims.role
            )));
        }

        Ok(claims)
    }

    /// Verify token and check if user can execute WASM
    pub fn verify_execute_access(&self, token: &str) -> Result<Claims> {
        self.verify_access(token, Role::can_execute)
    }

    /// Verify token and check if user can access audit logs
    pub fn verify_audit_access(&self, token: &str) -> Result<Claims> {
        self.verify_access(token, Role::can_audit)
    }

    /// Verify token and check if user can administrate
    pub fn verify_admin_access(&self, token: &str) -> Result<Claims> {
        self.verify_access(token, Role::can_administrate)
    }
}

#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn setup_auth() -> AuthContext {
        AuthContext::new(
            "test-secret-that-is-at-least-32-characters-long".to_string(),
            30,
        )
        .unwrap()
    }

    #[test]
    fn test_generate_and_verify_token() {
        let auth = setup_auth();
        let session_id = Uuid::new_v4().to_string();

        let token = auth
            .generate_token("user123".to_string(), Role::Agent, session_id)
            .unwrap();

        let claims = auth.verify_token(&token).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.role, Role::Agent);
        assert!(!claims.is_expired());
    }

    #[test]
    fn test_invalid_token() {
        let auth = setup_auth();
        let result = auth.verify_token("invalid.token.here");
        assert!(result.is_err());
    }

    #[test]
    fn test_expired_token() {
        let auth = AuthContext::new(
            "test-secret-that-is-at-least-32-characters-long".to_string(),
            -1, // Expired 1 minute ago
        )
        .unwrap();

        let session_id = Uuid::new_v4().to_string();
        let token = auth
            .generate_token("user123".to_string(), Role::Agent, session_id)
            .unwrap();

        // Token should be expired
        let result = auth.verify_token(&token);
        assert!(result.is_err());
    }

    #[test]
    fn test_role_permissions() {
        assert!(Role::Agent.can_execute());
        assert!(!Role::Agent.can_administrate());

        assert!(Role::Administrator.can_execute());
        assert!(Role::Administrator.can_audit());
        assert!(Role::Administrator.can_administrate());

        assert!(!Role::Auditor.can_execute());
        assert!(Role::Auditor.can_audit());
        assert!(!Role::Auditor.can_administrate());
    }

    #[test]
    fn test_verify_execute_access() {
        let auth = setup_auth();
        let session_id = Uuid::new_v4().to_string();

        // Agent can execute
        let token = auth
            .generate_token("agent1".to_string(), Role::Agent, session_id.clone())
            .unwrap();
        assert!(auth.verify_execute_access(&token).is_ok());

        // Auditor cannot execute
        let token = auth
            .generate_token("auditor1".to_string(), Role::Auditor, session_id)
            .unwrap();
        assert!(auth.verify_execute_access(&token).is_err());
    }

    #[test]
    fn test_short_secret() {
        let result = AuthContext::new("short".to_string(), 30);
        assert!(result.is_err());
    }
}
