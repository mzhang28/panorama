#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn hello();
    }
}

fn hello() {}
