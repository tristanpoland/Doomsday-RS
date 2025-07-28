use crate::storage::Accessor;
use crate::types::{CertificateData, PathList};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;
use x509_parser::prelude::*;

#[derive(Debug, Clone)]
pub struct OpsMgrAccessor {
    name: String,
    client: Client,
    base_url: Url,
    username: String,
    password: String,
    access_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrTokenRequest {
    grant_type: String,
    username: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrDeploymentsResponse {
    deployments: Vec<OpsMgrDeployment>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrDeployment {
    name: String,
    deployment_guid: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrCertificatesResponse {
    certificates: Vec<OpsMgrCertificate>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrCertificate {
    configurable: bool,
    property_reference: String,
    property_type: String,
    certificate: OpsMgrCertificateData,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpsMgrCertificateData {
    cert_pem: String,
    private_key_pem: String,
}

impl OpsMgrAccessor {
    pub fn new(
        name: String,
        base_url: Url,
        username: String,
        password: String,
    ) -> crate::Result<Self> {
        let client = Client::builder()
            .danger_accept_invalid_certs(true) // Ops Manager often uses self-signed certs
            .build()?;
        
        Ok(OpsMgrAccessor {
            name,
            client,
            base_url,
            username,
            password,
            access_token: None,
        })
    }
    
    pub fn from_config(name: String, properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let url = properties.get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Ops Manager URL is required"))?;
        
        let username = properties.get("username")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Ops Manager username is required"))?;
        
        let password = properties.get("password")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::DoomsdayError::config("Ops Manager password is required"))?;
        
        let base_url = Url::parse(url)
            .map_err(|e| crate::DoomsdayError::config(format!("Invalid Ops Manager URL: {}", e)))?;
        
        Self::new(
            name,
            base_url,
            username.to_string(),
            password.to_string(),
        )
    }
    
    async fn ensure_authenticated(&mut self) -> crate::Result<()> {
        if self.access_token.is_some() {
            return Ok(());
        }
        
        let token_url = format!(
            "{}/uaa/oauth/token",
            self.base_url.as_str().trim_end_matches('/')
        );
        
        let token_request = OpsMgrTokenRequest {
            grant_type: "password".to_string(),
            username: self.username.clone(),
            password: self.password.clone(),
        };
        
        let response = self.client
            .post(&token_url)
            .form(&token_request)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::auth("Failed to authenticate with Ops Manager"));
        }
        
        let token_response: OpsMgrTokenResponse = response.json().await?;
        self.access_token = Some(token_response.access_token);
        
        Ok(())
    }
    
    async fn get_auth_header(&mut self) -> crate::Result<String> {
        self.ensure_authenticated().await?;
        Ok(format!("Bearer {}", self.access_token.as_ref().unwrap()))
    }
    
    async fn get_deployments(&mut self) -> crate::Result<Vec<OpsMgrDeployment>> {
        let auth_header = self.get_auth_header().await?;
        
        let url = format!(
            "{}/api/v0/deployments",
            self.base_url.as_str().trim_end_matches('/')
        );
        
        let response = self.client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await?;
        
        if !response.status().is_success() {
            return Err(crate::DoomsdayError::backend("Failed to get deployments from Ops Manager"));
        }
        
        let deployments_response: OpsMgrDeploymentsResponse = response.json().await?;
        Ok(deployments_response.deployments)
    }
}

#[async_trait]
impl Accessor for OpsMgrAccessor {
    async fn list(&self) -> crate::Result<PathList> {
        let mut accessor = self.clone();
        let deployments = accessor.get_deployments().await?;
        
        let mut all_paths = Vec::new();
        let auth_header = accessor.get_auth_header().await?;
        
        for deployment in deployments {
            let url = format!(
                "{}/api/v0/deployments/{}/certificates",
                self.base_url.as_str().trim_end_matches('/'),
                deployment.deployment_guid
            );
            
            let response = self.client
                .get(&url)
                .header("Authorization", &auth_header)
                .send()
                .await?;
            
            if response.status().is_success() {
                let certs_response: OpsMgrCertificatesResponse = response.json().await?;
                
                for cert in certs_response.certificates {
                    let path = format!("{}/{}", deployment.name, cert.property_reference);
                    all_paths.push(path);
                }
            }
        }
        
        Ok(all_paths)
    }
    
    async fn get(&self, path: &str) -> crate::Result<Option<CertificateData>> {
        let mut accessor = self.clone();
        let deployments = accessor.get_deployments().await?;
        let auth_header = accessor.get_auth_header().await?;
        
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Ok(None);
        }
        
        let deployment_name = parts[0];
        let property_reference = parts[1];
        
        let deployment = deployments
            .iter()
            .find(|d| d.name == deployment_name);
        
        if let Some(deployment) = deployment {
            let url = format!(
                "{}/api/v0/deployments/{}/certificates",
                self.base_url.as_str().trim_end_matches('/'),
                deployment.deployment_guid
            );
            
            let response = self.client
                .get(&url)
                .header("Authorization", auth_header)
                .send()
                .await?;
            
            if response.status().is_success() {
                let certs_response: OpsMgrCertificatesResponse = response.json().await?;
                
                let cert = certs_response
                    .certificates
                    .iter()
                    .find(|c| c.property_reference == property_reference);
                
                if let Some(cert) = cert {
                    let (_, pem) = parse_x509_pem(cert.certificate.cert_pem.as_bytes())
                        .map_err(|e| crate::DoomsdayError::x509(format!("Failed to parse PEM: {}", e)))?;
                    
                    let (_, cert_obj) = parse_x509_certificate(&pem.contents)
                        .map_err(|e| crate::DoomsdayError::x509(format!("Failed to parse certificate: {}", e)))?;
                    
                    let cert_data = CertificateData::from_x509(&cert_obj, &cert.certificate.cert_pem)?;
                    return Ok(Some(cert_data));
                }
            }
        }
        
        Ok(None)
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}