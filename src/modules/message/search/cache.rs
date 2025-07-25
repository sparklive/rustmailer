use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

pub static IMAP_SEARCH_CACHE: LazyLock<ImapSearchCache> =
    LazyLock::new(|| ImapSearchCache::new(100, Duration::from_secs(120)));

#[derive(Clone)]
struct CacheEntry {
    data: Arc<Vec<String>>,
    total: u64,
    created_at: Instant,
}

pub struct ImapSearchCache {
    store: Arc<RwLock<LruCache<String, CacheEntry>>>,
    ttl: Duration,
}

impl ImapSearchCache {
    fn new(capacity: usize, ttl: Duration) -> Self {
        let store = Arc::new(RwLock::new(LruCache::new(
            NonZeroUsize::new(capacity).unwrap(),
        )));
        ImapSearchCache { store, ttl }
    }

    pub async fn get(&self, key: &str) -> Option<(Arc<Vec<String>>, u64)> {
        let store = self.store.read().await;
        if let Some(entry) = store.peek(key) {
            if Instant::now().duration_since(entry.created_at) <= self.ttl {
                return Some((entry.data.clone(), entry.total));
            }
        }
        None
    }

    pub async fn set(&self, key: String, data: Arc<Vec<String>>, total: u64) {
        let mut store = self.store.write().await;
        store.put(
            key,
            CacheEntry {
                data,
                total,
                created_at: Instant::now(),
            },
        );
    }
}
