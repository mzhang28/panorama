mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;
fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }
fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("GET", "rest/ping") => json_ok(serde_json::json!({"subsonic-response": {"status":"ok","version":"1.16.1","type":"panorama","serverVersion":"0.1.0"}})),
        ("GET", "rest/getArtists") => { let artists: Vec<serde_json::Value> = input.nodes.iter().filter(|n| !n.fields.contains_key("subsonic:artist_id") && !n.fields.contains_key("subsonic:audio_ref") && n.fields.contains_key("system:node_title")).map(|n| serde_json::json!({"id": n.id, "name": n.fields.get("system:node_title").and_then(|v| v["value"].as_str()).unwrap_or("Unknown")})).collect(); json_ok(serde_json::json!({"subsonic-response":{"status":"ok","artists":{"index":[{"artist":artists}]}}})) }
        ("GET", "rest/getAlbumList2") => { let albums: Vec<serde_json::Value> = input.nodes.iter().filter(|n| n.fields.contains_key("subsonic:artist_id") && !n.fields.contains_key("subsonic:audio_ref")).map(|n| serde_json::json!({"id": n.id, "name": n.fields.get("system:node_title").and_then(|v| v["value"].as_str()).unwrap_or("Unknown"), "artistId": n.fields.get("subsonic:artist_id").and_then(|v| v["value"].as_str()).unwrap_or("")})).collect(); json_ok(serde_json::json!({"subsonic-response":{"status":"ok","albumList2":{"album":albums}}})) }
        ("GET", "rest/stream") => json_ok(serde_json::json!({"subsonic-response":{"status":"ok","stream":"no audio data"}})),
        ("POST", "upload") => { let filename = input.request.query_params.get("filename").cloned().unwrap_or_default(); let title = input.request.query_params.get("title").cloned().unwrap_or_else(|| filename.clone()); let mut fields = HashMap::new(); fields.insert("system:node_title".into(), field_value("String", title)); if !filename.is_empty() { fields.insert("subsonic:audio_ref".into(), serde_json::json!({"type":"ObjectRef","value":{"bucket":"subsonic-audio","key":filename,"size":0}})); } let mut out = json_ok(serde_json::json!({"uploaded":true})); out.effects.push(create_effect(fields)); out }
        _ => not_found()
    }
}
