use std::fmt;

pub type Result<T> = std::result::Result<T, DoomsdayError>;

#[derive(Debug, thiserror::Error)]
pub enum DoomsdayError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    SerdeJson(#[from] serde_json::Error),
    
    #[error("YAML error: {0}")]
    SerdeYaml(#[from] serde_yaml::Error),
    
    #[error("HTTP error: {0}")]
    Reqwest(#[from] reqwest::Error),
    
    #[error("TLS error: {0}")]
    Rustls(#[from] rustls::Error),
    
    #[error("X509 parsing error: {0}")]
    X509(String),
    
    #[error("Authentication error: {0}")]
    Auth(String),
    
    #[error("Backend error: {0}")]
    Backend(String),
    
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("Cache error: {0}")]
    Cache(String),
    
    #[error("Scheduler error: {0}")]
    Scheduler(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl DoomsdayError {
    pub fn x509<T: fmt::Display>(msg: T) -> Self {
        Self::X509(msg.to_string())
    }
    
    pub fn auth<T: fmt::Display>(msg: T) -> Self {
        Self::Auth(msg.to_string())
    }
    
    pub fn backend<T: fmt::Display>(msg: T) -> Self {
        Self::Backend(msg.to_string())
    }
    
    pub fn config<T: fmt::Display>(msg: T) -> Self {
        Self::Config(msg.to_string())
    }
    
    pub fn cache<T: fmt::Display>(msg: T) -> Self {
        Self::Cache(msg.to_string())
    }
    
    pub fn scheduler<T: fmt::Display>(msg: T) -> Self {
        Self::Scheduler(msg.to_string())
    }
    
    pub fn not_found<T: fmt::Display>(msg: T) -> Self {
        Self::NotFound(msg.to_string())
    }
    
    pub fn permission_denied<T: fmt::Display>(msg: T) -> Self {
        Self::PermissionDenied(msg.to_string())
    }
    
    pub fn invalid_input<T: fmt::Display>(msg: T) -> Self {
        Self::InvalidInput(msg.to_string())
    }
    
    pub fn internal<T: fmt::Display>(msg: T) -> Self {
        Self::Internal(msg.to_string())
    }
}