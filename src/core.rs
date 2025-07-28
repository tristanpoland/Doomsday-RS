use crate::backends::create_accessor;
use crate::cache::{Cache, CacheDiff};
use crate::config::{BackendConfig, Config};
use crate::scheduler::Scheduler;
use crate::storage::Accessor;
use crate::types::{CacheObject, PathObject, PopulateStats, Task};
use chrono::Utc;
use sha1::{Sha1, Digest};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct Core {
    config: Arc<RwLock<Config>>,
    cache: Cache,
    accessors: Arc<RwLock<HashMap<String, Arc<dyn Accessor>>>>,
    scheduler: Scheduler,
}

impl Core {
    pub async fn new(config: Config) -> crate::Result<Self> {
        let cache = Cache::new();
        let scheduler = Scheduler::default();
        let mut accessors = HashMap::new();
        
        for backend_config in &config.backends {
            let accessor = create_accessor(backend_config)?;
            accessors.insert(backend_config.name.clone(), accessor);
        }
        
        let core = Core {
            config: Arc::new(RwLock::new(config)),
            cache,
            accessors: Arc::new(RwLock::new(accessors)),
            scheduler,
        };
        
        // Schedule initial refresh tasks
        core.schedule_refresh_tasks().await;
        
        Ok(core)
    }
    
