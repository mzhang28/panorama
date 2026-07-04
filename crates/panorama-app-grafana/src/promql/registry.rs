//! Metric registry — maps Prometheus metric names to Panorama namespace/field mappings.
//!
//! In Prometheus, a metric is identified by its name (e.g., `http_requests_total`)
//! and a set of key-value labels. In Panorama, data is stored as nodes with
//! namespaced fields. The metric registry maps between these two worlds.

use std::collections::HashMap;

/// A single metric mapping: Prometheus metric name → Panorama fields.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricMapping {
    /// The Prometheus metric name (e.g., "wakatime_duration").
    pub metric_name: String,
    /// The Panorama namespace (e.g., "wakatime").
    pub namespace: String,
    /// The field containing the numeric value (e.g., "duration").
    pub value_field: String,
    /// The field containing the timestamp (e.g., "time").
    /// Defaults to "system:node_time" if not set.
    #[serde(default = "default_time_field")]
    pub time_field: String,
    /// Fields that must be present on the node for it to be considered
    /// part of this metric (e.g., ["wakatime:entity"]).
    #[serde(default)]
    pub required_fields: Vec<String>,
    /// Default labels to add to every time series from this metric.
    #[serde(default)]
    pub default_labels: HashMap<String, String>,
    /// Description for the UI.
    #[serde(default)]
    pub description: String,
}

fn default_time_field() -> String {
    "system:node_time".to_string()
}

impl MetricMapping {
    /// Returns the fully-qualified Panorama field key for the value.
    pub fn value_field_key(&self) -> String {
        format!("{}:{}", self.namespace, self.value_field)
    }

    /// Returns the fully-qualified Panorama field key for the timestamp.
    pub fn time_field_key(&self) -> String {
        if self.time_field.contains(':') {
            self.time_field.clone()
        } else {
            format!("{}:{}", self.namespace, self.time_field)
        }
    }

    /// Returns the fully-qualified Panorama field key for a label.
    pub fn label_field_key(&self, label: &str) -> String {
        if label.contains(':') {
            label.to_string()
        } else {
            format!("{}:{}", self.namespace, label)
        }
    }
}

/// A registry of metric mappings, keyed by metric name.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MetricRegistry {
    /// Mappings from metric name to their field mapping.
    pub mappings: HashMap<String, MetricMapping>,
}

impl MetricRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self { mappings: HashMap::new() }
    }

    /// Look up a metric mapping by name.
    pub fn get(&self, metric_name: &str) -> Option<&MetricMapping> {
        self.mappings.get(metric_name)
    }

    /// Register a metric mapping.
    pub fn register(&mut self, mapping: MetricMapping) {
        self.mappings.insert(mapping.metric_name.clone(), mapping);
    }

    /// Remove a metric mapping.
    pub fn unregister(&mut self, metric_name: &str) -> Option<MetricMapping> {
        self.mappings.remove(metric_name)
    }

    /// List all registered metric names.
    pub fn metric_names(&self) -> Vec<&String> {
        self.mappings.keys().collect()
    }

    /// Try to auto-detect a metric mapping from known patterns.
    /// Returns the metric name if a mapping could be inferred.
    pub fn infer_metric_name(&self, namespace: &str, value_field: &str) -> Option<String> {
        for (name, mapping) in &self.mappings {
            if mapping.namespace == namespace && mapping.value_field == value_field {
                return Some(name.clone());
            }
        }
        // Fallback: use the naming convention `{namespace}_{value_field}`
        // Only if it's a known namespace with standard fields
        if matches!(namespace, "wakatime" | "beli") {
            return Some(format!("{}_{}", namespace, value_field));
        }
        None
    }

    /// Create a registry with default WakaTime metric mappings.
    pub fn with_wakatime_defaults() -> Self {
        let mut registry = Self::new();

        // WakaTime duration metric
        registry.register(MetricMapping {
            metric_name: "wakatime_duration".to_string(),
            namespace: "wakatime".to_string(),
            value_field: "duration".to_string(),
            // Use system:node_time as it's present on every node.
            // The DataPoint extractor handles both epoch floats and DateTime strings.
            time_field: "system:node_time".to_string(),
            required_fields: vec!["wakatime:entity".to_string()],
            default_labels: HashMap::from([
                ("source".to_string(), "wakatime".to_string()),
            ]),
            description: "Coding duration in seconds from WakaTime heartbeats".to_string(),
        });

        registry
    }
}
