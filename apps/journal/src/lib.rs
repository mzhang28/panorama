use std::ffi::CString;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub fn install() -> i32 {
  panorama_app_sdk::register_endpoint("/hello");
  123
}
