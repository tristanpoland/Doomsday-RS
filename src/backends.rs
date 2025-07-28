use crate::config::BackendConfig;
use crate::storage::{Accessor, credhub::CredHubAccessor, opsmgr::OpsMgrAccessor, tlsclient::TlsClientAccessor, vault::VaultAccessor};
use std::sync::Arc;

pub fn create_accessor(config: &BackendConfig) -> crate::Result<Arc<dyn Accessor>> {
    tracing::info!("Creating accessor for backend '{}' of type '{}'"  , config.name, config.backend_type);
    
    match config.backend_type.as_str() {
        "vault" => {
            tracing::debug!("Initializing Vault accessor for backend: {}", config.name);
            let accessor = VaultAccessor::from_config(config.name.clone(), &config.properties)?;
            tracing::info!("Vault accessor created successfully for backend: {}", config.name);
            Ok(Arc::new(accessor))
        },
        "credhub" => {
            tracing::debug!("Initializing CredHub accessor for backend: {}", config.name);
            let accessor = CredHubAccessor::from_config(config.name.clone(), &config.properties)?;
            tracing::info!("CredHub accessor created successfully for backend: {}", config.name);
            Ok(Arc::new(accessor))
        },
        "opsmgr" => {
            tracing::debug!("Initializing Ops Manager accessor for backend: {}", config.name);
            let accessor = OpsMgrAccessor::from_config(config.name.clone(), &config.properties)?;
            tracing::info!("Ops Manager accessor created successfully for backend: {}", config.name);
            Ok(Arc::new(accessor))
        },
        "tlsclient" => {
            tracing::debug!("Initializing TLS Client accessor for backend: {}", config.name);
            let accessor = TlsClientAccessor::from_config(config.name.clone(), &config.properties)?;
            tracing::info!("TLS Client accessor created successfully for backend: {}", config.name);
            Ok(Arc::new(accessor))
        },
        _ => {
            tracing::error!("Unknown backend type '{}' for backend '{}'", config.backend_type, config.name);
            Err(crate::DoomsdayError::config(
                format!("Unknown backend type: {}", config.backend_type)
            ))
        },
    }
}