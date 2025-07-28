use crate::types::{CacheItem, CacheObject, PathObject};
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Cache {
    inner: Arc<DashMap<String, CacheObject>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new()
    }
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            inner: Arc::new(DashMap::new()),
        }
    }
    
    pub fn get(&self, sha1: &str) -> Option<CacheObject> {
        self.inner.get(sha1).map(|entry| entry.clone())
    }
    
    pub fn insert(&self, sha1: String, object: CacheObject) {
        self.inner.insert(sha1, object);
    }
    
    pub fn remove(&self, sha1: &str) -> Option<CacheObject> {
        self.inner.remove(sha1).map(|(_, obj)| obj)
    }
    
    pub fn len(&self) -> usize {
        self.inner.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
    
    pub fn clear(&self) {
        self.inner.clear();
    }
    
    pub fn list(&self) -> Vec<CacheItem> {
        let mut items = Vec::new();
        
        for entry in self.inner.iter() {
            let obj = entry.value();
            items.push(CacheItem {
                subject: obj.subject.clone(),
                not_after: obj.not_after,
                paths: obj.paths.clone(),
            });
        }
        
        // Sort by expiry date
        items.sort_by(|a, b| a.not_after.cmp(&b.not_after));
        tracing::debug!("Listed {} certificates from cache", items.len());
        items
    }
    
    pub fn list_filtered<F>(&self, filter: F) -> Vec<CacheItem>
    where
        F: Fn(&CacheItem) -> bool,
    {
        self.list().into_iter().filter(filter).collect()
    }
    
    pub fn update_from_diff(&self, diff: CacheDiff) -> crate::Result<()> {
        tracing::debug!("Updating cache: {} items to add, {} to remove", 
            diff.added.len(), diff.removed.len());
        
        // Remove deleted items
        for sha1 in &diff.removed {
            if let Some(removed_obj) = self.remove(sha1) {
                tracing::debug!("Removed certificate from cache: {}", removed_obj.subject);
            }
        }
        
        // Add or update items
        for (sha1, object) in diff.added {
            tracing::debug!("Adding/updating certificate in cache: {} ({})", object.subject, sha1);
            self.insert(sha1, object);
        }
        
        tracing::debug!("Cache update completed, new size: {}", self.len());
        Ok(())
    }
    
    pub fn get_stats(&self) -> CacheStats {
        let now = Utc::now();
        let mut stats = CacheStats::default();
        
        for entry in self.inner.iter() {
            let obj = entry.value();
            stats.total += 1;
            
            let days_until_expiry = (obj.not_after - now).num_days();
            
            if days_until_expiry < 0 {
                stats.expired += 1;
            } else if days_until_expiry <= 30 {
                stats.expiring_soon += 1;
            } else {
                stats.ok += 1;
            }
        }
        
        stats
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheDiff {
    pub added: HashMap<String, CacheObject>,
    pub removed: Vec<String>,
}

impl CacheDiff {
    pub fn new() -> Self {
        CacheDiff {
            added: HashMap::new(),
            removed: Vec::new(),
        }
    }
    
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CacheStats {
    pub total: usize,
    pub ok: usize,
    pub expiring_soon: usize,
    pub expired: usize,
}

impl CacheStats {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    
    fn create_test_object(subject: &str, days_from_now: i64) -> CacheObject {
        CacheObject {
            subject: subject.to_string(),
            not_after: Utc::now() + Duration::days(days_from_now),
            sha1: format!("sha1_{}", subject),
            paths: vec![PathObject {
                backend: "test".to_string(),
                path: format!("/test/{}", subject),
            }],
        }
    }
    
    #[test]
    fn test_cache_operations() {
        let cache = Cache::new();
        
        let obj = create_test_object("test.com", 30);
        let sha1 = "test_sha1".to_string();
        
        cache.insert(sha1.clone(), obj.clone());
        
        assert_eq!(cache.len(), 1);
        assert!(!cache.is_empty());
        
        let retrieved = cache.get(&sha1).unwrap();
        assert_eq!(retrieved.subject, obj.subject);
        
        let removed = cache.remove(&sha1).unwrap();
        assert_eq!(removed.subject, obj.subject);
        assert_eq!(cache.len(), 0);
        assert!(cache.is_empty());
    }
    
    #[test]
    fn test_cache_stats() {
        let cache = Cache::new();
        
        // Add certificates with different expiry dates
        cache.insert("1".to_string(), create_test_object("expired.com", -10));
        cache.insert("2".to_string(), create_test_object("soon.com", 15));
        cache.insert("3".to_string(), create_test_object("ok.com", 100));
        
        let stats = cache.get_stats();
        assert_eq!(stats.total, 3);
        assert_eq!(stats.expired, 1);
        assert_eq!(stats.expiring_soon, 1);
        assert_eq!(stats.ok, 1);
    }
    
    #[test]
    fn test_cache_list_filtered() {
        let cache = Cache::new();
        
        cache.insert("1".to_string(), create_test_object("a.com", 30));
        cache.insert("2".to_string(), create_test_object("b.com", 60));
        cache.insert("3".to_string(), create_test_object("c.com", 90));
        
        let filtered = cache.list_filtered(|item| item.subject.starts_with("a"));
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].subject, "a.com");
    }
}