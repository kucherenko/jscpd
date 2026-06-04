#!/usr/bin/env bash
set -e

echo "=== Building release binary ==="
cargo build --release --bin cpd

echo ""
echo "=== Running benchmark (10 iterations) ==="
echo ""

for i in {1..10}; do
    echo -n "Run $i: "
    cargo run --release --bin cpd -- ../fixtures -r time 2>&1 | grep "time:"
done

echo ""
echo "=== Comparison with jscpd-rs ==="
if [ -f "../tmp/jscpd-rs/target/release/jscpd" ]; then
    echo -n "jscpd-rs: "
    ../tmp/jscpd-rs/target/release/jscpd ../fixtures -r time 2>&1 | grep "time:" || echo "(time reporter not available)"
else
    echo "jscpd-rs binary not found at ../tmp/jscpd-rs/target/release/jscpd"
fi

echo ""
echo "=== Summary ==="
echo "Run with --release flag for optimized performance:"
echo "  cargo run --release --bin cpd -- ../fixtures -r time"
echo ""
echo "Or use the compiled binary directly:"
echo "  ./target/release/cpd ../fixtures -r time"
