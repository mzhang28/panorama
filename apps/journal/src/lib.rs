#![no_std]

#[macro_use]
extern crate panorama_app_sdk;

#[no_mangle]
pub fn get_date_info() {
  panorama_app_sdk::get_current_time();
}
