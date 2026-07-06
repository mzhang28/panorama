//! LRU cache for tracking hot compiled SQL strings.
//!
//! SQLite already caches prepared statements internally (sqlite3_stmt cache).
//! This module tracks which SQL strings are frequently prepared so we can
//! skip the parser→IR→SQL compilation step for hot queries.
//!
//! Keyed on the AST query string hash. Invalidation: `clear()` after schema
//! migrations or index changes.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Mutex;

/// Thread-safe query cache with LRU eviction (max 256 entries).
pub struct StatementCache {
  /// Cached compiled queries keyed by source query hash.
  entries: Mutex<HashMap<u64, CachedEntry>>,
  /// LRU tracking — most recent at front.  Stores hash + use count.
  lru: Mutex<Vec<(u64, u64)>>,
  max_entries: usize,
}

/// A cached compiled query.
#[derive(Clone)]
pub struct CachedEntry {
  pub sql: String,
  pub param_count: usize,
}

impl StatementCache {
  pub fn new() -> Self {
    Self {
      entries: Mutex::new(HashMap::new()),
      lru: Mutex::new(Vec::new()),
      max_entries: 256,
    }
  }

  /// Look up a compiled query by source query text.
  pub fn get(&self, source_query: &str) -> Option<CachedEntry> {
    let hash = hash_str(source_query);
    let entries = self.entries.lock().unwrap();
    let result = entries.get(&hash).cloned();
    if result.is_some() {
      self.touch(hash);
    }
    result
  }

  /// Store a compiled query, keyed by source query text.
  pub fn insert(&self, source_query: &str, entry: CachedEntry) {
    let hash = hash_str(source_query);
    let mut entries = self.entries.lock().unwrap();
    entries.insert(hash, entry);
    self.touch(hash);
  }

  /// Record a hit on the given hash (promote to front of LRU).
  fn touch(&self, hash: u64) {
    let mut lru = self.lru.lock().unwrap();
    lru.retain(|(h, _)| *h != hash);
    lru.insert(0, (hash, 1));
    // Evict if over max
    if lru.len() > self.max_entries {
      if let Some((evicted_hash, _)) = lru.pop() {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(&evicted_hash);
      }
    }
  }

  /// Clear all cached entries.
  pub fn clear(&self) {
    self.entries.lock().unwrap().clear();
    self.lru.lock().unwrap().clear();
  }

  pub fn len(&self) -> usize {
    self.lru.lock().unwrap().len()
  }
}

fn hash_str(s: &str) -> u64 {
  let mut h = DefaultHasher::new();
  s.hash(&mut h);
  h.finish()
}
