use crate::types::{CertificateData, PathList};
use async_trait::async_trait;

#[async_trait]
pub trait Accessor: Send + Sync {
    async fn list(&self) -> crate::Result<PathList>;
    async fn get(&self, path: &str) -> crate::Result<Option<CertificateData>>;
    fn name(&self) -> &str;
}

pub mod credhub;
pub mod opsmgr;
pub mod tlsclient;
pub mod vault;
