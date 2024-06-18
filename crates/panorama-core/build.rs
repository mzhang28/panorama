fn main() {
  println!("cargo:rerun-if-changed=../../apps");
  println!("cargo:rerun-if-changed=migrations");
}
