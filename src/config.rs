use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub backends: Vec<BackendConfig>,
    pub server: ServerConfig,
    pub notifications: Option<NotificationConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub name: String,
    pub refresh_interval: Option<u64>, // minutes
    pub properties: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub tls: Option<TlsConfig>,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub auth_type: String,
    pub properties: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub doomsday_url: String,
    pub backend: NotificationBackend,
    pub schedule: ScheduleConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationBackend {
    #[serde(rename = "type")]
    pub backend_type: String,
    pub properties: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    #[serde(rename = "type")]
    pub schedule_type: String,
    pub properties: HashMap<String, serde_yaml::Value>,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> crate::Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    pub fn default() -> Self {
        Config {
            backends: vec![],
            server: ServerConfig {
                port: 8111,
                tls: None,
                auth: AuthConfig {
                    auth_type: "none".to_string(),
                    properties: HashMap::new(),
                },
            },
            notifications: None,
        }
    }

    pub fn validate(&self) -> crate::Result<()> {
        if self.backends.is_empty() {
            return Err(crate::DoomsdayError::config(
                "At least one backend must be configured",
            ));
        }

        for backend in &self.backends {
            if backend.name.is_empty() {
                return Err(crate::DoomsdayError::config("Backend name cannot be empty"));
            }

            match backend.backend_type.as_str() {
                "vault" | "credhub" | "opsmgr" | "tlsclient" => {}
                _ => {
                    return Err(crate::DoomsdayError::config(format!(
                        "Unknown backend type: {}",
                        backend.backend_type
                    )))
                }
            }
        }

        match self.server.auth.auth_type.as_str() {
            "none" | "userpass" => {}
            _ => {
                return Err(crate::DoomsdayError::config(format!(
                    "Unknown auth type: {}",
                    self.server.auth.auth_type
                )))
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub targets: HashMap<String, ClientTarget>,
    pub current_target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientTarget {
    pub name: String,
    pub address: String,
    pub skip_verify: bool,
    pub token: Option<String>,
    pub token_expires: Option<chrono::DateTime<chrono::Utc>>,
}

impl ClientConfig {
    pub fn load() -> crate::Result<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| crate::DoomsdayError::config("Could not find config directory"))?;

        let config_path = config_dir.join("doomsday").join("config.yml");

        if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            let config: ClientConfig = serde_yaml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(ClientConfig {
                targets: HashMap::new(),
                current_target: None,
            })
        }
    }

    pub fn save(&self) -> crate::Result<()> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| crate::DoomsdayError::config("Could not find config directory"))?;

        let doomsday_dir = config_dir.join("doomsday");
        fs::create_dir_all(&doomsday_dir)?;

        let config_path = doomsday_dir.join("config.yml");
        let content = serde_yaml::to_string(self)?;
        fs::write(&config_path, content)?;

        Ok(())
    }

    pub fn current_target(&self) -> Option<&ClientTarget> {
        self.current_target
            .as_ref()
            .and_then(|name| self.targets.get(name))
    }
}
