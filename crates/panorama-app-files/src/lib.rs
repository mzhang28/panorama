//! File Upload App - Resumable file uploads and object storage management.
//! Demonstrates from DESIGN.md: object storage API, resumable uploads, file browsing.
//!
//! Workflow:
//! - Upload files (stored in object storage, file node created for metadata)
//! - Initiate resumable uploads for large files
//! - Browse uploaded files as nodes with folder grouping
//! - Download files from object storage with Content-Disposition header
//! - Delete files and their associated object storage data

use async_trait::async_trait;
use panorama_core::*;
use uuid::Uuid;

pub struct FilesPlugin;

impl FilesPlugin {
  pub fn new() -> Self {
    Self
  }

  fn file_schema() -> Schema {
    Schema {
      node_id: Uuid::nil(),
      name: "files/File".to_string(),
      version: SchemaVersion::new(1, 0),
      fields: vec![
        SchemaField {
          name: "node_title".to_string(),
          namespace: "system".to_string(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "String".into(),
            element_type: None,
          }),
          required: true,
          default: None,
          description: Some("File name".to_string()),
          computed: None,
        },
        SchemaField {
          name: "object_ref".to_string(),
          namespace: "files".to_string(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "ObjectRef".into(),
            element_type: None,
          }),
          required: true,
          default: None,
          description: Some("Object storage reference".to_string()),
          computed: None,
        },
        SchemaField {
          name: "file_size".to_string(),
          namespace: "files".to_string(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "Integer".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("File size in bytes".to_string()),
          computed: None,
        },
        SchemaField {
          name: "mime_type".to_string(),
          namespace: "files".to_string(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "String".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("MIME type".to_string()),
          computed: None,
        },
        SchemaField {
          name: "folder".to_string(),
          namespace: "files".to_string(),
          field_type: Some(FieldTypeConstraint {
            type_tag: "String".into(),
            element_type: None,
          }),
          required: false,
          default: None,
          description: Some("Virtual folder path".to_string()),
          computed: None,
        },
      ],
      schema_mode: SchemaMode::Preferred,
      previous_versions: vec![],
      migrations: vec![],
    }
  }
}

#[async_trait]
impl Plugin for FilesPlugin {
  fn id(&self) -> &str {
    "io.mzhang.panorama.files"
  }

  fn name(&self) -> &str {
    "File Manager"
  }

  fn version(&self) -> &str {
    "0.1.0"
  }

  fn description(&self) -> &str {
    "File uploads with resumable transfers and object storage management"
  }

  fn schemas(&self) -> Vec<Schema> {
    vec![Self::file_schema()]
  }

