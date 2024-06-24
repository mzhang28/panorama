// use crate::RegisterCallback;

use core::ffi::{c_char, c_void};

extern "C" {
  pub fn _print(len: u64, ptr: *const c_char);

  pub fn register_endpoint(
    url_len: u64,
    url: *const c_char,
    // callback: *mut RegisterCallback,
    callback_data: *mut c_void,
  );

  /// Returns the current time in nanoseconds
  pub fn _get_current_time() -> i64;
}
