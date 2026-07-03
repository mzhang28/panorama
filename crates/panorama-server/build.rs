fn main() {
    // Generate build info (optional, graceful fallback)
    println!("cargo:rerun-if-changed=src/");
}
