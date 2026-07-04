use afl::fuzz;
use panorama_app_grafana::promql::{parse, translate, MetricRegistry};

fn main() {
    let registry = MetricRegistry::default();

    fuzz!(|data: &[u8]| {
        if let Ok(input) = std::str::from_utf8(data) {
            // Fuzz PromQL parser
            if let Ok(expr) = parse(input) {
                // If parsing succeeds, fuzz PromQL -> PQL translator
                let _ = translate(&expr, &registry, "2026-07-04T00:00:00Z", "2026-07-04T05:00:00Z");
            }
        }
    });
}
