use crate::config::NotificationConfig;
use crate::types::CacheItem;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use serde_json::json;
use std::collections::HashMap;

#[async_trait]
pub trait NotificationBackend: Send + Sync {
    async fn send_notification(&self, message: &NotificationMessage) -> crate::Result<()>;
}

#[derive(Debug, Clone)]
pub struct NotificationMessage {
    pub title: String,
    pub body: String,
    pub urgency: NotificationUrgency,
    pub certificates: Vec<CacheItem>,
}

#[derive(Debug, Clone)]
pub enum NotificationUrgency {
    Low,
    Normal,
    High,
    Critical,
}

pub fn create_notification_backend(
    backend_type: &str,
    properties: &HashMap<String, serde_yaml::Value>,
) -> crate::Result<Box<dyn NotificationBackend>> {
    match backend_type {
        "slack" => {
            let backend = SlackNotificationBackend::from_config(properties)?;
            Ok(Box::new(backend))
        },
        "shout" => {
            let backend = ShoutNotificationBackend::from_config(properties)?;
            Ok(Box::new(backend))
        },
        _ => Err(crate::DoomsdayError::config(
            format!("Unknown notification backend: {}", backend_type)
        )),
    }
}

pub struct NotificationService {
    backend: Box<dyn NotificationBackend>,
    doomsday_url: String,
}

impl NotificationService {
    pub fn new(config: &NotificationConfig) -> crate::Result<Self> {
        let backend = create_notification_backend(
            &config.backend.backend_type,
            &config.backend.properties,
        )?;
        
        Ok(NotificationService {
            backend,
            doomsday_url: config.doomsday_url.clone(),
        })
    }
    
    pub async fn check_and_notify(&self, certificates: &[CacheItem]) -> crate::Result<()> {
        let now = Utc::now();
        
        let expired: Vec<CacheItem> = certificates.iter()
            .filter(|cert| cert.not_after < now)
            .cloned()
            .collect();
        
        let expiring_soon: Vec<CacheItem> = certificates.iter()
            .filter(|cert| {
                let days_until_expiry = (cert.not_after - now).num_days();
                days_until_expiry > 0 && days_until_expiry <= 30
            })
            .cloned()
            .collect();
        
        if !expired.is_empty() {
            let message = NotificationMessage {
                title: "⚠️ Expired Certificates".to_string(),
                body: format!(
                    "{} certificate(s) have expired. Please check {} for details.",
                    expired.len(),
                    self.doomsday_url
                ),
                urgency: NotificationUrgency::Critical,
                certificates: expired,
            };
            
            self.backend.send_notification(&message).await?;
        }
        
        if !expiring_soon.is_empty() {
            let message = NotificationMessage {
                title: "⏰ Certificates Expiring Soon".to_string(),
                body: format!(
                    "{} certificate(s) will expire within 30 days. Please check {} for details.",
                    expiring_soon.len(),
                    self.doomsday_url
                ),
                urgency: NotificationUrgency::High,
                certificates: expiring_soon,
            };
            
            self.backend.send_notification(&message).await?;
        }
        
        Ok(())
    }
}

pub struct SlackNotificationBackend {
    webhook_url: String,
    channel: Option<String>,
    username: Option<String>,
    client: reqwest::Client,
}

impl SlackNotificationBackend {
    pub fn new(webhook_url: String, channel: Option<String>, username: Option<String>) -> Self {
        SlackNotificationBackend {
            webhook_url,
            channel,
            username,
            client: reqwest::Client::new(),
        }
    }
    
    pub fn from_config(properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let webhook_url = properties.get("webhook_url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Slack webhook_url is required"))?;
        
        let channel = properties.get("channel")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        let username = properties.get("username")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        
        Ok(SlackNotificationBackend::new(
            webhook_url.to_string(),
            channel,
            username,
        ))
    }
}

#[async_trait]
impl NotificationBackend for SlackNotificationBackend {
    async fn send_notification(&self, message: &NotificationMessage) -> crate::Result<()> {
        let color = match message.urgency {
            NotificationUrgency::Low => "#36a64f",     // Green
            NotificationUrgency::Normal => "#2196F3",   // Blue
            NotificationUrgency::High => "#ff9800",     // Orange
            NotificationUrgency::Critical => "#f44336", // Red
        };
        
        let mut fields = vec![];
        
        // Group certificates by expiry status
        let now = Utc::now();
        let mut expired_count = 0;
        let mut expiring_soon_count = 0;
        
        for cert in &message.certificates {
            if cert.not_after < now {
                expired_count += 1;
            } else {
                expiring_soon_count += 1;
            }
        }
        
        if expired_count > 0 {
            fields.push(json!({
                "title": "Expired",
                "value": format!("{} certificates", expired_count),
                "short": true
            }));
        }
        
        if expiring_soon_count > 0 {
            fields.push(json!({
                "title": "Expiring Soon",
                "value": format!("{} certificates", expiring_soon_count),
                "short": true
            }));
        }
        
        let mut payload = json!({
            "text": message.title,
            "attachments": [{
                "color": color,
                "text": message.body,
                "fields": fields,
                "footer": "Doomsday Certificate Monitor",
                "ts": Utc::now().timestamp()
            }]
        });
        
        if let Some(channel) = &self.channel {
            payload["channel"] = json!(channel);
        }
        
        if let Some(username) = &self.username {
            payload["username"] = json!(username);
        }
        
        let response = self.client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::internal(
                format!("Slack notification failed: {}", response.status())
            ));
        }
        
        Ok(())
    }
}

pub struct ShoutNotificationBackend {
    url: String,
    client: reqwest::Client,
}

impl ShoutNotificationBackend {
    pub fn new(url: String) -> Self {
        ShoutNotificationBackend {
            url,
            client: reqwest::Client::new(),
        }
    }
    
    pub fn from_config(properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let url = properties.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Shout URL is required"))?;
        
        Ok(ShoutNotificationBackend::new(url.to_string()))
    }
}

#[async_trait]
impl NotificationBackend for ShoutNotificationBackend {
    async fn send_notification(&self, message: &NotificationMessage) -> crate::Result<()> {
        let payload = json!({
            "title": message.title,
            "body": message.body,
            "urgency": match message.urgency {
                NotificationUrgency::Low => "low",
                NotificationUrgency::Normal => "normal",
                NotificationUrgency::High => "high",
                NotificationUrgency::Critical => "critical",
            },
            "certificates": message.certificates.len(),
            "timestamp": Utc::now().to_rfc3339(),
        });
        
        let response = self.client
            .post(&self.url)
            .json(&payload)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::internal(
                format!("Shout notification failed: {}", response.status())
            ));
        }
        
        Ok(())
    }
}