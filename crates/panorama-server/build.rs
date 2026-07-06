fn main() {
  println!("cargo:rerun-if-changed=src/");
  println!("cargo::rustc-check-cfg=cfg(frontend_embedded)");

  // Detect if the frontend has been built. When present, the server embeds
  // the production frontend build and serves it as an SPA fallback.
  let frontend_dist = std::path::Path::new("../../frontend/dist/index.html");
  if frontend_dist.exists() {
    println!("cargo:rustc-cfg=frontend_embedded");
    println!("cargo:rerun-if-changed=../../frontend/dist/");
  }
}
