use serde::{Deserialize, Serialize};

/// Capability grants for plugins.
/// Apps must declare and be granted capabilities to perform actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityGrants {
    /// Version of the capability declaration (major bump = re-prompt user)
    pub version: u32,
    /// Human-readable reason for capability changes
    pub reason: Option<String>,
    /// Network hosts the app can contact (one grant per host)
    pub network_hosts: Vec<String>,
    /// Specific fields the app can read (namespaced field keys)
    pub field_read: Vec<String>,
    /// Specific fields the app can write
    pub field_write: Vec<String>,
    /// Whether the app can write to nodes it created
    pub write_own_nodes: bool,
    /// Whether the app can have app-managed nodes (user can't modify directly)
    pub app_managed_nodes: bool,
    /// Whether the app can read files from the filesystem
    pub file_read: bool,
    /// Whether the app can write files to the filesystem
    pub file_write: bool,
    /// Whether the app can execute external programs
    pub execute: bool,
    /// Whether the app can make DNS requests (per-host grants in network_hosts)
    pub dns_requests: bool,
    /// Object storage access
    pub object_storage_read: bool,
    pub object_storage_write: bool,
}

impl Default for CapabilityGrants {
    fn default() -> Self {
        Self {
            version: 1,
            reason: None,
            network_hosts: Vec::new(),
            field_read: Vec::new(),
            field_write: Vec::new(),
            write_own_nodes: true,
            app_managed_nodes: false,
            file_read: false,
            file_write: false,
            execute: false,
            dns_requests: false,
            object_storage_read: false,
            object_storage_write: false,
        }
    }
}

impl CapabilityGrants {
    /// Check if the app can read a specific field.
    /// Supports glob-style patterns: `"journal:*"` matches `"journal:content"`,
    /// `"*"` matches everything.
    pub fn can_read_field(&self, field_key: &str) -> bool {
        self.field_read.iter().any(|f| pattern_match(f, field_key))
    }

    /// Check if the app can write a specific field.
    /// Supports glob-style patterns: `"journal:*"` matches `"journal:content"`,
    /// `"*"` matches everything.
    pub fn can_write_field(&self, field_key: &str) -> bool {
        self.field_write.iter().any(|f| pattern_match(f, field_key))
    }

    /// Check if the app can contact a specific network host.
    /// Supports glob-style patterns: `"*.example.com"`, `"*"`.
    pub fn can_contact_host(&self, host: &str) -> bool {
        self.network_hosts
            .iter()
            .any(|h| pattern_match(h, host))
    }

    /// Bump major version (required for permission changes)
    pub fn bump_major(&mut self) {
        self.version += 1;
    }
}

/// A capability request from a plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub requested: CapabilityGrants,
    /// The previously granted capabilities (for diffing)
    pub previous: Option<CapabilityGrants>,
}

// ── Pattern matching for capability wildcards ───────────────────────────────

/// Match a field key against a capability pattern.
///
/// Supported patterns:
/// - `"*"` — matches everything
/// - `"journal:*"` — matches any field in the "journal" namespace
/// - `"journal:content"` — exact match only
fn pattern_match(pattern: &str, field_key: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_suffix(":*") {
        // Namespace wildcard: "journal:*" matches "journal:content", "journal:mood", etc.
        return field_key.starts_with(suffix)
            && field_key[suffix.len()..].starts_with(':');
    }
    if let Some(prefix) = pattern.strip_prefix("*.") {
        // Domain suffix: "*.example.com" matches "sub.example.com"
        return field_key.ends_with(prefix)
            && field_key[..field_key.len() - prefix.len()].ends_with('.');
    }
    // Exact match
    pattern == field_key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_match_exact() {
        assert!(pattern_match("journal:content", "journal:content"));
        assert!(!pattern_match("journal:content", "journal:mood"));
    }

    #[test]
    fn test_pattern_match_star() {
        assert!(pattern_match("*", "anything"));
        assert!(pattern_match("*", "journal:content"));
    }

    #[test]
    fn test_pattern_match_namespace_wildcard() {
        assert!(pattern_match("journal:*", "journal:content"));
        assert!(pattern_match("journal:*", "journal:mood"));
        assert!(pattern_match("journal:*", "journal:paragraph_refs"));
        assert!(!pattern_match("journal:*", "other:field"));
        assert!(!pattern_match("journal:*", "journal"));  // no colon, not a field
    }

    #[test]
    fn test_pattern_match_domain_suffix() {
        assert!(pattern_match("*.example.com", "sub.example.com"));
        assert!(pattern_match("*.example.com", "api.example.com"));
        assert!(!pattern_match("*.example.com", "example.com"));
        assert!(!pattern_match("*.example.com", "other.net"));
    }

    #[test]
    fn test_capability_grants_namespace_wildcard() {
        let caps = CapabilityGrants {
            field_read: vec!["journal:*".to_string(), "system:node_title".to_string()],
            field_write: vec!["journal:*".to_string()],
            ..Default::default()
        };
        assert!(caps.can_read_field("journal:content"));
        assert!(caps.can_read_field("journal:mood"));
        assert!(caps.can_read_field("system:node_title"));
        assert!(!caps.can_read_field("other:field"));

        assert!(caps.can_write_field("journal:content"));
        assert!(caps.can_write_field("journal:paragraph_refs"));
        assert!(!caps.can_write_field("system:node_title"));
    }
}
