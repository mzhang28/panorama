//! LRU cache for compiled SQL statements including Phase-1 compile context.
//!
//! Keyed on the IR plan shape hash (QUERY_DESIGN.md §7.3: "the IR shape (query
//! structure minus parameter values)" rather than the raw source string.
//! Two queries that differ only in a literal value (`WHERE n.foo = "bar"` vs
//! `WHERE n.foo = "baz"`) share the same cache entry.
//!
//! Stores both the SQL template AND the `CompileCtx` so cache hits can call
//! `compile_phase2` directly, skipping repeated meta-table lookups (§1.4).
//!
//! Invalidation triggers (§7.3):
//! - Schema migration bumps `physical_table_name` or `field_mappings`
//! - `managed_indexes` status change
//! - Field promotion (JSONB → column)

use std::collections::HashMap;
use std::sync::Mutex;

use crate::query::compiler::{CompileCtx, ParamValue};

/// Thread-safe query cache with LRU eviction (max 256 entries).
pub struct StatementCache {
  entries: Mutex<HashMap<u64, CachedEntry>>,
  lru: Mutex<Vec<u64>>,
  max_entries: usize,
}

/// A cached compiled query — SQL template plus the Phase-1 compile context
/// so cache hits can skip meta-table lookups entirely (§1.4).
#[derive(Clone)]
pub struct CachedEntry {
  pub sql: String,
  pub params: Vec<ParamValue>,
  /// Cached Phase-1 context: resolved schemas, namespace IDs, CTE names.
  /// When present, `compile_phase2` can be called directly.
  pub compile_ctx: Option<CompileCtx>,
}

impl StatementCache {
  pub fn new() -> Self {
    Self {
      entries: Mutex::new(HashMap::new()),
      lru: Mutex::new(Vec::new()),
      max_entries: 256,
    }
  }

  pub fn get(&self, cache_key: u64) -> Option<CachedEntry> {
    let entries = self.entries.lock().unwrap();
    let result = entries.get(&cache_key).cloned();
    if result.is_some() {
      self.touch(cache_key);
    }
    result
  }

  pub fn insert(&self, cache_key: u64, entry: CachedEntry) {
    let mut entries = self.entries.lock().unwrap();
    entries.insert(cache_key, entry);
    self.touch(cache_key);
  }

  fn touch(&self, key: u64) {
    let mut lru = self.lru.lock().unwrap();
    lru.retain(|k| *k != key);
    lru.insert(0, key);
    if lru.len() > self.max_entries {
      if let Some(evicted_key) = lru.pop() {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(&evicted_key);
      }
    }
  }

  /// Look up a cached SQL template plus Phase-1 context by IR shape key.
  pub fn get_sql(&self, cache_key: u64) -> Option<(String, Option<CompileCtx>)> {
    let entries = self.entries.lock().unwrap();
    let result = entries
      .get(&cache_key)
      .map(|e| (e.sql.clone(), e.compile_ctx.clone()));
    if result.is_some() {
      self.touch(cache_key);
    }
    result
  }

  /// Store a SQL template and Phase-1 compile context keyed by IR shape hash.
  pub fn insert_sql(&self, cache_key: u64, sql: String, compile_ctx: CompileCtx) {
    let mut entries = self.entries.lock().unwrap();
    entries.insert(
      cache_key,
      CachedEntry {
        sql,
        params: Vec::new(),
        compile_ctx: Some(compile_ctx),
      },
    );
    self.touch(cache_key);
  }

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
