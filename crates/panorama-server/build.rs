fn main() {
  println!("cargo:rerun-if-changed=src/");
  println!("cargo::rustc-check-cfg=cfg(frontend_embedded)");

  // Use CARGO_MANIFEST_DIR to determine the absolute path to frontend/dist
  let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
  let dist_dir = std::path::Path::new(&manifest_dir).join("../../frontend/dist");
  let index_html = dist_dir.join("index.html");

  println!("cargo:rerun-if-changed={}", dist_dir.display());
  println!("cargo:rerun-if-changed={}", index_html.display());

  // Detect if the frontend has been built. When present, the server embeds
  // the production frontend build and serves it as an SPA fallback.
  if index_html.exists() {
    println!("cargo:rustc-cfg=frontend_embedded");
    walk_and_emit(&dist_dir);
  }
}

fn walk_and_emit(dir: &std::path::Path) {
  if let Ok(entries) = std::fs::read_dir(dir) {
    for entry in entries.flatten() {
      let path = entry.path();
      if path.is_dir() {
        walk_and_emit(&path);
      } else if let Some(path_str) = path.to_str() {
        println!("cargo:rerun-if-changed={}", path_str);
      }
    }
  }
}
