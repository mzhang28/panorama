use anyhow::Result;

#[cxx::bridge]
mod ffi {
    extern "Rust" {
        fn run_bridge();
    }
}

pub fn run() -> Result<()> {
    println!("Running daemon from Rust...");
    Ok(())
}

fn run_bridge() {
    let _ = run();
}
