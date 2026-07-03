mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;

fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }

fn handle(input: &WasmInput) -> WasmOutput {
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "query") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let agg = body["aggregation"].as_str().unwrap_or("count");
            let gb = body["group_by"].as_str();
            let rows = host_run_query("MATCH (n) IN space(\"default\") RETURN n ORDER BY n.system.node_time ASC");
            let all_nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            let nodes: Vec<&WasmNode> = all_nodes.iter().filter(|n| n.fields.contains_key("wakatime:entity")).collect();
            match agg {
                "count" if gb.is_some() => {
                    let gb_key = gb.unwrap();
                    let mut groups: HashMap<String, usize> = HashMap::new();
                    for n in &nodes {
                        if let Some(k) = fstr(n, gb_key) {
                            *groups.entry(k).or_default() += 1;
                        }
                    }
                    let result: Vec<_> = groups.iter().map(|(k,c)| serde_json::json!({"key":k,"count":c})).collect();
                    json_ok(serde_json::json!(result))
                }
                "leaderboard" => {
                    let gb_key = gb.unwrap_or("wakatime:project");
                    let mut groups: HashMap<String, f64> = HashMap::new();
                    for n in &nodes {
                        if let Some(k) = fstr(n, gb_key) {
                            *groups.entry(k).or_default() += ff64(n, "wakatime:duration").unwrap_or(0.0);
                        }
                    }
                    let mut result: Vec<_> = groups.iter().map(|(k,s)| serde_json::json!({"project":k,"hours":s/3600.0})).collect();
                    result.sort_by(|a,b| b["hours"].as_f64().partial_cmp(&a["hours"].as_f64()).unwrap_or(std::cmp::Ordering::Equal));
                    json_ok(serde_json::json!(result))
                }
                _ => json_ok(serde_json::json!({"total": nodes.len()}))
            }
        }
        ("POST", "dashboards") => {
            let body: serde_json::Value = input.request.body.as_deref().and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["title"].as_str().unwrap_or("Untitled")));
            fields.insert("grafana:config".into(), field_value("Json", body));
            host_create(&fields);
            json_ok(serde_json::json!({"saved":true}))
        }
        ("GET", "dashboards") => {
            let rows = host_run_query("MATCH (n) IN space(\"default\") WHERE HAS_FIELD(n, \"grafana\", \"config\") RETURN n ORDER BY n.system.updated_at DESC");
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        _ => not_found()
    }
}
