use afl::fuzz;
use panorama_core::query::parse_query;

fn main() {
  fuzz!(|data: &[u8]| {
    if let Ok(input) = std::str::from_utf8(data) {
      // Fuzz PanoramaQL (PQL) parser
      let _ = parse_query(input);
    }
  });
}
