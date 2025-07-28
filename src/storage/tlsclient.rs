use crate::storage::Accessor;
use crate::types::{CertificateData, PathList};
use async_trait::async_trait;
use base64::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::net::TcpStream;
use tokio_rustls::{rustls, TlsConnector};
use x509_parser::prelude::*;

#[derive(Debug, Clone)]
pub struct TlsClientAccessor {
    name: String,
    targets: Vec<TlsTarget>,
}

#[derive(Debug, Clone)]
struct TlsTarget {
    host: String,
    port: u16,
    server_name: Option<String>,
}

impl TlsClientAccessor {
    pub fn new(name: String, targets: Vec<TlsTarget>) -> Self {
        TlsClientAccessor { name, targets }
    }
    
    pub fn from_config(name: String, properties: &HashMap<String, serde_yaml::Value>) -> crate::Result<Self> {
        let targets_config = properties.get("targets")
            .and_then(|v| v.as_sequence())
            .ok_or_else(|| crate::DoomsdayError::config("TLS client targets are required"))?;
        
        let mut targets = Vec::new();
        
        for target_config in targets_config {
            let target_map = target_config.as_mapping()
                .ok_or_else(|| crate::DoomsdayError::config("Each target must be a mapping"))?;
            
            let host = target_map.get(&serde_yaml::Value::String("host".to_string()))
                .and_then(|v| v.as_str())
                .ok_or_else(|| crate::DoomsdayError::config("Target host is required"))?;
            
            let port = target_map.get(&serde_yaml::Value::String("port".to_string()))
                .and_then(|v| v.as_u64())
                .unwrap_or(443) as u16;
            
            let server_name = target_map.get(&serde_yaml::Value::String("server_name".to_string()))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            
            targets.push(TlsTarget {
                host: host.to_string(),
                port,
                server_name,
            });
        }
        
        Ok(Self::new(name, targets))
    }
    
    async fn get_certificate_from_target(&self, target: &TlsTarget) -> crate::Result<Option<CertificateData>> {
        let addr = format!("{}:{}", target.host, target.port);
        let socket_addr = SocketAddr::from_str(&addr)
            .or_else(|_| {
                // Try to resolve hostname
                std::net::ToSocketAddrs::to_socket_addrs(&addr)?
                    .next()
                    .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Could not resolve address"))
            })?;
        
        let stream = TcpStream::connect(socket_addr).await?;
        
        let mut root_store = rustls::RootCertStore::empty();
        root_store.add_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.iter().map(|ta| {
            rustls::OwnedTrustAnchor::from_subject_spki_name_constraints(
                ta.subject,
                ta.spki,
                ta.name_constraints,
            )
        }));
        
        let config = rustls::ClientConfig::builder()
            .with_safe_defaults()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        
        let connector = TlsConnector::from(std::sync::Arc::new(config));
        
        let server_name = target.server_name.as_deref().unwrap_or(&target.host);
        let domain = rustls::ServerName::try_from(server_name)
            .map_err(|e| crate::DoomsdayError::internal(format!("Invalid server name: {}", e)))?;
        
        let tls_stream = connector.connect(domain, stream).await?;
        
        let (_, session) = tls_stream.get_ref();
        let peer_certificates = session.peer_certificates()
            .ok_or_else(|| crate::DoomsdayError::internal("No peer certificates found"))?;
        
        if peer_certificates.is_empty() {
            return Ok(None);
        }
        
        // Use the first certificate in the chain (the server certificate)
        let cert_der = &peer_certificates[0];
        let (_, cert) = parse_x509_certificate(cert_der.as_ref())
            .map_err(|e| crate::DoomsdayError::x509(format!("Failed to parse certificate: {}", e)))?;
        
        // Convert DER to PEM for the certificate data
        let pem_data = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----",
            base64::prelude::BASE64_STANDARD.encode(cert_der.as_ref())
        );
        
        let path = format!("{}:{}", target.host, target.port);
        let cert_data = CertificateData::from_x509(&cert, &pem_data)?;
        
        Ok(Some(cert_data))
    }
}

#[async_trait]
impl Accessor for TlsClientAccessor {
    async fn list(&self) -> crate::Result<PathList> {
        let paths: Vec<String> = self.targets
            .iter()
            .map(|target| format!("{}:{}", target.host, target.port))
            .collect();
        
        Ok(paths)
    }
    
    async fn get(&self, path: &str) -> crate::Result<Option<CertificateData>> {
        let target = self.targets
            .iter()
            .find(|t| format!("{}:{}", t.host, t.port) == path);
        
        if let Some(target) = target {
            self.get_certificate_from_target(target).await
        } else {
            Ok(None)
        }
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}