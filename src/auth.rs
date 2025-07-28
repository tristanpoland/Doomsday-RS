use crate::config::AuthConfig;
use crate::types::{AuthRequest, AuthResponse};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait AuthProvider: Send + Sync {
    async fn authenticate(&self, request: &AuthRequest) -> crate::Result<AuthResponse>;
    async fn validate_token(&self, token: &str) -> crate::Result<bool>;
    async fn revoke_token(&self, token: &str) -> crate::Result<()>;
    fn requires_auth(&self) -> bool;
}

pub fn create_auth_provider(config: &AuthConfig) -> crate::Result<Arc<dyn AuthProvider>> {
    tracing::info!("Creating authentication provider: {}", config.auth_type);

    match config.auth_type.as_str() {
        "none" => {
            tracing::info!("No authentication required");
            Ok(Arc::new(NopAuthProvider::new()))
        }
        "userpass" => {
            tracing::info!("Setting up username/password authentication");
            let provider = UserPassAuthProvider::from_config(&config.properties)?;
            tracing::info!("Username/password authentication provider created");
            Ok(Arc::new(provider))
        }
        _ => {
            tracing::error!("Unknown authentication type: {}", config.auth_type);
            Err(crate::DoomsdayError::config(format!(
                "Unknown auth type: {}",
                config.auth_type
            )))
        }
    }
}

#[derive(Debug)]
pub struct NopAuthProvider;

impl NopAuthProvider {
    pub fn new() -> Self {
        NopAuthProvider
    }
}

#[async_trait]
impl AuthProvider for NopAuthProvider {
    async fn authenticate(&self, _request: &AuthRequest) -> crate::Result<AuthResponse> {
        Err(crate::DoomsdayError::auth("Authentication not required"))
    }

    async fn validate_token(&self, _token: &str) -> crate::Result<bool> {
        Ok(true) // No authentication required
    }

    async fn revoke_token(&self, _token: &str) -> crate::Result<()> {
        Ok(())
    }

    fn requires_auth(&self) -> bool {
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionInfo {
    username: String,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_used: DateTime<Utc>,
}

#[derive(Debug)]
pub struct UserPassAuthProvider {
    users: HashMap<String, String>, // username -> password hash
    sessions: Arc<DashMap<String, SessionInfo>>,
    session_timeout: Duration,
    refresh_on_use: bool,
}

impl UserPassAuthProvider {
    pub fn new(
        users: HashMap<String, String>,
        session_timeout: Duration,
        refresh_on_use: bool,
    ) -> Self {
        UserPassAuthProvider {
            users,
            sessions: Arc::new(DashMap::new()),
            session_timeout,
            refresh_on_use,
        }
    }

    pub fn from_config(properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        tracing::debug!("Configuring username/password authentication from properties");

        let users_config = properties
            .get("users")
            .and_then(|v| v.as_mapping())
            .ok_or_else(|| {
                crate::DoomsdayError::config("userpass auth requires users configuration")
            })?;

        let mut users = HashMap::new();
        tracing::info!(
            "Configuring {} users for authentication",
            users_config.len()
        );

        for (username, password) in users_config {
            let username_str = username
                .as_str()
                .ok_or_else(|| crate::DoomsdayError::config("Username must be a string"))?;
            let password_str = password
                .as_str()
                .ok_or_else(|| crate::DoomsdayError::config("Password must be a string"))?;

            tracing::debug!("Hashing password for user: {}", username_str);
            // Hash the password
            let password_hash = bcrypt::hash(password_str, bcrypt::DEFAULT_COST).map_err(|e| {
                crate::DoomsdayError::auth(format!("Failed to hash password: {}", e))
            })?;

            users.insert(username_str.to_string(), password_hash);
            tracing::debug!("User {} configured successfully", username_str);
        }

        let session_timeout_minutes = properties
            .get("session_timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(60); // Default 1 hour

        let refresh_on_use = properties
            .get("refresh_on_use")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        tracing::info!(
            "Authentication configured: session_timeout={}min, refresh_on_use={}",
            session_timeout_minutes,
            refresh_on_use
        );

        Ok(UserPassAuthProvider::new(
            users,
            Duration::minutes(session_timeout_minutes as i64),
            refresh_on_use,
        ))
    }

    fn cleanup_expired_sessions(&self) {
        let now = Utc::now();
        let expired_tokens: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| entry.expires_at < now)
            .map(|entry| entry.key().clone())
            .collect();

        if !expired_tokens.is_empty() {
            tracing::debug!("Cleaning up {} expired sessions", expired_tokens.len());
            for token in expired_tokens {
                if let Some((_, session)) = self.sessions.remove(&token) {
                    tracing::debug!("Removed expired session for user: {}", session.username);
                }
            }
        }
    }
}

#[async_trait]
impl AuthProvider for UserPassAuthProvider {
    async fn authenticate(&self, request: &AuthRequest) -> crate::Result<AuthResponse> {
        tracing::debug!("Authentication attempt for user: {}", request.username);
        self.cleanup_expired_sessions();

        let password_hash = self.users.get(&request.username).ok_or_else(|| {
            tracing::warn!("Authentication failed: user {} not found", request.username);
            crate::DoomsdayError::auth("Invalid credentials")
        })?;

        tracing::debug!("Verifying password for user: {}", request.username);
        let valid = bcrypt::verify(&request.password, password_hash).map_err(|e| {
            crate::DoomsdayError::auth(format!("Password verification failed: {}", e))
        })?;

        if !valid {
            tracing::warn!(
                "Authentication failed: invalid password for user {}",
                request.username
            );
            return Err(crate::DoomsdayError::auth("Invalid credentials"));
        }

        let token = Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + self.session_timeout;

        let session = SessionInfo {
            username: request.username.clone(),
            created_at: now,
            expires_at,
            last_used: now,
        };

        self.sessions.insert(token.clone(), session);

        tracing::info!(
            "Authentication successful for user: {} (token expires: {})",
            request.username,
            expires_at
        );

        Ok(AuthResponse { token, expires_at })
    }

    async fn validate_token(&self, token: &str) -> crate::Result<bool> {
        tracing::debug!("Validating token: {}...", &token[..8.min(token.len())]);
        self.cleanup_expired_sessions();

        if let Some(mut session) = self.sessions.get_mut(token) {
            let now = Utc::now();

            if session.expires_at < now {
                tracing::debug!("Token expired for user: {}", session.username);
                return Ok(false);
            }

            if self.refresh_on_use {
                session.last_used = now;
                session.expires_at = now + self.session_timeout;
                tracing::debug!(
                    "Token refreshed for user: {} (new expiry: {})",
                    session.username,
                    session.expires_at
                );
            }

            tracing::debug!("Token validation successful for user: {}", session.username);
            Ok(true)
        } else {
            tracing::debug!("Token not found in active sessions");
            Ok(false)
        }
    }

    async fn revoke_token(&self, token: &str) -> crate::Result<()> {
        self.sessions.remove(token);
        Ok(())
    }

    fn requires_auth(&self) -> bool {
        true
    }
}
