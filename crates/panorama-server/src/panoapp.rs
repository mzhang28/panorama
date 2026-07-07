//! .panoapp — single-file ZIP archive containing:
//!   manifest.json   — required: plugin metadata, schemas, endpoints, capabilities
//!   plugin.wasm     — optional: WASM module for backend handlers
//!   ui/*            — optional: frontend UI assets (JS, CSS, HTML)
//!
//! This is the standard third-party distribution format for Panorama plugins.
//! Plugins are built elsewhere and installed as .panoapp files.

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use panorama_core::capabilities::CapabilityGrants;
use panorama_core::plugin::{HttpEndpoint, UiComponent};
use panorama_core::schema::{Schema, SchemaField, SchemaMode};
use panorama_core::types::SchemaVersion;

/// The manifest inside a .panoapp archive
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanoAppManifest {
  pub manifest_version: u32,
  pub id: String,
  pub name: String,
  pub version: String,
  pub description: String,
  #[serde(default)]
  pub author: Option<String>,
  #[serde(default)]
  pub homepage: Option<String>,
  #[serde(default)]
  pub icon: Option<String>,
  #[serde(default)]
  pub min_platform_version: Option<String>,
  #[serde(default)]
  pub schemas: Vec<ManifestSchema>,
  #[serde(default)]
  pub http_endpoints: Vec<HttpEndpoint>,
  #[serde(default)]
  pub background_tasks: Vec<ManifestBackgroundTask>,
  #[serde(default)]
  pub ui_components: Vec<UiComponent>,
  #[serde(default)]
  pub capabilities: CapabilityGrants,
  #[serde(default)]
  pub wasm_module: Option<String>,
  #[serde(default)]
  pub env_vars: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSchema {
  pub name: String,
  pub version: SchemaVersion,
  pub fields: Vec<SchemaField>,
  pub schema_mode: SchemaMode,
  #[serde(default)]
  pub indexes: Vec<panorama_core::schema::SchemaIndex>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestBackgroundTask {
  pub name: String,
  pub interval_seconds: Option<u64>,
  pub description: String,
  #[serde(default)]
  pub wasm_function: Option<String>,
}

/// A loaded .panoapp file in memory
#[derive(Debug, Clone)]
pub struct PanoAppPackage {
  pub manifest: PanoAppManifest,
  pub wasm_bytes: Option<Vec<u8>>,
  pub ui_files: HashMap<String, Vec<u8>>,
}

impl PanoAppPackage {
  /// Load a .panoapp from a single ZIP file
  pub fn load_from_file(path: impl AsRef<Path>) -> Result<Self, String> {
    let file = std::fs::File::open(path.as_ref()).map_err(|e| e.to_string())?;
    let mut archive =
      zip::ZipArchive::new(file).map_err(|e| format!("Invalid .panoapp ZIP: {}", e))?;

    // Read manifest first
    let manifest: PanoAppManifest = {
      let mut mf = archive
        .by_name("manifest.json")
        .map_err(|_| "manifest.json not found in .panoapp".to_string())?;
      serde_json::from_reader(&mut mf).map_err(|e| format!("Invalid manifest.json: {}", e))?
    };

    if manifest.id.is_empty() {
      return Err("Manifest 'id' is required".into());
    }

    // Read WASM module
    let wasm_bytes = {
      let wasm_path = manifest.wasm_module.as_deref().unwrap_or("plugin.wasm");
      match archive.by_name(wasm_path) {
        Ok(mut wf) => {
          let mut buf = Vec::new();
          wf.read_to_end(&mut buf).map_err(|e| e.to_string())?;
          Some(buf)
        }
        Err(_) => None,
      }
    };

    // Read all UI files
    let mut ui_files = HashMap::new();
    for i in 0..archive.len() {
      if let Ok(mut entry) = archive.by_index(i) {
        let name = entry.name().to_string();
        if name.starts_with("ui/") && !name.ends_with('/') {
          let rel = name[3..].to_string(); // strip "ui/" prefix
          let mut buf = Vec::new();
          if entry.read_to_end(&mut buf).is_ok() {
            ui_files.insert(rel, buf);
          }
        }
      }
    }

    Ok(Self {
      manifest,
      wasm_bytes,
      ui_files,
    })
  }

  /// Convert manifest schemas to runtime Schema objects
  pub fn to_schemas(&self) -> Vec<Schema> {
    self
      .manifest
      .schemas
      .iter()
      .map(|ms| Schema {
        node_id: Uuid::new_v4(),
        name: format!("{}/{}", self.manifest.id, ms.name),
        version: ms.version.clone(),
        fields: ms.fields.clone(),
        schema_mode: ms.schema_mode.clone(),
        previous_versions: vec![],
        migrations: vec![],
        indexes: ms.indexes.clone(),
      })
      .collect()
  }
}

/// Build a .panoapp file from parts.
/// Used by plugin build scripts to create distributable packages.
pub fn build_panoapp(
  output_path: &Path,
  manifest: &PanoAppManifest,
  wasm_bytes: Option<&[u8]>,
  ui_files: &HashMap<String, Vec<u8>>,
) -> Result<(), String> {
  let file = std::fs::File::create(output_path).map_err(|e| e.to_string())?;
  let mut zw = zip::ZipWriter::new(file);
  let opts =
    zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

  // manifest.json
  zw.start_file("manifest.json", opts)
    .map_err(|e| e.to_string())?;
  let mj = serde_json::to_vec_pretty(manifest).map_err(|e| e.to_string())?;
  std::io::Write::write_all(&mut zw, &mj).map_err(|e| e.to_string())?;

  // plugin.wasm
  if let Some(wasm) = wasm_bytes {
    let wasm_path = manifest.wasm_module.as_deref().unwrap_or("plugin.wasm");
    zw.start_file(wasm_path, opts).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zw, wasm).map_err(|e| e.to_string())?;
  }

  // ui/*
  for (rel, content) in ui_files {
    zw.start_file(&format!("ui/{}", rel), opts)
      .map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zw, content).map_err(|e| e.to_string())?;
  }

  zw.finish().map_err(|e| e.to_string())?;
  Ok(())
}
