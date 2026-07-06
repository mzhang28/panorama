//! Test-only WASM plugin for deterministic reactor testing.
//!
//! Exports reactor handler functions that return fixed, predictable results:
//! - `test_validate_approve` → Approved
//! - `test_validate_reject` → Rejected { reason: "test rejection" }
//! - `test_transform` → Transformed { new_value }
//! - `test_compute` → Computed { field_key, value }

use async_trait::async_trait;
use panorama_core::plugin::{HttpRequest, HttpResponse, Plugin, PluginContext, PluginError};
use panorama_core::reactor::{EagerReactorResult, ReactorActionInput, ReactorActionOutput};

pub struct TestReactorPlugin;

#[async_trait]
impl Plugin for TestReactorPlugin {
  fn id(&self) -> &str {
    "test.reactor.plugin"
  }
  fn name(&self) -> &str {
    "Test Reactor Plugin"
  }
  fn version(&self) -> &str {
    "0.1.0"
  }
  fn description(&self) -> &str {
    "Test-only WASM plugin for deterministic reactor testing"
  }

  fn required_capabilities(&self) -> panorama_core::capabilities::CapabilityGrants {
    panorama_core::capabilities::CapabilityGrants {
      field_write: vec!["test:*".to_string()],
      field_read: vec!["*".to_string()],
      write_own_nodes: true,
      ..Default::default()
    }
  }

  async fn handle_http_request(
    &self,
    endpoint: &str,
    request: HttpRequest,
    _ctx: &dyn PluginContext,
  ) -> Result<HttpResponse, PluginError> {
    // Reactor actions arrive as "__reactor__/{function_name}"
    if let Some(function_name) = endpoint.strip_prefix("__reactor__/") {
      return handle_reactor_action(function_name, &request);
    }
    Err(PluginError::not_found(&format!(
      "Unknown endpoint: {}",
      endpoint
    )))
  }
}

fn handle_reactor_action(
  function_name: &str,
  request: &HttpRequest,
) -> Result<HttpResponse, PluginError> {
  let body_str = request
    .body
    .as_ref()
    .map(|b| String::from_utf8_lossy(b).to_string())
    .unwrap_or_default();

  let _input: ReactorActionInput = serde_json::from_str(&body_str)
    .map_err(|e| PluginError::bad_request(&format!("Invalid reactor input: {}", e)))?;

  let output = match function_name {
    "test_validate_approve" => ReactorActionOutput {
      result: EagerReactorResult::Approved,
      log: Some("validate approved by test WASM".into()),
    },
    "test_validate_reject" => ReactorActionOutput {
      result: EagerReactorResult::Rejected {
        reason: "test rejection".into(),
      },
      log: Some("validate rejected by test WASM".into()),
    },
    "test_transform" => ReactorActionOutput {
      result: EagerReactorResult::Transformed {
        new_value: serde_json::json!("transformed-by-wasm"),
      },
      log: None,
    },
    "test_compute" => ReactorActionOutput {
      result: EagerReactorResult::Computed {
        field_key: "test:computed_value".into(),
        value: serde_json::json!(42),
      },
      log: None,
    },
    _ => ReactorActionOutput {
      result: EagerReactorResult::Rejected {
        reason: format!("Unknown reactor function: {}", function_name),
      },
      log: None,
    },
  };

  HttpResponse::json(&output)
}
