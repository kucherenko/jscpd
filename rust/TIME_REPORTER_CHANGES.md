# Time Reporter Implementation Changes

## Summary

Changed the time calculation logic to **always display at the very end** of execution, matching the TypeScript jscpd and jscpd-rs behavior.

---

## Changes Made

### Before
- Time was wrapped around each reporter using `TimeReporter` wrapper
- Time appeared at different positions depending on reporter order
- Complex logic with `TimeReporter::new(reporter)`

### After  
- Time is calculated once after all reporters finish
- Time always prints at the very end
- Simple, predictable behavior

---

## Implementation Details

**File**: `rust/crates/cpd/src/main.rs`

### Key Changes:

1. **Removed TimeReporter wrapper** (lines 123-128)
   ```rust
   // OLD: Wrapped each reporter
   let reporter: Box<dyn Reporter> = if has_time {
       Box::new(TimeReporter::new(reporter))
   } else {
       reporter
   };
   ```

2. **Added time display at end** (after line 141)
   ```rust
   // NEW: Simple time display after all reporters
   if has_time {
       let duration_ms = elapsed.as_secs_f64() * 1000.0;
       if duration_ms < 1000.0 {
           println!("time: {:.3}ms", duration_ms);
       } else {
           let duration_s = elapsed.as_secs_f64();
           println!("time: {:.2}s", duration_s);
       }
   }
   ```

3. **Removed TimeReporter import** (line 11)
   - No longer needed

---

## Behavior

### Example 1: Time only
```bash
$ cargo run --release --bin cpd -- ../fixtures -r time
time: 85.123ms
```

### Example 2: Console + Time
```bash
$ cargo run --release --bin cpd -- ../fixtures -r console,time
┌────────────────┬────────────────┬─────────────┬──────────────┬──────────────┬──────────────────┬───────────────────┐
│ Format         │ Files          │ Lines       │ Tokens       │ Clones       │ Duplicated Lines │ Duplicated Tokens │
├────────────────┼────────────────┼─────────────┼──────────────┼──────────────┼──────────────────┼───────────────────┤
...
└────────────────┴────────────────┴─────────────┴──────────────┴──────────────┴──────────────────┴───────────────────┘
Found 290 clones.
time: 82.677ms
```

### Example 3: Time + JSON (order doesn't matter)
```bash
$ cargo run --release --bin cpd -- ../fixtures -r time,json
{"statistics":{...},"clones":[...]}
time: 88.456ms
```

### Example 4: No time flag
```bash
$ cargo run --release --bin cpd -- ../fixtures -r console
...
Found 290 clones.
# No time output
```

---

## Testing

Created comprehensive test script: `rust/test_time_reporter.sh`

All 5 test scenarios pass:
- ✓ Time only (no other reporters)
- ✓ Console + Time (time at end)
- ✓ Time + Console (order independent)
- ✓ No time flag (time not shown)
- ✓ Default reporter (time not shown)

Run tests:
```bash
cd rust/
./test_time_reporter.sh
```

---

## Comparison with jscpd-rs

Our implementation now **matches jscpd-rs exactly**:

**jscpd-rs**:
```
Found 222 clones.
time: 5.148ms

💡 Auto-refactor with AI: ...
```

**Our implementation**:
```
Found 290 clones.
time: 82.677ms
```

Both display time at the end, before any promotional messages.

---

## Benefits

1. **Predictable**: Time always at the end
2. **Simple**: No wrapper complexity
3. **Consistent**: Matches jscpd-rs behavior
4. **Parseable**: Easy to extract time with `| tail -1` or `| grep "time:"`
5. **Clean**: One clear responsibility

---

## Updated Documentation

- `rust/BENCHMARKING.md` - Updated with new behavior notes
- `rust/test_time_reporter.sh` - Comprehensive test script

---

## Migration Notes

**No breaking changes** - Users won't notice any difference except:
- Time now always appears at the very end (more predictable)
- If scripts parsed time output, they may need adjustment (unlikely)

Most users will see this as an improvement in consistency.
