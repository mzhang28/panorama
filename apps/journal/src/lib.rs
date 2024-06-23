#![no_std]

#[macro_use]
extern crate panorama_app_sdk;

use panorama_app_sdk::prelude::*;

panorama_app_sdk::init!();

#[no_mangle]
pub fn install() -> i32 {
  println!("SHIET");
  // panorama_app_sdk::register_endpoint("/get_todays_date");
  123
}
