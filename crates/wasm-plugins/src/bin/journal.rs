mod common { include!("../common.rs"); }
use common::*;
use std::collections::HashMap;

fn main() { let input = read_input(); let output = handle(&input); write_output(&output); }

fn handle(input: &WasmInput) -> WasmOutput {
    let ep = input.endpoint.as_str();
    match (input.request.method.as_str(), input.endpoint.as_str()) {
        ("POST", "entries") => {
            let body: serde_json::Value = input.request.body.as_deref()
                .and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();
            let mut fields = HashMap::new();
            fields.insert("system:node_title".into(), field_value("String", body["title"].as_str().unwrap_or("Untitled")));
            let time_str = body["time"].as_str().map(|s| s.to_string())
                .unwrap_or_else(|| "2025-01-01T00:00:00Z".to_string());
            fields.insert("system:node_time".into(), field_value("DateTime", time_str.as_str()));
            let content = body["content"].as_str().unwrap_or("");
            fields.insert("journal:content".into(), field_value("String", content));
            fields.insert("journal:deleted".into(), field_value("Boolean", false));
            if let Some(m) = body["mood"].as_str() {
                if !m.is_empty() { fields.insert("journal:mood".into(), field_value("String", m)); }
            }

            match host_create(&fields) {
                Some(entry_id) => {
                    // Paragraph decomposition: split content by blank lines
                    let paragraphs: Vec<&str> = content.split("\n\n")
                        .map(|p| p.trim())
                        .filter(|p| !p.is_empty())
                        .collect();

                    if !paragraphs.is_empty() {
                        let mut para_refs: Vec<serde_json::Value> = Vec::new();
                        for (i, para_text) in paragraphs.iter().enumerate() {
                            let mut para_fields = HashMap::new();
                            para_fields.insert("system:node_title".into(), field_value("String", format!("Paragraph {}", i + 1)));
                            para_fields.insert("system:node_time".into(), field_value("DateTime", time_str.as_str()));
                            para_fields.insert("journal:content".into(), field_value("String", *para_text));
                            para_fields.insert("journal:entry_ref".into(), serde_json::json!({"type": "NodeRef", "value": entry_id}));
                            para_fields.insert("journal:paragraph_index".into(), field_value("Integer", i as i64));

                            if let Some(pid) = host_create(&para_fields) {
                                para_refs.push(serde_json::json!({"type": "NodeRef", "value": pid}));
                            }
                        }

                        // Update entry with paragraph refs
                        let mut refs_update = HashMap::new();
                        refs_update.insert("journal:paragraph_refs".into(), serde_json::json!({"type": "Array", "value": para_refs}));
                        host_update(&entry_id, &refs_update);
                    }

                    match host_get(&entry_id) {
                        Some(node) => json_ok(node),
                        None => json_ok(serde_json::json!({"id": entry_id, "created": true})),
                    }
                }
                None => json_ok(serde_json::json!({"created": false, "error": "create failed"})),
            }
        }
        ("GET", "entries") => {
            let mood_filter = input.request.query_params.get("mood");
            let from = input.request.query_params.get("from");
            let to = input.request.query_params.get("to");

            let mut preds: Vec<String> = Vec::new();
            preds.push("HAS_FIELD(n, \"journal\", \"content\")".to_string());

            if let Some(mood) = mood_filter {
                if !mood.is_empty() {
                    preds.push(format!("n.journal.mood = \"{}\"", mood.replace('"', "\\\"")));
                }
            }
            if let Some(from_date) = from {
                if !from_date.is_empty() {
                    preds.push(format!("n.system.node_time >= \"{}\"", from_date.replace('"', "\\\"")));
                }
            }
            if let Some(to_date) = to {
                if !to_date.is_empty() {
                    preds.push(format!("n.system.node_time <= \"{}\"", to_date.replace('"', "\\\"")));
                }
            }

            let query = format!(
                "MATCH (n) IN space(\"default\") WHERE {} RETURN n ORDER BY n.system.node_time DESC LIMIT 100",
                preds.join(" AND ")
            );
            let rows = host_run_query(&query);
            let nodes: Vec<WasmNode> = rows.iter().filter_map(row_to_wasm_node).collect();
            json_ok(serde_json::json!(nodes))
        }
        ("GET", _) if ep.starts_with("entries/") && !ep.ends_with("/paragraphs") => {
            let id = &ep["entries/".len()..];
            match host_get(id) {
                Some(node) => json_ok(node),
                None => not_found(),
            }
        }
        ("PUT", _) if ep.starts_with("entries/") => {
            let id = &ep["entries/".len()..];
            let body: serde_json::Value = input.request.body.as_deref()
                .and_then(|b| serde_json::from_str(b).ok()).unwrap_or_default();

            let mut updates = HashMap::new();
            if let Some(title) = body.get("title").and_then(|v| v.as_str()) {
                updates.insert("system:node_title".into(), field_value("String", title));
            }
            if let Some(content) = body.get("content").and_then(|v| v.as_str()) {
                updates.insert("journal:content".into(), field_value("String", content));
                updates.insert("system:node_time".into(), field_value("DateTime", "2025-01-01T00:00:00Z"));
            }
            if let Some(mood) = body.get("mood").and_then(|v| v.as_str()) {
                updates.insert("journal:mood".into(), field_value("String", mood));
            }

            if updates.is_empty() {
                json_ok(serde_json::json!({"error": "no fields to update"}))
            } else {
                host_update(id, &updates);
                match host_get(id) {
                    Some(node) => json_ok(node),
                    None => json_ok(serde_json::json!({"id": id, "updated": true})),
                }
            }
        }
        ("DELETE", _) if ep.starts_with("entries/") => {
            let id = &ep["entries/".len()..];
            let mut updates = HashMap::new();
            updates.insert("journal:deleted".into(), field_value("Boolean", true));
            host_update(id, &updates);
            match host_get(id) {
                Some(node) => json_ok(node),
                None => json_ok(serde_json::json!({"id": id, "deleted": true})),
            }
        }
        _ => not_found()
    }
}
