use core::{
  fmt,
  sync::atomic::{AtomicBool, Ordering},
};

use alloc::{
  ffi::CString,
  string::String,
  vec::{self, Vec},
};

#[doc(hidden)]
pub fn _println(args: fmt::Arguments<'_>) {
  _print(format_args!("{}\n", args))
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments<'_>) {
  // print_to(args, stdout, "stdout");
  // TODO: Lock prints?
  let mut buf = String::new();
  alloc::fmt::write(&mut buf, args);
  let cs = CString::new(buf).unwrap();
  let len = cs.as_bytes_with_nul().len() as u64;
  let cptr = cs.into_raw();
  unsafe { crate::sys::_print(len, cptr) };
}
