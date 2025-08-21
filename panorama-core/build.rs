fn main() {
    cxx_build::bridge("src/lib.rs")
        .std("c++20")
        .compile("panorama_core");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
