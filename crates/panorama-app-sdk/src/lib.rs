// Reference for FFI closures
// https://adventures.michaelfbryan.com/posts/rust-closures-in-ffi/

use std::ffi::{c_char, c_void, CString};

pub mod sys {
  use std::ffi::{c_char, c_void};

  use crate::RegisterCallback;

  extern "C" {
    pub fn register_endpoint(
      url_len: u64,
      url: *const c_char,
      callback: *mut RegisterCallback,
      callback_data: *mut c_void,
    );
  }
}

pub struct RegisterContext {}
pub type RegisterCallback = fn(*mut RegisterContext) -> ();

unsafe extern "C" fn register_endpoint_callback_trampoline<F>(
  context: *mut RegisterContext,
  register_callback: *mut c_void,
) where
  F: FnMut(*mut RegisterContext),
{
  let register_callback = &mut *(register_callback as *mut F);
  register_callback(context);
}

pub fn get_trampoline<F>(_closure: &F) -> RegisterCallback
where
  F: FnMut(*mut RegisterCallback),
{
  register_endpoint_callback_trampoline::<F>
}

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
      register_endpoint_callback_trampoline,
      &mut callback2 as *mut _ as *mut c_void,
    )
  };
  println!("Result: {:?}", result);
}
