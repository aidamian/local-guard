#![warn(missing_docs)]
//! # local-guard-auth
//!
//! ## Purpose
//! Implements authentication primitives and session lifecycle handling for
//! `local-guard`.
//!
//! ## Responsibilities
//! - Validate auth endpoint policy (`/r1/cstore-auth`, HTTPS).
//! - Execute login requests through an injectable transport abstraction.
//! - Model safe session transitions used to gate capture.
//!
//! ## Data flow
//! UI collects credentials -> [`AuthClient::login`] sends request through
//! [`AuthTransport`] -> receives [`SessionToken`] -> [`AuthStateMachine`]
//! updates runtime state.
//!
//! ## Ownership and lifetimes
//! Token/session values are owned (`String`) to decouple transport and runtime
//! state machine lifetimes.
//!
//! ## Error model
//! Endpoint policy violations and transport failures are surfaced as
//! [`AuthError`], allowing the app to either prompt reauth or block capture.
//!
//! ## Security and privacy notes
//! This crate does not log credentials or token values.
//! Callers are expected to keep credential inputs ephemeral.
//!
//! ## Example
//! ```rust
//! use local_guard_auth::{AuthStateMachine, AuthState};
//!
//! let machine = AuthStateMachine::new();
//! assert!(matches!(machine.state(), AuthState::Unauthenticated));
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

/// Required auth path suffix for v1.
pub const REQUIRED_AUTH_PATH: &str = "/r1/cstore-auth";

/// User-provided login credentials.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credentials {
    /// Account username.
    pub username: String,
    /// Account password or secret.
    pub password: String,
}

/// Login request payload forwarded to auth transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginRequest {
    /// Username for account lookup.
    pub username: String,
    /// Password/secret for auth verification.
    pub password: String,
}

/// Login response payload returned by auth transport.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginResponse {
    /// Access token used for protected API calls.
    pub access_token: String,
    /// Server-issued session identifier.
    pub session_id: String,
    /// Lifetime duration in seconds.
    pub expires_in_seconds: u64,
}

/// Session token with absolute expiry timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionToken {
    /// Bearer token used by downstream APIs.
    pub access_token: String,
    /// Session id propagated to payload metadata.
    pub session_id: String,
    /// Absolute epoch milliseconds when token expires.
    pub expires_at_ms: u64,
}

impl SessionToken {
    /// Returns `true` when token has expired at `now_ms`.
    pub fn is_expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at_ms
    }
}

/// Runtime authentication state used by capture guard logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthState {
    /// No valid session exists.
    Unauthenticated,
    /// Session is currently valid.
    Authenticated(SessionToken),
    /// Session expired or invalid; reauth is required.
    ReauthRequired,
}

/// Auth state machine with explicit legal transitions.
#[derive(Debug, Clone)]
pub struct AuthStateMachine {
    state: AuthState,
}

impl AuthStateMachine {
    /// Creates a new state machine in `Unauthenticated` state.
    pub fn new() -> Self {
        Self {
            state: AuthState::Unauthenticated,
        }
    }

    /// Returns current auth state snapshot.
    pub fn state(&self) -> &AuthState {
        &self.state
    }

    /// Applies login success transition.
    pub fn on_login_success(&mut self, token: SessionToken) {
        self.state = AuthState::Authenticated(token);
    }

    /// Re-evaluates state based on token expiry.
    pub fn on_tick(&mut self, now_ms: u64) {
        if let AuthState::Authenticated(token) = &self.state
            && token.is_expired(now_ms)
        {
            self.state = AuthState::ReauthRequired;
        }
    }

    /// Explicit logout transition.
    pub fn logout(&mut self) {
        self.state = AuthState::Unauthenticated;
    }

    /// Returns `true` when capture is allowed.
    pub fn can_capture(&self, now_ms: u64) -> bool {
        matches!(
            &self.state,
            AuthState::Authenticated(token) if !token.is_expired(now_ms)
        )
    }
}

impl Default for AuthStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

/// Abstract transport used by auth client.
pub trait AuthTransport: Send + Sync {
    /// Sends login request to auth backend.
    fn authenticate(
        &self,
        endpoint: &str,
        request: &LoginRequest,
    ) -> Result<LoginResponse, AuthError>;
}

