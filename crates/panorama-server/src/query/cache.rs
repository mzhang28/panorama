//! LRU cache for compiled SQL statements.
//!
//! Keyed on the IR plan shape hash (QUERY_DESIGN.md §7.3: "the IR shape (query
//! structure minus parameter values)" rather than the raw source string.
//! Two queries that differ only in a literal value (`WHERE n.foo = "bar"` vs
//! `WHERE n.foo = "baz"`) share the same cache entry.
//!
//! Invalidation triggers (§7.3):
//! - Schema migration bumps `physical_table_name` or `field_mappings`
//! - `managed_indexes` status change
//! - Field promotion (JSONB → column)

use std::collections::HashMap;
use std::sync::Mutex;

use crate::query::compiler::ParamValue;

/// Thread-safe query cache with LRU eviction (max 256 entries).
pub struct StatementCache {
  /// Cached compiled SQL keyed by query hash.
  entries: Mutex<HashMap<u64, CachedEntry>>,
  /// LRU tracking — most recent at front.
  lru: Mutex<Vec<u64>>,
  max_entries: usize,
}

/// A cached compiled query.
#[derive(Clone)]
pub struct CachedEntry {
  pub sql: String,
  pub params: Vec<ParamValue>,
}

impl StatementCache {
  pub fn new() -> Self {
    Self {
      entries: Mutex::new(HashMap::new()),
      lru: Mutex::new(Vec::new()),
      max_entries: 256,
    }
  }

  /// Look up a compiled query by IR shape hash.
  pub fn get(&self, cache_key: u64) -> Option<CachedEntry> {
    let entries = self.entries.lock().unwrap();
    let result = entries.get(&cache_key).cloned();
    if result.is_some() {
      self.touch(cache_key);
    }
    result
  }

  /// Store a compiled query keyed by IR shape hash.
  pub fn insert(&self, cache_key: u64, entry: CachedEntry) {
    let mut entries = self.entries.lock().unwrap();
    entries.insert(cache_key, entry);
    self.touch(cache_key);
  }

  /// Record a cache hit (promote to front of LRU).
  fn touch(&self, key: u64) {
    let mut lru = self.lru.lock().unwrap();
    lru.retain(|k| *k != key);
    lru.insert(0, key);
    // Evict if over max
    if lru.len() > self.max_entries {
      if let Some(evicted_key) = lru.pop() {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(&evicted_key);
      }
    }
  }

  /// Clear all cached entries (e.g. after schema migration or index change).
  pub fn clear(&self) {
    self.entries.lock().unwrap().clear();
    self.lru.lock().unwrap().clear();
  }

  pub fn len(&self) -> usize {
    self.lru.lock().unwrap().len()
  }
}

impl Default for StatementCache {
  fn default() -> Self {
    Self::new()
  }
}
