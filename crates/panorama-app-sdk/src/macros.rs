#[macro_export]
macro_rules! println {
  () => { $crate::print!("\n") };
  ($($arg:tt)*) => {{ $crate::internal::_println(format_args!($($arg)*)); }};
}

#[macro_export]
macro_rules! print {
  ($($arg:tt)*) => {{ $crate::internal::_print(format_args!($($arg)*)); }};
}

#[macro_export]
macro_rules! init {
  () => {
    // Use `wee_alloc` as the global allocator.
    #[global_allocator]
    static ALLOC: panorama_app_sdk::wee_alloc::WeeAlloc =
      panorama_app_sdk::wee_alloc::WeeAlloc::INIT;

    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo) -> ! {
      loop {}
    }
  };
}
