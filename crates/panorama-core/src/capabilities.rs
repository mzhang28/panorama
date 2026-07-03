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
    /// Check if the app can read a specific field
    pub fn can_read_field(&self, field_key: &str) -> bool {
        self.field_read.iter().any(|f| f == field_key || f == "*")
    }

    /// Check if the app can write a specific field
    pub fn can_write_field(&self, field_key: &str) -> bool {
        self.field_write.iter().any(|f| f == field_key || f == "*")
    }

    /// Check if the app can contact a specific network host
    pub fn can_contact_host(&self, host: &str) -> bool {
        self.network_hosts
            .iter()
            .any(|h| h == host || h == "*")
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
