#[macro_use]
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate sugars;

pub mod migrations;
pub mod state;

#[cfg(test)]
mod tests;

use std::fmt;

pub use crate::state::AppState;

use miette::{bail, IntoDiagnostic, Result};
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeId(Uuid);

impl fmt::Display for NodeId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0.to_string())
  }
}

pub fn ensure_ok(s: &str) -> Result<()> {
  let status: Value = serde_json::from_str(&s).into_diagnostic()?;
  let status = status.as_object().unwrap();
  let ok = status.get("ok").unwrap().as_bool().unwrap_or(false);
  if !ok {
    let display = status.get("display").unwrap().as_str().unwrap();
    bail!("shit (error: {display})")
  }
  Ok(())
}
