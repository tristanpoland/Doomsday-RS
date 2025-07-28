use crate::storage::Accessor;
use crate::types::{CertificateData, PathList};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use x509_parser::prelude::*;

#[derive(Debug, Clone)]
pub struct VaultAccessor {
    name: String,
    client: Client,
    base_url: Url,
    token: String,
    mount_path: String,
    secret_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultListResponse {
    data: VaultListData,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultListData {
    keys: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct VaultSecretResponse {
    data: HashMap<String, serde_json::Value>,
}

impl VaultAccessor {
    pub fn new(
        name: String,
        base_url: Url,
        token: String,
        mount_path: String,
        secret_path: String,
    ) -> crate::Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(false)
            .build()?;
        
        Ok(VaultAccessor {
            name,
            client,
            base_url,
            token,
            mount_path,
            secret_path,
        })
    }
    
    pub fn from_config(name: String, properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let url = properties.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Vault URL is required"))?;
        
        let token = properties.get("token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Vault token is required"))?;
        
        let mount_path = properties.get("mount_path")
            .and_then(|v| v.as_str())
            .unwrap_or("secret");
        
        let secret_path = properties.get("secret_path")
            .and_then(|v| v.as_str())
            .unwrap_or("/");
        
        let base_url = Url::parse(url)
            .map_err(|e| crate::DoomsdayError::config(format!("Invalid Vault URL: {}", e)))?;
        
        Self::new(
            name,
            base_url,
            token.to_string(),
            mount_path.to_string(),
            secret_path.to_string(),
        )
    }
    
    async fn list_recursive(&self, path: &str) -> crate::Result<Vec<String>> {
        let mut all_paths = Vec::new();
        let mut to_process = vec![path.to_string()];
        
        while let Some(current_path) = to_process.pop() {
            let url = format!(
                "{}/v1/{}/metadata/{}",
                self.base_url.as_str().trim_end_matches('/'),
                self.mount_path,
                current_path.trim_start_matches('/')
            );
            
            let response = self.client
                .get(&url)
                .header("X-Vault-Token", &self.token)
                .query(&[("list", "true")])
                .send()
                .await?;
            
            if response.status().is_success() {
                let vault_response: VaultListResponse = response.json().await?;
                
                for key in vault_response.data.keys {
                    let full_path = if current_path.is_empty() || current_path == "/" {
                        key.clone()
                    } else {
                        format!("{}/{}", current_path.trim_end_matches('/'), key)
                    };
                    
                    if key.ends_with('/') {
                        // It's a directory, add to processing queue
                        to_process.push(full_path.trim_end_matches('/').to_string());
                    } else {
                        // It's a secret
                        all_paths.push(full_path);
                    }
                }
            }
        }
        
        Ok(all_paths)
    }
}

#[async_trait]
impl Accessor for VaultAccessor {
    async fn list(&self) -> crate::Result<PathList> {
        self.list_recursive(&self.secret_path).await
    }
    
    async fn get(&self, path: &str) -> crate::Result<Option<CertificateData>> {
        let url = format!(
            "{}/v1/{}/data/{}",
            self.base_url.as_str().trim_end_matches('/'),
            self.mount_path,
            path.trim_start_matches('/')
        );
        
        let response = self.client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Ok(None);
        }
        
        let vault_response: VaultSecretResponse = response.json().await?;
        
        // Look for certificate data in common fields
        let cert_pem = vault_response.data.get("certificate")
            .or_else(|| vault_response.data.get("cert"))
            .or_else(|| vault_response.data.get("crt"))
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