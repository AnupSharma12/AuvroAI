use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub expired: u64,
}

#[derive(Clone, Debug)]
pub struct ResponseCacheConfig {
    pub ttl: Duration,
    pub max_bytes: usize,
}

impl Default for ResponseCacheConfig {
    fn default() -> Self {
        let ttl_secs = std::env::var("AUVRO_CACHE_TTL_SECS")
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
            .unwrap_or(600);
        let max_mb = std::env::var("AUVRO_CACHE_MAX_MB")
            .ok()
            .and_then(|value| value.trim().parse::<usize>().ok())
            .unwrap_or(50);

        Self {
            ttl: Duration::from_secs(ttl_secs),
            max_bytes: max_mb.saturating_mul(1_048_576),
        }
    }
}

#[derive(Clone, Debug)]
struct CacheEntry {
    response: String,
    inserted_at: Instant,
    size_bytes: usize,
}

#[derive(Clone, Debug)]
pub struct ResponseCache {
    config: ResponseCacheConfig,
    entries: HashMap<String, CacheEntry>,
    lru: VecDeque<String>,
    used_bytes: usize,
    stats: CacheStats,
}

impl ResponseCache {
    pub fn new(config: ResponseCacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            lru: VecDeque::new(),
            used_bytes: 0,
            stats: CacheStats::default(),
        }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        let now = Instant::now();

        if let Some(entry) = self.entries.get(key) {
            if now.duration_since(entry.inserted_at) > self.config.ttl {
                self.remove_key(key);
                self.stats.misses = self.stats.misses.saturating_add(1);
                self.stats.expired = self.stats.expired.saturating_add(1);
                return None;
            }
        }

        if let Some(response) = self.entries.get(key).map(|entry| entry.response.clone()) {
            self.touch_key(key);
            self.stats.hits = self.stats.hits.saturating_add(1);
            return Some(response);
        }

        self.stats.misses = self.stats.misses.saturating_add(1);
        None
    }

    pub fn put(&mut self, key: String, response: String) {
        let new_size = key.len().saturating_add(response.len());

        if let Some(existing) = self.entries.get(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(existing.size_bytes);
            self.remove_lru_key(&key);
        }

        let entry = CacheEntry {
            response,
            inserted_at: Instant::now(),
            size_bytes: new_size,
        };

        self.used_bytes = self.used_bytes.saturating_add(entry.size_bytes);
        self.entries.insert(key.clone(), entry);
        self.lru.push_back(key);

        self.evict_if_needed();
    }

    #[allow(dead_code)]
    pub fn stats(&self) -> CacheStats {
        self.stats.clone()
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.entries.clear();
        self.lru.clear();
        self.used_bytes = 0;
        self.stats = CacheStats::default();
    }

    fn evict_if_needed(&mut self) {
        while self.used_bytes > self.config.max_bytes {
            let Some(oldest_key) = self.lru.pop_front() else {
                break;
            };

            if let Some(entry) = self.entries.remove(&oldest_key) {
                self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
                self.stats.evictions = self.stats.evictions.saturating_add(1);
            }
        }
    }

    fn remove_key(&mut self, key: &str) {
        if let Some(entry) = self.entries.remove(key) {
            self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
        }
        self.remove_lru_key(key);
    }

    fn remove_lru_key(&mut self, key: &str) {
        if let Some(index) = self.lru.iter().position(|k| k == key) {
            let _ = self.lru.remove(index);
        }
    }

    fn touch_key(&mut self, key: &str) {
        self.remove_lru_key(key);
        self.lru.push_back(key.to_owned());
    }
}

static RESPONSE_CACHE: OnceLock<Mutex<ResponseCache>> = OnceLock::new();

pub fn make_cache_key(prompt: &str, model: &str, system_prompt: &str) -> String {
    let mut raw = String::with_capacity(prompt.len() + model.len() + system_prompt.len());
    raw.push_str(prompt);
    raw.push_str(model);
    raw.push_str(system_prompt);
    blake3::hash(raw.as_bytes()).to_hex().to_string()
}

pub fn get_cached_response(key: &str) -> Option<String> {
    let mut cache = response_cache().lock().expect("Response cache mutex poisoned");
    cache.get(key)
}

pub fn insert_cached_response(key: String, response: String) {
    let mut cache = response_cache().lock().expect("Response cache mutex poisoned");
    cache.put(key, response);
}

#[allow(dead_code)]
pub fn response_cache_stats() -> CacheStats {
    let cache = response_cache().lock().expect("Response cache mutex poisoned");
    cache.stats()
}

#[allow(dead_code)]
pub fn reset_response_cache() {
    let mut cache = response_cache().lock().expect("Response cache mutex poisoned");
    cache.clear();
}

fn response_cache() -> &'static Mutex<ResponseCache> {
    RESPONSE_CACHE.get_or_init(|| Mutex::new(ResponseCache::new(ResponseCacheConfig::default())))
}
