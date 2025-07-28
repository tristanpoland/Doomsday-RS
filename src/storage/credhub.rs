use crate::storage::Accessor;
use crate::types::{CertificateData, PathList};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use x509_parser::prelude::*;

#[derive(Debug, Clone)]
pub struct CredHubAccessor {
    name: String,
    client: Client,
    base_url: Url,
    client_id: String,
    client_secret: String,
    access_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredHubCredentialsResponse {
    credentials: Vec<CredHubCredential>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredHubCredential {
    name: String,
    #[serde(rename = "type")]
    credential_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredHubValueResponse {
    #[serde(rename = "type")]
    credential_type: String,
    value: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredHubTokenRequest {
    grant_type: String,
    client_id: String,
    client_secret: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CredHubTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

impl CredHubAccessor {
    pub fn new(
        name: String,
        base_url: Url,
        client_id: String,
        client_secret: String,
    ) -> crate::Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(false)
            .build()?;
        
        Ok(CredHubAccessor {
            name,
            client,
            base_url,
            client_id,
            client_secret,
            access_token: None,
        })
    }
    
    pub fn from_config(name: String, properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let url = properties.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("CredHub URL is required"))?;
        
        let client_id = properties.get("client_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("CredHub client_id is required"))?;
        
        let client_secret = properties.get("client_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("CredHub client_secret is required"))?;
        
        let base_url = Url::parse(url)
            .map_err(|e| crate::DoomsdayError::config(format!("Invalid CredHub URL: {}", e)))?;
        
        Self::new(
            name,
            base_url,
            client_id.to_string(),
            client_secret.to_string(),
        )
    }
    
    async fn ensure_authenticated(&mut self) -> crate::Result<()> {
        if self.access_token.is_some() {
            return Ok(());
        }
        
        let token_url = format!(
            "{}/oauth/token",
            self.base_url.as_str().trim_end_matches('/')
        );
        
        let token_request = CredHubTokenRequest {
            grant_type: "client_credentials".to_string(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
        };
        
        let response = self.client
            .post(&token_url)
            .json(&token_request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::auth("Failed to authenticate with CredHub"));
        }
        
        let token_response: CredHubTokenResponse = response.json().await?;
        self.access_token = Some(token_response.access_token);
        
        Ok(())
    }
    
    async fn get_auth_header(&mut self) -> crate::Result<String> {
        self.ensure_authenticated().await?;
        Ok(format!("Bearer {}", self.access_token.as_ref().unwrap()))
    }
}

#[async_trait]
impl Accessor for CredHubAccessor {
    async fn list(&self) -> crate::Result<PathList> {
        let mut accessor = self.clone();
        let auth_header = accessor.get_auth_header().await?;
        
        let url = format!(
            "{}/api/v1/credentials",
            self.base_url.as_str().trim_end_matches('/')
        );
        
        let response = self.client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::backend("Failed to list credentials from CredHub"));
        }
        
        let credentials_response: CredHubCredentialsResponse = response.json().await?;
        
        let certificate_paths: Vec<String> = credentials_response
            .credentials
            .into_iter()
            .filter(|cred| cred.credential_type == "certificate")
            .map(|cred| cred.name)
            .collect();
        
        Ok(certificate_paths)
    }
    
    async fn get(&self, path: &str) -> crate::Result<Option<CertificateData>> {
        let mut accessor = self.clone();
        let auth_header = accessor.get_auth_header().await?;
        
        let url = format!(
            "{}/api/v1/credentials?name={}",
            self.base_url.as_str().trim_end_matches('/'),
            urlencoding::encode(path)
        );
        
        let response = self.client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let value_response: CredHubValueResponse = response.json().await?;
        
        if value_response.credential_type != "certificate" {
            return Ok(None);
        }
        
        let cert_pem = value_response.value.get("certificate")
            .and_then(|v| v.as_str());
        
        if let Some(pem_data) = cert_pem {
            let (_, pem) = parse_x509_pem(pem_data.as_bytes())
                .map_err(|e| crate::DoomsdayError::x509(format!("Failed to parse PEM: {}", e)))?;
            
            let (_, cert) = parse_x509_certificate(&pem.contents)
                .map_err(|e| crate::DoomsdayError::x509(format!("Failed to parse certificate: {}", e)))?;
            
            let cert_data = CertificateData::from_x509(&cert, pem_data)?;
            Ok(Some(cert_data))
        } else {
            Ok(None)
        }
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}