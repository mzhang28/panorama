#![no_std]

pub extern crate wee_alloc;

#[macro_use]
extern crate alloc;

// Reference for FFI closures
// https://adventures.michaelfbryan.com/posts/rust-closures-in-ffi/

#[macro_use]
pub mod macros;
pub mod internal;
pub mod sys;

pub mod prelude {
  // pub use crate::macros::println;
}
