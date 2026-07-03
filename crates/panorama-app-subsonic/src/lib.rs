//! Subsonic-compatible Music Streaming App.
//! Demonstrates from DESIGN.md: external protocol compatibility, streaming, object storage.
//!
//! Workflow:
//! - Expose Subsonic API endpoints (ping, getArtists, getAlbumList2, stream) for music clients
//! - Store music metadata (artists, albums, tracks) as nodes
//! - Store audio files in object storage
//! - Stream audio with proper Content-Type and Content-Length headers

use async_trait::async_trait;
use panorama_core::*;
use uuid::Uuid;

pub struct SubsonicPlugin;

impl SubsonicPlugin {
    pub fn new() -> Self {
        Self
    }

    fn artist_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "subsonic/Artist".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Artist name".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn album_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "subsonic/Album".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Album name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "artist_id".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Reference to artist node".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "year".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Release year".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "cover_art_ref".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Object storage ref for cover art".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn track_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "subsonic/Track".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Track title".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "album_id".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Reference to album node".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "artist_id".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Reference to artist node".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "track_number".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Track number in album".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "duration".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: false,
                    default: None,
                    description: Some("Duration in seconds".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "audio_ref".to_string(),
                    namespace: "subsonic".to_string(),
                    field_type: None,
                    required: true,
                    default: None,
                    description: Some("Object storage ref for audio file".to_string()),
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
impl Plugin for SubsonicPlugin {
    fn id(&self) -> &str {
        "com.panorama.subsonic"
    }

    fn name(&self) -> &str {
        "Subsonic Music"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Subsonic-compatible music streaming interface"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::artist_schema(), Self::album_schema(), Self::track_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/rest/ping".to_string(),
                description: "Subsonic ping endpoint for client compatibility".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/rest/getArtists".to_string(),
                description: "List all artists".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/rest/getAlbumList2".to_string(),
                description: "List all albums".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/rest/stream".to_string(),
                description: "Stream audio file with proper headers".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/upload".to_string(),
                description: "Upload a music file to object storage".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![UiComponent {
            id: "subsonic-library".to_string(),
            name: "Music Library".to_string(),
            mount_point: UiMountPoint::MainPage,
            bundle_path: "ui/subsonic.js".to_string(),
        }]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_read: vec![
                "subsonic:*".to_string(),
                "system:node_title".to_string(),
            ],
            field_write: vec![
                "subsonic:*".to_string(),
                "system:node_title".to_string(),
            ],
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
            ("GET", "rest/ping") => {
                // Subsonic-compatible ping response
                HttpResponse::json(&serde_json::json!({
                    "subsonic-response": {
                        "status": "ok",
                        "version": "1.16.1",
                        "type": "panorama",
                        "serverVersion": "0.1.0"
                    }
                }))
            }
            ("GET", "rest/getArtists") => {
                let nodes = ctx.query_nodes(NodeQuery::new()).await?;
                let artists: Vec<_> = nodes
                    .iter()
                    .filter(|n| {
                        n.get_field("subsonic:artist_id").is_none()
                            && n.get_field("subsonic:album_id").is_none()
                            && n.get_field("subsonic:audio_ref").is_none()
                            && n.title().is_some()
                    })
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id.to_string(),
                            "name": n.title().unwrap_or("Unknown"),
                        })
                    })
                    .collect();
                HttpResponse::json(&serde_json::json!({
                    "subsonic-response": {
                        "status": "ok",
                        "artists": { "index": [{ "artist": artists }] }
                    }
                }))
            }
            ("GET", "rest/getAlbumList2") => {
                let nodes = ctx.query_nodes(NodeQuery::new()).await?;
                let albums: Vec<_> = nodes
                    .iter()
                    .filter(|n| {
                        n.get_field("subsonic:artist_id").is_some()
                            && n.get_field("subsonic:audio_ref").is_none()
                    })
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id.to_string(),
                            "name": n.title().unwrap_or("Unknown"),
                            "artistId": match n.get_field("subsonic:artist_id") {
                                Some(FieldValue::String(s)) => s.clone(),
                                _ => String::new(),
                            },
                        })
                    })
                    .collect();
                HttpResponse::json(&serde_json::json!({
                    "subsonic-response": {
                        "status": "ok",
                        "albumList2": { "album": albums }
                    }
                }))
            }
            ("GET", "rest/stream") => {
                let track_id = request
                    .query_params
                    .get("id")
                    .ok_or_else(|| PluginError::bad_request("id query parameter required"))?;
                let id = Uuid::parse_str(track_id)
                    .map_err(|_| PluginError::bad_request("Invalid track ID"))?;
                let node = ctx
                    .get_node(id)
                    .await?
                    .ok_or_else(|| PluginError::not_found("Track not found"))?;

                // Get audio ref from node and stream from object storage
                if let Some(FieldValue::ObjectRef(obj_ref)) = node.get_field("subsonic:audio_ref") {
                    let obj = ctx
                        .get_object(&obj_ref.bucket, &obj_ref.key)
                        .await?
                        .ok_or_else(|| PluginError::not_found("Audio file not found"))?;
                    let mut headers = std::collections::HashMap::new();
                    headers.insert("Content-Type".to_string(), obj.mime_type);
                    headers.insert("Content-Length".to_string(), obj.size.to_string());
                    Ok(HttpResponse {
                        status: 200,
                        headers,
                        body: obj.data,
                    })
                } else {
                    Err(PluginError::not_found("No audio file associated with this track"))
                }
            }
            ("POST", "upload") => {
                let body =
                    request.body.ok_or_else(|| PluginError::bad_request("No file data"))?;
                let filename = request
                    .query_params
                    .get("filename")
                    .cloned()
                    .unwrap_or_else(|| "unknown.mp3".to_string());
                let title = request
                    .query_params
                    .get("title")
                    .cloned()
                    .unwrap_or_else(|| filename.clone());

                // Store audio in object storage
                let obj_ref = ctx
                    .put_object("subsonic-audio", &filename, body, "audio/mpeg")
                    .await?;

                // Create track node
                let mut node = Node::new(Uuid::nil());
                node.set_field("system:node_title", FieldValue::String(title));
                node.set_field("subsonic:audio_ref", FieldValue::ObjectRef(obj_ref));
                let track = ctx.create_node(node).await?;
                HttpResponse::json(&track)
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
