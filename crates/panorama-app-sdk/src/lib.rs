use std::ffi::CString;

pub mod sys {
  use std::ffi::c_char;

  extern "C" {
    pub fn register_endpoint(url: *const c_char);
  }
}

pub fn register_endpoint(url: impl AsRef<str>) {
  let url = CString::new(url.as_ref()).unwrap();
  let result = unsafe { sys::register_endpoint(url.into_raw()) };
  println!("Result: {:?}", result);
}
