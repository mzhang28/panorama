mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;

fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }

fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "restaurants") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["name"].as_str().unwrap_or("Unknown")));
            if let Some(v) = body["cuisine"].as_str() { fields.insert("beli:cuisine".into(), field_value("String", v)); }
            if let Some(v) = body["location"].as_str() { fields.insert("beli:location".into(), field_value("String", v)); }
            host_create(&fields);
            json_ok(serde_json::json!({"created":true}))
        }
        ("POST", "compare") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            if let Some(v) = body["better_id"].as_str() { fields.insert("beli:better_id".into(), field_value("String", v)); }
            if let Some(v) = body["worse_id"].as_str() { fields.insert("beli:worse_id".into(), field_value("String", v)); }
            if let Some(v) = body["context"].as_str() { fields.insert("beli:context".into(), field_value("String", v)); }
            host_create(&fields);
            json_ok(serde_json::json!({"compared":true}))
        }
        ("GET", "restaurants") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"beli\", \"cuisine\") RETURN n");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        ("GET", "rankings") => {
            let all_rows = host_run_query("MATCH (n) IN space(\"default\") RETURN n");
            let all_nodes: Vec<WasmNode> = all_rows.iter().filter_map(row_to_wasm_node).collect();
            let restaurants: Vec<_> = all_nodes.iter().filter(|n| n.fields.contains_key("beli:cuisine") || n.fields.contains_key("beli:location")).collect();
            let comparisons: Vec<_> = all_nodes.iter().filter(|n| n.fields.contains_key("beli:better_id")).collect();
            let mut in_deg: HashMap<&str, usize> = HashMap::new();
            let mut edges: HashMap<&str, Vec<&str>> = HashMap::new();
            let mut names: HashMap<&str, String> = HashMap::new();
            for r in &restaurants {
                let id = r.id.as_str();
                in_deg.entry(id).or_insert(0);
                names.insert(id, fstr(r, "system:node_title").unwrap_or_else(|| "Unknown".to_string()));
            }
            for c in &comparisons {
                let better = fstr(c, "beli:better_id");
                let worse = fstr(c, "beli:worse_id");
                if let (Some(b), Some(w)) = (better, worse) {
                    let b_ref = names.keys().find(|k| **k == b).copied();
                    let w_ref = names.keys().find(|k| **k == w).copied();
                    if let (Some(b_id), Some(w_id)) = (b_ref, w_ref) {
                        edges.entry(b_id).or_default().push(w_id);
                        *in_deg.entry(w_id).or_default() += 1;
                        in_deg.entry(b_id).or_insert(0);
                    }
                }
            }
            let mut tiers: Vec<Vec<serde_json::Value>> = Vec::new();
            let mut current: Vec<&str> = in_deg.iter().filter(|(_,&d)| d == 0).map(|(k,_)| *k).collect();
            while !current.is_empty() {
                let tier: Vec<serde_json::Value> = current.iter().map(|id| serde_json::json!({"id": id, "name": names.get(id).cloned().unwrap_or_else(|| "Unknown".to_string())})).collect();
                tiers.push(tier);
                let mut next: Vec<&str> = Vec::new();
                for id in &current {
                    if let Some(neighbors) = edges.get(id) {
                        for n in neighbors {
                            if let Some(d) = in_deg.get_mut(n) {
                                *d -= 1;
                                if *d == 0 { next.push(n); }
                            }
                        }
                    }
                }
                current = next;
            }
            json_ok(serde_json::json!({"tiers": tiers, "total_restaurants": restaurants.len(), "total_comparisons": comparisons.len()}))
        }
        _ => not_found()
    }
}
