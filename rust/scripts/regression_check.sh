#!/usr/bin/env bash
# regression_check.sh — verify detection output matches golden baseline
# Run after each Wave 2 and Wave 3 sub-change.
# Exit 0 = no regression. Exit 1 = regression or tool missing.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$(dirname "$SCRIPT_DIR")"
GOLDEN="$RUST_DIR/tests/golden.json"
FIXTURES_DIR="$RUST_DIR/../fixtures"
BINARY="$RUST_DIR/target/release/cpd"
REPORT="$RUST_DIR/report/jscpd-report.json"

# Check prerequisites
if ! command -v jq &>/dev/null; then
  echo "ERROR: jq is required but not found. Install with: brew install jq"
  exit 1
fi

if [[ ! -f "$BINARY" ]]; then
  echo "ERROR: release binary not found at $BINARY"
  echo "       Run: cargo build --release --bin cpd"
  exit 1
fi

if [[ ! -f "$GOLDEN" ]]; then
  echo "ERROR: golden baseline not found at $GOLDEN"
  echo "       Capture it before Wave 2: see Contract FR-013"
  exit 1
fi

# Run detection and produce report
"$BINARY" "$FIXTURES_DIR" -r json >/dev/null 2>&1

# Extract clone pairs (format + sorted file pair) from current run.
# Normalize source paths to "../fixtures/..." form so the gate is independent
# of whether the binary emits absolute or relative paths.
CURRENT="$(jq '[.duplicates[] | {files: ([.fragment_a.source_id, .fragment_b.source_id] | map(sub(".*/fixtures/"; "../fixtures/")) | sort), format: .format}] | sort_by(.format + .files[0] + .files[1])' "$REPORT")"

GOLDEN_COUNT="$(jq length "$GOLDEN")"
CURRENT_COUNT="$(echo "$CURRENT" | jq length)"

if [[ "$GOLDEN_COUNT" != "$CURRENT_COUNT" ]]; then
  echo "REGRESSION: clone count changed: golden=$GOLDEN_COUNT current=$CURRENT_COUNT"
  exit 1
fi

# Compare file-pair sets (order-insensitive, format-grouped)
MATCH="$(jq -n \
  --argjson golden "$(cat "$GOLDEN")" \
  --argjson current "$CURRENT" \
  '($golden | sort_by(.format + .files[0] + .files[1])) == ($current | sort_by(.format + .files[0] + .files[1]))')"

if [[ "$MATCH" != "true" ]]; then
  echo "REGRESSION: clone file-pair sets differ from golden baseline"
  echo "  Run: diff <(cat '$GOLDEN') <(echo '$CURRENT' | jq .)"
  exit 1
fi

echo "OK: $CURRENT_COUNT clone pairs match golden baseline"
