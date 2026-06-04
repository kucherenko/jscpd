#!/usr/bin/env bash
# bench_compare.sh — compare cpd vs jscpd-rs on fixtures
#
# Usage:
#   scripts/bench_compare.sh [--runs N] [--warmup N]
#
# Defaults: 10 runs, 3 warmup.
# Must be run from the rust/ directory (or any directory inside the repo).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$(dirname "$SCRIPT_DIR")"
REPO_DIR="$(dirname "$RUST_DIR")"

CPD_BIN="$RUST_DIR/target/release/cpd"
JSCPD_BIN="$REPO_DIR/tmp/jscpd-rs/target/release/jscpd"
FIXTURES="$REPO_DIR/fixtures"

RUNS=10
WARMUP=3

while [[ $# -gt 0 ]]; do
    case "$1" in
        --runs)   RUNS="$2";   shift 2 ;;
        --warmup) WARMUP="$2"; shift 2 ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# ── prerequisite checks ──────────────────────────────────────────────────────

fail() { echo "ERROR: $*" >&2; exit 1; }

[[ -f "$CPD_BIN" ]]   || fail "cpd binary not found at $CPD_BIN — run: cargo build --release --bin cpd"
[[ -f "$JSCPD_BIN" ]] || fail "jscpd binary not found at $JSCPD_BIN — run: cargo build --release in tmp/jscpd-rs/"
[[ -d "$FIXTURES" ]]  || fail "fixtures directory not found at $FIXTURES"
command -v hyperfine &>/dev/null || fail "hyperfine not found — brew install hyperfine"

CPD_VER="$("$CPD_BIN" --version 2>/dev/null || echo 'cpd (unknown version)')"
JSCPD_VER="$("$JSCPD_BIN" --version 2>/dev/null || echo 'jscpd (unknown version)')"

echo "================================================================"
echo "  Clone detection benchmark"
echo "================================================================"
echo "  cpd      : $CPD_BIN"
echo "             $CPD_VER"
echo "  jscpd-rs : $JSCPD_BIN"
echo "             $JSCPD_VER"
echo "  fixtures : $FIXTURES"
echo "  runs     : $RUNS  (warmup: $WARMUP)"
echo "================================================================"
echo ""
echo "NOTE: Both binaries run with -r silent / --reporters silent so"
echo "      wall-clock time covers walk + read + tokenize + detect."
echo "      The 'time: Xms' jscpd-rs prints internally excludes I/O."
echo ""

hyperfine \
    --warmup "$WARMUP" \
    --runs   "$RUNS" \
    --shell  none \
    --command-name "cpd (ours)"      "$CPD_BIN $FIXTURES -r silent" \
    --command-name "jscpd-rs"        "$JSCPD_BIN $FIXTURES --reporters silent"
