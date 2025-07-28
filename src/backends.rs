use crate::config::BackendConfig;
use crate::storage::{Accessor, credhub::CredHubAccessor, opsmgr::OpsMgrAccessor, tlsclient::TlsClientAccessor, vault::VaultAccessor};
use std::sync::Arc;

pub fn create_accessor(config: &BackendConfig) -> crate::Result<Arc<dyn Accessor>> {
    match config.backend_type.as_str() {
        "vault" => {
            let accessor = VaultAccessor::from_config(config.name.clone(), &config.properties)?;
            Ok(Arc::new(accessor))
        },
        "credhub" => {
            let accessor = CredHubAccessor::from_config(config.name.clone(), &config.properties)?;
            Ok(Arc::new(accessor))
        },
        "opsmgr" => {
            let accessor = OpsMgrAccessor::from_config(config.name.clone(), &config.properties)?;
            Ok(Arc::new(accessor))
        },
        "tlsclient" => {
            let accessor = TlsClientAccessor::from_config(config.name.clone(), &config.properties)?;
            Ok(Arc::new(accessor))
        },
        _ => Err(crate::DoomsdayError::config(
            format!("Unknown backend type: {}", config.backend_type)
        )),
    }
}