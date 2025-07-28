use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::collections::HashMap;
use x509_parser::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheItem {
    pub subject: String,
    pub not_after: DateTime<Utc>,
    pub paths: Vec<PathObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheObject {
    pub subject: String,
    pub not_after: DateTime<Utc>,
    pub sha1: String,
    pub paths: Vec<PathObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathObject {
    pub backend: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulateStats {
    pub num_certs: usize,
    pub num_paths: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfoResponse {
    pub version: String,
    pub auth_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerInfo {
    pub workers: usize,
    pub pending_tasks: usize,
    pub running_tasks: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Task {
    RefreshBackend { backend_name: String },
    RenewAuthToken { backend_name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub task: Task,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: TaskStatus,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshRequest {
    pub backends: Option<Vec<String>>,
}

pub type PathList = Vec<String>;

#[derive(Debug, Clone)]
pub struct X509CertWrapper {
    pub path: String,
    pub cert: X509Certificate<'static>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateData {
    pub subject: String,
    pub not_before: DateTime<Utc>,
    pub not_after: DateTime<Utc>,
    pub serial_number: String,
    pub issuer: String,
    pub subject_alt_names: Vec<String>,
    pub key_usage: Vec<String>,
    pub ext_key_usage: Vec<String>,
    pub is_ca: bool,
    pub fingerprint_sha1: String,
    pub fingerprint_sha256: String,
    pub pem_data: String,
}

impl CertificateData {
    pub fn from_x509(cert: &X509Certificate, pem_data: &str) -> crate::Result<Self> {
        let subject = cert.subject().to_string();
        let issuer = cert.issuer().to_string();

        let not_before_dt = DateTime::from_timestamp(cert.validity().not_before.timestamp(), 0)
            .unwrap_or_else(|| Utc::now());

        let not_after_dt = DateTime::from_timestamp(cert.validity().not_after.timestamp(), 0)
            .unwrap_or_else(|| Utc::now());

        let serial = hex::encode(&cert.serial.to_bytes_be());

        // Compute fingerprints from DER data
        let der_data = cert.as_ref();
        let mut hasher_sha1 = Sha1::new();
        hasher_sha1.update(der_data);
        let fingerprint_sha1 = hex::encode(hasher_sha1.finalize());

        let mut hasher_sha256 = Sha256::new();
        hasher_sha256.update(der_data);
        let fingerprint_sha256 = hex::encode(hasher_sha256.finalize());

        let subject_alt_names = cert
            .extensions()
            .iter()
            .filter_map(|ext| {
                if let ParsedExtension::SubjectAlternativeName(san) = &ext.parsed_extension() {
                    Some(
                        san.general_names
                            .iter()
                            .filter_map(|name| match name {
                                GeneralName::DNSName(dns) => Some(dns.to_string()),
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    )
                } else {
                    None
                }
            })
            .flatten()
            .collect();

        let key_usage = vec![]; // TODO: Parse key usage extensions
        let ext_key_usage = vec![]; // TODO: Parse extended key usage
        let is_ca = cert.extensions().iter().any(
            |ext| matches!(ext.parsed_extension(), ParsedExtension::BasicConstraints(bc) if bc.ca),
        );

        Ok(CertificateData {
            subject,
            not_before: not_before_dt,
            not_after: not_after_dt,
            serial_number: serial,
            issuer,
            subject_alt_names,
            key_usage,
            ext_key_usage,
            is_ca,
            fingerprint_sha1,
            fingerprint_sha256,
            pem_data: pem_data.to_string(),
        })
    }
}