/// Auth client that validates endpoint policy and executes login flow.
#[derive(Clone)]
pub struct AuthClient {
    endpoint: String,
    transport: Arc<dyn AuthTransport>,
}

impl AuthClient {
    /// Creates a validated auth client.
    ///
    /// # Errors
    /// Returns [`AuthError::InvalidEndpoint`] when URL is not HTTPS or does not
    /// include required `/r1/cstore-auth` path.
    pub fn new(
        endpoint: impl Into<String>,
        transport: Arc<dyn AuthTransport>,
    ) -> Result<Self, AuthError> {
        let endpoint = endpoint.into();
        validate_auth_endpoint(&endpoint)?;
        Ok(Self {
            endpoint,
            transport,
        })
    }

    /// Executes login and converts server response into a session token.
    ///
    /// # Errors
    /// Returns [`AuthError::EmptyCredential`] for blank username/password.
    /// Propagates transport errors as-is for caller retry/prompt behavior.
    pub fn login(&self, credentials: &Credentials, now_ms: u64) -> Result<SessionToken, AuthError> {
        if credentials.username.trim().is_empty() || credentials.password.trim().is_empty() {
            return Err(AuthError::EmptyCredential);
        }

        let response = self.transport.authenticate(
            &self.endpoint,
            &LoginRequest {
                username: credentials.username.clone(),
                password: credentials.password.clone(),
            },
        )?;

        if response.access_token.trim().is_empty() || response.session_id.trim().is_empty() {
            return Err(AuthError::InvalidResponse(
                "response missing token or session id".to_string(),
            ));
        }

        let expires_at_ms = now_ms.saturating_add(response.expires_in_seconds.saturating_mul(1000));

        Ok(SessionToken {
            access_token: response.access_token,
            session_id: response.session_id,
            expires_at_ms,
        })
    }

    /// Returns configured auth endpoint.
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

/// Validates v1 auth endpoint constraints.
///
/// # Errors
/// Returns [`AuthError::InvalidEndpoint`] for non-HTTPS or path mismatch.
pub fn validate_auth_endpoint(endpoint: &str) -> Result<(), AuthError> {
    let parsed = Url::parse(endpoint)
        .map_err(|error| AuthError::InvalidEndpoint(format!("invalid auth url: {error}")))?;

    if parsed.scheme() != "https" {
        return Err(AuthError::InvalidEndpoint(
            "auth endpoint must use https".to_string(),
        ));
    }

    if !parsed.path().ends_with(REQUIRED_AUTH_PATH) {
        return Err(AuthError::InvalidEndpoint(format!(
            "auth endpoint path must end with {REQUIRED_AUTH_PATH}"
        )));
    }

    Ok(())
}

/// Errors produced by auth client/state logic.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Endpoint violates security or contract requirements.
    #[error("invalid endpoint: {0}")]
    InvalidEndpoint(String),
    /// Credentials are missing/blank.
    #[error("username and password must be non-empty")]
    EmptyCredential,
    /// Transport failure from auth backend.
    #[error("auth transport failure: {0}")]
    Transport(String),
    /// Response payload violated auth contract expectations.
    #[error("invalid auth response: {0}")]
    InvalidResponse(String),
}

#[cfg(test)]
mod tests {
    //! Unit tests for auth endpoint and state transitions.

    use super::*;

    #[test]
    fn validates_expected_endpoint_policy() {
        validate_auth_endpoint("https://example.test/r1/cstore-auth")
            .expect("endpoint should pass");
        assert!(validate_auth_endpoint("http://example.test/r1/cstore-auth").is_err());
        assert!(validate_auth_endpoint("https://example.test/r2/other").is_err());
    }

    #[test]
    fn state_machine_requires_reauth_after_expiry() {
        let mut machine = AuthStateMachine::new();
        machine.on_login_success(SessionToken {
            access_token: "token".to_string(),
            session_id: "session".to_string(),
            expires_at_ms: 1_000,
        });
        machine.on_tick(1_001);
        assert!(matches!(machine.state(), AuthState::ReauthRequired));
    }
}
