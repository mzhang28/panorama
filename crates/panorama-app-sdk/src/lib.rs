use std::ffi::CString;

use sys::RegisterCallback;

pub mod sys {
  use std::ffi::c_char;

  use crate::RegisterContext;

  pub type RegisterCallback = fn(*mut RegisterContext) -> ();

  extern "C" {
    pub fn register_endpoint(
      url_len: u64,
      url: *const c_char,
      callback: *mut RegisterCallback,
    );
  }
}

pub struct RegisterContext {}

pub fn register_endpoint(
  url: impl AsRef<str>,
  callback: fn(&RegisterContext) -> (),
) {
  let url = url.as_ref();
  let url_cstr = CString::new(url).unwrap();
  let mut callback2 = |ctx: *mut RegisterContext| callback(&*ctx);
  let result = unsafe {
    sys::register_endpoint(
      url.len() as u64,
      url_cstr.into_raw(),
      &mut callback2 as *mut RegisterCallback,
    )
  };
  println!("Result: {:?}", result);
}
