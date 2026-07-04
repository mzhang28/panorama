//! Trip Planner App - Plan trips with events, calendar and map views.
//! Demonstrates from DESIGN.md: event scheduling, time-range queries, geolocation data.
//!
//! Workflow:
//! - Create trips with start/end dates
//! - Create events with time, location (lat/lng), and notes scoped to a trip
//! - View events in calendar format (time-based sorting)
//! - View events on a map (geolocation field query)

use async_trait::async_trait;
use panorama_core::*;
use uuid::Uuid;

pub struct TripsPlugin;

impl TripsPlugin {
    pub fn new() -> Self {
        Self
    }

    fn trip_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "trips/Trip".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "String".into(), element_type: None }),
                    required: true,
                    default: None,
                    description: Some("Trip name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "start_date".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "DateTime".into(), element_type: None }),
                    required: true,
                    default: None,
                    description: Some("Trip start date".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "end_date".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "DateTime".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Trip end date".to_string()),
                    computed: None,
                },
            ],
            schema_mode: SchemaMode::Preferred,
            previous_versions: vec![],
            migrations: vec![],
        }
    }

    fn event_schema() -> Schema {
        Schema {
            node_id: Uuid::nil(),
            name: "trips/Event".to_string(),
            version: SchemaVersion::new(1, 0),
            fields: vec![
                SchemaField {
                    name: "node_title".to_string(),
                    namespace: "system".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "String".into(), element_type: None }),
                    required: true,
                    default: None,
                    description: Some("Event name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "node_start_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "DateTime".into(), element_type: None }),
                    required: true,
                    default: None,
                    description: Some("Event start time".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "node_end_time".to_string(),
                    namespace: "system".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "DateTime".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Event end time".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "trip_id".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "NodeRef".into(), element_type: None }),
                    required: true,
                    default: None,
                    description: Some("Reference to the parent trip".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "latitude".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "Float".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Event location latitude".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "longitude".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "Float".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Event location longitude".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "location_name".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "String".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Human-readable location name".to_string()),
                    computed: None,
                },
                SchemaField {
                    name: "notes".to_string(),
                    namespace: "trips".to_string(),
                    field_type: Some(FieldTypeConstraint { type_tag: "String".into(), element_type: None }),
                    required: false,
                    default: None,
                    description: Some("Event notes/description".to_string()),
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
impl Plugin for TripsPlugin {
    fn id(&self) -> &str {
        "io.mzhang.panorama.trips"
    }

    fn name(&self) -> &str {
        "Trip Planner"
    }

    fn version(&self) -> &str {
        "0.1.0"
    }

    fn description(&self) -> &str {
        "Trip planner with events, calendar and map views"
    }

    fn schemas(&self) -> Vec<Schema> {
        vec![Self::trip_schema(), Self::event_schema()]
    }

    fn http_endpoints(&self) -> Vec<HttpEndpoint> {
        vec![
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/trips".to_string(),
                description: "Create a trip".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/trips".to_string(),
                description: "List trips".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::POST,
                path: "/events".to_string(),
                description: "Create an event".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/events".to_string(),
                description: "List events (optionally filtered by trip_id)".to_string(),
            },
            HttpEndpoint {
                method: HttpMethod::GET,
                path: "/events/map".to_string(),
                description: "Get events with geo data for map view".to_string(),
            },
        ]
    }

    fn ui_components(&self) -> Vec<UiComponent> {
        vec![
            UiComponent {
                id: "trips-main".to_string(),
                name: "Trip Planner".to_string(),
                mount_point: UiMountPoint::MainPage,
                bundle_path: "ui/trips.js".to_string(),
            },
            UiComponent {
                id: "trips-calendar".to_string(),
                name: "Calendar View".to_string(),
                mount_point: UiMountPoint::Custom("calendar".to_string()),
                bundle_path: "ui/calendar.js".to_string(),
            },
            UiComponent {
                id: "trips-map".to_string(),
                name: "Map View".to_string(),
                mount_point: UiMountPoint::Custom("map".to_string()),
                bundle_path: "ui/map.js".to_string(),
            },
        ]
    }

    fn required_capabilities(&self) -> CapabilityGrants {
        CapabilityGrants {
            field_read: vec![
                "trips:*".to_string(),
                "system:node_title".to_string(),
                "system:node_start_time".to_string(),
                "system:node_end_time".to_string(),
            ],
            field_write: vec![
                "trips:*".to_string(),
                "system:node_title".to_string(),
                "system:node_start_time".to_string(),
                "system:node_end_time".to_string(),
            ],
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
            ("POST", "trips") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let mut node = Node::new(Uuid::nil());
                node.set_field(
                    "system:node_title",
                    FieldValue::String(
                        body["title"].as_str().unwrap_or("Untitled Trip").to_string(),
                    ),
                );
                if let Some(start) = body["start_date"].as_str() {
                    node.set_field("trips:start_date", FieldValue::DateTime(start.to_string()));
                }
                if let Some(end) = body["end_date"].as_str() {
                    node.set_field("trips:end_date", FieldValue::DateTime(end.to_string()));
                }
                let trip = ctx.create_node(node).await?;
                HttpResponse::json(&trip)
            }
            ("GET", "trips") => {
                let rows = ctx.query(
                    "MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"start_date\") RETURN n"
                ).await?;
                HttpResponse::json(&rows)
            }
            ("POST", "events") => {
                let body: serde_json::Value = serde_json::from_slice(
                    request.body.as_deref().unwrap_or(&[]),
                )
                .map_err(|e| PluginError::bad_request(&e.to_string()))?;
                let mut node = Node::new(Uuid::nil());
                node.set_field(
                    "system:node_title",
                    FieldValue::String(
                        body["title"].as_str().unwrap_or("Untitled Event").to_string(),
                    ),
                );
                if let Some(start) = body["start_time"].as_str() {
                    node.set_field(
                        "system:node_start_time",
                        FieldValue::DateTime(start.to_string()),
                    );
                }
                if let Some(end) = body["end_time"].as_str() {
                    node.set_field(
                        "system:node_end_time",
                        FieldValue::DateTime(end.to_string()),
                    );
                }
                if let Some(trip_id) = body["trip_id"].as_str() {
                    node.set_field("trips:trip_id", FieldValue::String(trip_id.to_string()));
                }
                if let Some(lat) = body["latitude"].as_f64() {
                    node.set_field("trips:latitude", FieldValue::Float(lat));
                }
                if let Some(lon) = body["longitude"].as_f64() {
                    node.set_field("trips:longitude", FieldValue::Float(lon));
                }
                if let Some(loc) = body["location_name"].as_str() {
                    node.set_field(
                        "trips:location_name",
                        FieldValue::String(loc.to_string()),
                    );
                }
                if let Some(notes) = body["notes"].as_str() {
                    node.set_field("trips:notes", FieldValue::String(notes.to_string()));
                }
                let event = ctx.create_node(node).await?;
                HttpResponse::json(&event)
            }
            ("GET", "events") => {
                let trip_filter = request.query_params.get("trip_id");
                let rows = ctx.query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"trip_id\") RETURN n").await?;
                let mut nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();
                if let Some(tid) = trip_filter {
                    nodes.retain(|n| {
                        n.get_field("trips:trip_id")
                            .map(|v| match v {
                                FieldValue::String(s) => s == tid,
                                _ => false,
                            })
                            .unwrap_or(false)
                    });
                }
                nodes.sort_by(|a, b| {
                    let ta = a
                        .get_field("system:node_start_time")
                        .and_then(|v| match v {
                            FieldValue::DateTime(s) => Some(s.clone()),
                            _ => None,
                        });
                    let tb = b
                        .get_field("system:node_start_time")
                        .and_then(|v| match v {
                            FieldValue::DateTime(s) => Some(s.clone()),
                            _ => None,
                        });
                    ta.cmp(&tb)
                });
                HttpResponse::json(&nodes)
            }
            ("GET", "events/map") => {
                let rows = ctx.query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"trips\", \"latitude\") RETURN n").await?;
                let nodes: Vec<Node> = rows.iter().filter_map(panorama_core::query::row_to_node).collect();
                let map_events: Vec<_> = nodes
                    .into_iter()
                    .filter(|n| {
                        n.get_field("trips:latitude").is_some()
                            && n.get_field("trips:longitude").is_some()
                    })
                    .map(|n| {
                        serde_json::json!({
                            "id": n.id.to_string(),
                            "title": n.title(),
                            "latitude": match n.get_field("trips:latitude") {
                                Some(FieldValue::Float(f)) => *f,
                                _ => 0.0,
                            },
                            "longitude": match n.get_field("trips:longitude") {
                                Some(FieldValue::Float(f)) => *f,
                                _ => 0.0,
                            },
                            "location": match n.get_field("trips:location_name") {
                                Some(FieldValue::String(s)) => s.clone(),
                                _ => String::new(),
                            },
                        })
                    })
                    .collect();
                HttpResponse::json(&map_events)
            }
            _ => Err(PluginError::not_found(&format!(
                "Unknown endpoint: {}",
                endpoint
            ))),
        }
    }
}
