#![no_std]

use chrono::{DateTime, Utc};

pub extern crate wee_alloc;

#[macro_use]
extern crate alloc;

// Reference for FFI closures
// https://adventures.michaelfbryan.com/posts/rust-closures-in-ffi/

#[macro_use]
pub mod macros;
pub mod internal;
pub mod sys;

pub mod prelude {}

/// Returns the current time
pub fn get_current_time() -> DateTime<Utc> {
  let result = unsafe { sys::_get_current_time() };
  DateTime::from_timestamp_nanos(result)
}
