#!/usr/bin/env bash
set -e

echo "=== Testing Time Reporter ==="
echo ""

echo "Test 1: Time only (no other reporters)"
cargo run --release --bin cpd -- ../fixtures -r time 2>&1 | grep "time:" && echo "✓ PASS" || echo "✗ FAIL"
echo ""

echo "Test 2: Console + Time (time should be at end)"
OUTPUT=$(cargo run --release --bin cpd -- ../fixtures -r console,time 2>&1)
LAST_LINE=$(echo "$OUTPUT" | tail -1)
if echo "$LAST_LINE" | grep -q "time:"; then
    echo "✓ PASS - Time at end"
else
    echo "✗ FAIL - Time not at end"
    echo "Last line: $LAST_LINE"
fi
echo ""

echo "Test 3: Time + Console (order shouldn't matter)"
OUTPUT=$(cargo run --release --bin cpd -- ../fixtures -r time,console 2>&1)
LAST_LINE=$(echo "$OUTPUT" | tail -1)
if echo "$LAST_LINE" | grep -q "time:"; then
    echo "✓ PASS - Time at end regardless of order"
else
    echo "✗ FAIL - Time not at end"
fi
echo ""

echo "Test 4: No time flag (should not show time)"
OUTPUT=$(cargo run --release --bin cpd -- ../fixtures -r console 2>&1)
if echo "$OUTPUT" | grep -q "time:"; then
    echo "✗ FAIL - Time shown when not requested"
else
    echo "✓ PASS - Time not shown"
fi
echo ""

echo "Test 5: Default reporter (no -r flag, should not show time)"
OUTPUT=$(cargo run --release --bin cpd -- ../fixtures 2>&1)
if echo "$OUTPUT" | grep -q "time:"; then
    echo "✗ FAIL - Time shown when not requested"
else
    echo "✓ PASS - Time not shown"
fi
echo ""

echo "=== All Tests Complete ==="
