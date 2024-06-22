use std::ffi::CString;

pub mod sys {
  use std::ffi::c_char;

  extern "C" {
    pub fn register_endpoint(url_len: u64, url: *const c_char);
  }
}

pub fn register_endpoint(url: impl AsRef<str>) {
  let url = url.as_ref();
  let url_cstr = CString::new(url).unwrap();
  let result =
    unsafe { sys::register_endpoint(url.len() as u64, url_cstr.into_raw()) };
  println!("Result: {:?}", result);
}