    pub async fn populate_cache(&self) -> crate::Result<PopulateStats> {
        let start_time = Instant::now();
        let accessors = self.accessors.read().await;
        let mut all_paths = Vec::new();
        
        // Collect all paths from all backends
        for (backend_name, accessor) in accessors.iter() {
            match accessor.list().await {
                Ok(paths) => {
                    for path in paths {
                        all_paths.push((backend_name.clone(), path));
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to list paths from backend {}: {}", backend_name, e);
                }
            }
        }
        
        let num_paths = all_paths.len();
        let mut num_certs = 0;
        let mut new_cache_objects: HashMap<String, CacheObject> = HashMap::new();
        
        // Process paths in chunks for better performance
        let chunk_size = 100;
        for chunk in all_paths.chunks(chunk_size) {
            let mut tasks = Vec::new();
            
            for (backend_name, path) in chunk {
                let accessor = accessors.get(backend_name).unwrap().clone();
                let path = path.clone();
                let backend_name = backend_name.clone();
                
                let task = tokio::spawn(async move {
                    accessor.get(&path).await.map(|cert_data| (backend_name, path, cert_data))
                });
                
                tasks.push(task);
            }
            
            // Wait for all tasks in this chunk to complete
            for task in tasks {
                match task.await {
                    Ok(Ok((backend_name, path, Some(cert_data)))) => {
                        let sha1 = cert_data.fingerprint_sha1.clone();
                        
                        if let Some(existing) = new_cache_objects.get_mut(&sha1) {
                            // Certificate already exists, add this path
                            existing.paths.push(PathObject {
                                backend: backend_name,
                                path,
                            });
                        } else {
                            // New certificate
                            let cache_object = CacheObject {
                                subject: cert_data.subject,
                                not_after: cert_data.not_after,
                                sha1: sha1.clone(),
                                paths: vec![PathObject {
                                    backend: backend_name,
                                    path,
                                }],
                            };
                            
                            new_cache_objects.insert(sha1, cache_object);
                            num_certs += 1;
                        }
                    },
                    Ok(Ok((_, _, None))) => {
                        // No certificate data at this path
                    },
                    Ok(Err(e)) => {
                        tracing::error!("Failed to get certificate data: {}", e);
                    },
                    Err(e) => {
                        tracing::error!("Task failed: {}", e);
                    }
                }
            }
        }
        
        // Update cache with new data
        let diff = CacheDiff {
            added: new_cache_objects,
            removed: Vec::new(), // TODO: Implement proper diffing to remove stale entries
        };
        
        self.cache.update_from_diff(diff)?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(PopulateStats {
            num_certs,
            num_paths,
            duration_ms,
        })
    }
    
    pub async fn refresh_backend(&self, backend_name: &str) -> crate::Result<PopulateStats> {
        let start_time = Instant::now();
        let accessors = self.accessors.read().await;
        
        let accessor = accessors.get(backend_name)
            .ok_or_else(|| crate::DoomsdayError::not_found(format!("Backend {} not found", backend_name)))?;
        
        let paths = accessor.list().await?;
        let num_paths = paths.len();
        let mut num_certs = 0;
        let mut backend_cache_objects: HashMap<String, CacheObject> = HashMap::new();
        
        // Process paths in chunks
        let chunk_size = 50;
        for chunk in paths.chunks(chunk_size) {
            let mut tasks = Vec::new();
            
            for path in chunk {
                let accessor = accessor.clone();
                let path = path.clone();
                
                let task = tokio::spawn(async move {
                    accessor.get(&path).await.map(|cert_data| (path, cert_data))
                });
                
                tasks.push(task);
            }
            
            for task in tasks {
                match task.await {
                    Ok(Ok((path, Some(cert_data)))) => {
                        let sha1 = cert_data.fingerprint_sha1.clone();
                        
                        if let Some(existing) = backend_cache_objects.get_mut(&sha1) {
                            existing.paths.push(PathObject {
                                backend: backend_name.to_string(),
                                path,
                            });
                        } else {
                            let cache_object = CacheObject {
                                subject: cert_data.subject,
                                not_after: cert_data.not_after,
                                sha1: sha1.clone(),
                                paths: vec![PathObject {
                                    backend: backend_name.to_string(),
                                    path,
                                }],
                            };
                            
                            backend_cache_objects.insert(sha1, cache_object);
                            num_certs += 1;
                        }
                    },
                    Ok(Ok((_, None))) => {},
                    Ok(Err(e)) => {
                        tracing::error!("Failed to get certificate from {}: {}", backend_name, e);
                    },
                    Err(e) => {
                        tracing::error!("Task failed: {}", e);
                    }
                }
            }
        }
        
        // Remove old entries for this backend from cache
        let all_cache_items = self.cache.list();
        let mut to_remove = Vec::new();
        
        for item in all_cache_items {
            if item.paths.iter().any(|p| p.backend == backend_name) {
                // This certificate has paths from the backend we're refreshing
                // We need to check if it still exists in our new data
                let sha1 = Sha1::digest(item.subject.as_bytes());
                let sha1_hex = hex::encode(sha1);
                
                if !backend_cache_objects.contains_key(&sha1_hex) {
                    to_remove.push(sha1_hex);
                }
            }
        }
        
        let diff = CacheDiff {
            added: backend_cache_objects,
            removed: to_remove,
        };
        
        self.cache.update_from_diff(diff)?;
        
        let duration_ms = start_time.elapsed().as_millis() as u64;
        
        Ok(PopulateStats {
            num_certs,
            num_paths,
            duration_ms,
        })
    }
    
    pub fn get_cache(&self) -> &Cache {
        &self.cache
    }
    
    pub fn get_scheduler(&self) -> &Scheduler {
        &self.scheduler
    }
    
    pub async fn schedule_refresh_tasks(&self) {
        let config = self.config.read().await;
        
        for backend_config in &config.backends {
            let task = Task::RefreshBackend {
                backend_name: backend_config.name.clone(),
            };
            
            if let Err(e) = self.scheduler.schedule_task(task) {
                tracing::error!("Failed to schedule refresh task for {}: {}", backend_config.name, e);
            }
        }
    }
    
    pub async fn schedule_periodic_tasks(&self) {
        let config = self.config.read().await;
        
        for backend_config in &config.backends {
            if let Some(refresh_interval) = backend_config.refresh_interval {
                let backend_name = backend_config.name.clone();
                let scheduler = self.scheduler.clone();
                
                tokio::spawn(async move {
                    let mut interval = tokio::time::interval(
                        tokio::time::Duration::from_secs(refresh_interval * 60)
                    );
                    
                    loop {
                        interval.tick().await;
                        
                        let task = Task::RefreshBackend {
                            backend_name: backend_name.clone(),
                        };
                        
                        if let Err(e) = scheduler.schedule_task(task) {
                            tracing::error!("Failed to schedule periodic refresh for {}: {}", backend_name, e);
                        }
                    }
                });
            }
        }
    }
    
    pub async fn get_config(&self) -> Config {
        self.config.read().await.clone()
    }
    
    pub async fn update_config(&self, new_config: Config) -> crate::Result<()> {
        new_config.validate()?;
        
        // Update accessors based on new config
        let mut new_accessors = HashMap::new();
        for backend_config in &new_config.backends {
            let accessor = create_accessor(backend_config)?;
            new_accessors.insert(backend_config.name.clone(), accessor);
        }
        
        {
            let mut config = self.config.write().await;
            let mut accessors = self.accessors.write().await;
            
            *config = new_config;
            *accessors = new_accessors;
        }
        
        // Reschedule tasks with new configuration
        self.schedule_refresh_tasks().await;
        self.schedule_periodic_tasks().await;
        
        Ok(())
    }
}