mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;
fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }
fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "restaurants") => { let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default(); let mut fields = HashMap::new(); fields.insert("system:node_title".into(), field_value("String", body["name"].as_str().unwrap_or("Unknown"))); if let Some(v) = body["cuisine"].as_str() { fields.insert("beli:cuisine".into(), field_value("String", v)); } if let Some(v) = body["location"].as_str() { fields.insert("beli:location".into(), field_value("String", v)); } let mut out = json_ok(serde_json::json!({"created":true})); out.effects.push(create_effect(fields)); out }
        ("GET", "restaurants") => { let r: Vec<&WasmNode> = input.nodes.iter().filter(|n| n.fields.contains_key("beli:cuisine") || n.fields.contains_key("beli:location")).collect(); json_ok(serde_json::json!(r)) }
        ("POST", "compare") => { let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default(); let mut fields = HashMap::new(); if let Some(v) = body["better_id"].as_str() { fields.insert("beli:better_id".into(), field_value("String", v)); } if let Some(v) = body["worse_id"].as_str() { fields.insert("beli:worse_id".into(), field_value("String", v)); } if let Some(v) = body["context"].as_str() { fields.insert("beli:context".into(), field_value("String", v)); } let mut out = json_ok(serde_json::json!({"compared":true})); out.effects.push(create_effect(fields)); out }
        ("GET", "rankings") => {
            let restaurants: Vec<&WasmNode> = input.nodes.iter().filter(|n| n.fields.contains_key("beli:cuisine") || n.fields.contains_key("beli:location")).collect();
            let comparisons: Vec<&WasmNode> = input.nodes.iter().filter(|n| n.fields.contains_key("beli:better_id")).collect();
            let mut in_deg: HashMap<&str, usize> = HashMap::new(); let mut edges: HashMap<&str, Vec<&str>> = HashMap::new(); let mut names: HashMap<&str, &str> = HashMap::new();
            for r in &restaurants { in_deg.entry(&r.id).or_insert(0); names.insert(&r.id, r.fields.get("system:node_title").and_then(|v| v["value"].as_str()).unwrap_or("Unknown")); }
            for c in &comparisons { let better = c.fields.get("beli:better_id").and_then(|v| v["value"].as_str()); let worse = c.fields.get("beli:worse_id").and_then(|v| v["value"].as_str()); if let (Some(b), Some(w)) = (better, worse) { edges.entry(b).or_default().push(w); *in_deg.entry(w).or_default() += 1; in_deg.entry(b).or_insert(0); } }
            let mut tiers: Vec<Vec<serde_json::Value>> = Vec::new(); let mut current: Vec<&str> = in_deg.iter().filter(|(_,&d)| d == 0).map(|(k,_)| *k).collect();
            while !current.is_empty() { let tier: Vec<serde_json::Value> = current.iter().map(|id| serde_json::json!({"id": id, "name": names.get(id).copied().unwrap_or("Unknown")})).collect(); tiers.push(tier); let mut next: Vec<&str> = Vec::new(); for id in &current { if let Some(neighbors) = edges.get(id) { for n in neighbors { if let Some(d) = in_deg.get_mut(n) { *d -= 1; if *d == 0 { next.push(n); } } } } } current = next; }
            json_ok(serde_json::json!({"tiers": tiers, "total_restaurants": restaurants.len(), "total_comparisons": comparisons.len()}))
        }
        _ => not_found()
    }
}
