#!/usr/bin/env bash
set -euo pipefail

TARGET="${1:-promql}"

cd /workspace

if [ "$TARGET" = "promql" ]; then
    echo "=== Launching AFL++ Fuzzer for PromQL Parser & Translator ==="
    mkdir -p fuzz/findings/promql
    exec cargo afl fuzz -i fuzz/corpus/promql -o fuzz/findings/promql fuzz/target/release/fuzz_promql
elif [ "$TARGET" = "pql" ] || [ "$TARGET" = "panoramaql" ]; then
    echo "=== Launching AFL++ Fuzzer for PanoramaQL (PQL) Parser ==="
    mkdir -p fuzz/findings/pql
    exec cargo afl fuzz -i fuzz/corpus/pql -o fuzz/findings/pql fuzz/target/release/fuzz_pql
elif [ "$TARGET" = "all" ]; then
    echo "=== Launching Parallel AFL++ Fuzzers for PromQL and PQL ==="
    mkdir -p fuzz/findings/promql fuzz/findings/pql
    cargo afl fuzz -i fuzz/corpus/promql -o fuzz/findings/promql -M main_promql fuzz/target/release/fuzz_promql &
    cargo afl fuzz -i fuzz/corpus/pql -o fuzz/findings/pql -M main_pql fuzz/target/release/fuzz_pql &
    wait
else
    echo "Usage: $0 [promql|pql|all]"
    exit 1
fi