  fn http_endpoints(&self) -> Vec<HttpEndpoint> {
    vec![
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/upload".to_string(),
        description: "Upload a file (stores in object storage + creates file node)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::POST,
        path: "/upload/initiate".to_string(),
        description: "Initiate a resumable upload".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/files".to_string(),
        description: "List files (optionally filtered by folder)".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::GET,
        path: "/files/{id}".to_string(),
        description: "Download a file with Content-Disposition header".to_string(),
      },
      HttpEndpoint {
        method: HttpMethod::DELETE,
        path: "/files/{id}".to_string(),
        description: "Delete a file and its object storage data".to_string(),
      },
    ]
  }

  fn ui_components(&self) -> Vec<UiComponent> {
    vec![
      UiComponent {
        id: "files-main".to_string(),
        name: "File Browser".to_string(),
        mount_point: UiMountPoint::MainPage,
        bundle_path: "ui/files.js".to_string(),
      },
      UiComponent {
        id: "files-upload".to_string(),
        name: "File Upload".to_string(),
        mount_point: UiMountPoint::Sidebar,
        bundle_path: "ui/upload.js".to_string(),
      },
    ]
  }

  fn required_capabilities(&self) -> CapabilityGrants {
    CapabilityGrants {
      field_read: vec!["files:*".to_string(), "system:node_title".to_string()],
      field_write: vec!["files:*".to_string(), "system:node_title".to_string()],
      object_storage_read: true,
      object_storage_write: true,
      write_own_nodes: true,
      ..Default::default()
    }
  }

  async fn handle_http_request(
    &self,
    endpoint: &str,
    request: HttpRequest,
    ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    match (request.method.as_str(), endpoint) {
      ("POST", "upload") => {
        let body = request
          .body
          .ok_or_else(|| PluginError::bad_request("No file data"))?;
        let filename = request
          .query_params
          .get("filename")
          .cloned()
          .unwrap_or_else(|| "unnamed".to_string());
        let mime_type = request
          .query_params
          .get("mime_type")
          .cloned()
          .unwrap_or_else(|| "application/octet-stream".to_string());
        let folder = request.query_params.get("folder").cloned();

        // Store in object storage
        let obj_ref = ctx.put_object("files", &filename, body, &mime_type).await?;

        // Create file node with metadata
        let mut node = Node::new(Uuid::nil());
        node.set_field("system:node_title", FieldValue::String(filename));
        node.set_field("files:object_ref", FieldValue::ObjectRef(obj_ref.clone()));
        node.set_field("files:file_size", FieldValue::Integer(obj_ref.size as i64));
        node.set_field("files:mime_type", FieldValue::String(mime_type));
        if let Some(f) = folder {
          node.set_field("files:folder", FieldValue::String(f));
        }
        let file_node = ctx.create_node(node).await?;
        HttpResponse::json(&file_node)
      }
      ("POST", "upload/initiate") => {
        // Delegate to the platform's resumable upload API
        HttpResponse::json(&serde_json::json!({
            "message": "Use platform upload API at /api/uploads for resumable uploads",
            "endpoint": format!("{}/api/uploads", ctx.base_path())
        }))
      }
      ("GET", "files") => {
        let folder = request.query_params.get("folder").cloned();
        let rows = ctx.query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"files\", \"object_ref\") RETURN n ORDER BY n.system.updated_at DESC").await?;
        let mut files: Vec<Node> = rows
          .iter()
          .filter_map(panorama_core::query::row_to_node)
          .collect();
        if let Some(f) = folder {
          files.retain(|n| {
            n.get_field("files:folder")
              .map(|v| match v {
                FieldValue::String(s) => s == &f,
                _ => false,
              })
              .unwrap_or(false)
          });
        }
        HttpResponse::json(&files)
      }
      ("GET", _) if endpoint.starts_with("files/") => {
        let id_str = &endpoint["files/".len()..];
        let id = Uuid::parse_str(id_str).map_err(|_| PluginError::bad_request("Invalid UUID"))?;
        let node = ctx
          .get_node(id)
          .await?
          .ok_or_else(|| PluginError::not_found("File not found"))?;

        if let Some(FieldValue::ObjectRef(obj_ref)) = node.get_field("files:object_ref") {
          let obj = ctx
            .get_object(&obj_ref.bucket, &obj_ref.key)
            .await?
            .ok_or_else(|| PluginError::not_found("Object data not found"))?;
          let mut headers = std::collections::HashMap::new();
          headers.insert("Content-Type".to_string(), obj.mime_type);
          headers.insert(
            "Content-Disposition".to_string(),
            format!(
              "attachment; filename=\"{}\"",
              node.title().unwrap_or("file")
            ),
          );
          Ok(HttpResponse {
            status: 200,
            headers,
            body: obj.data,
          })
        } else {
          Err(PluginError::not_found("No object reference on file node"))
        }
      }
      ("DELETE", _) if endpoint.starts_with("files/") => {
        let id_str = &endpoint["files/".len()..];
        let id = Uuid::parse_str(id_str).map_err(|_| PluginError::bad_request("Invalid UUID"))?;
        let node = ctx
          .get_node(id)
          .await?
          .ok_or_else(|| PluginError::not_found("File not found"))?;

        if let Some(FieldValue::ObjectRef(obj_ref)) = node.get_field("files:object_ref") {
          ctx.delete_object(&obj_ref.bucket, &obj_ref.key).await?;
        }
        ctx.delete_node(id).await?;
        let headers = std::collections::HashMap::new();
        Ok(HttpResponse {
          status: 204,
          headers,
          body: bytes::Bytes::new(),
        })
      }
      _ => Err(PluginError::not_found(&format!(
        "Unknown endpoint: {}",
        endpoint
      ))),
    }
  }
}
