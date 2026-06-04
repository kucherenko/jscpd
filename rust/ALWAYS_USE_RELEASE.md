# ⚠️ IMPORTANT: Always Use --release Flag!

## The Problem

If you run:
```bash
cargo run --bin cpd -- ../fixtures -r time
```

You get **SLOW** performance (~600ms) because you're using the **debug build**.

## The Solution

**ALWAYS** add `--release` flag:
```bash
cargo run --release --bin cpd -- ../fixtures -r time
```

This uses the **optimized release build** (~90ms).

---

## Debug vs Release Build

| Build Type | Command | Speed | Use Case |
|------------|---------|-------|----------|
| **Debug** | `cargo run` | ~600ms | Development & debugging |
| **Release** | `cargo run --release` | ~90ms | **Benchmarking & production** |

**Debug builds** are 7x slower because they:
- Include debug symbols
- Disable optimizations
- Add runtime checks
- Help with debugging

**Release builds** are fast because they:
- Remove debug symbols
- Enable all optimizations (-O3)
- Strip unnecessary code
- Optimize for speed

---

## Quick Reference

### ✅ CORRECT Commands

```bash
# Run with timing (fast)
cargo run --release --bin cpd -- ../fixtures -r time

# Run with console output (fast)
cargo run --release --bin cpd -- ../fixtures -r console,time

# Use compiled binary directly (fastest, no cargo overhead)
./target/release/cpd ../fixtures -r time
```

### ❌ WRONG Commands

```bash
# Missing --release (7x slower!)
cargo run --bin cpd -- ../fixtures -r time

# Correct path but wrong directory
cd /path/to/wrong/dir
cargo run --release --bin cpd -- ../fixtures -r time
```

---

## Performance Expectations

### With `--release` (correct):
```
First run:  ~130-150ms  (cold cache)
After:      ~85-100ms   (warm cache)
Average:    ~90-95ms
```

### Without `--release` (wrong):
```
All runs:   ~600ms  (7x slower!)
```

---

## Why You Saw 609ms

Your command:
```bash
cargo run cpd ../fixtures -r time
```

Problems:
1. ❌ Missing `--release` flag
2. ❌ Missing `--bin` specifier  
3. ✅ Path is correct

The build output shows:
```
Finished `dev` profile [unoptimized + debuginfo] target(s)
                ^^^
            This means DEBUG BUILD!
```

Corrected command:
```bash
cargo run --release --bin cpd -- ../fixtures -r time
```

The build output should show:
```
Finished `release` profile [optimized] target(s)
                 ^^^^^^^
            This means OPTIMIZED BUILD!
```

---

## Pro Tip: Use the Compiled Binary

After building once with `--release`, use the binary directly:

```bash
# Build once
cargo build --release --bin cpd

# Then use binary directly (no cargo overhead)
./target/release/cpd ../fixtures -r time
./target/release/cpd ../fixtures -r time
./target/release/cpd ../fixtures -r time
```

This is **slightly faster** because it skips cargo's startup overhead.

---

## Comparison

| Your Command | Time | Reason |
|-------------|------|--------|
| `cargo run cpd ../fixtures -r time` | **609ms** ❌ | Debug build (unoptimized) |
| `cargo run --release --bin cpd -- ../fixtures -r time` | **92ms** ✅ | Release build (optimized) |
| `./target/release/cpd ../fixtures -r time` | **88ms** ✅ | Direct binary (no cargo) |

**vs jscpd-rs**: 6ms (our goal, requires v2.0 architecture)

---

## Remember

🔴 **Debug** = Slow (600ms) = Development  
🟢 **Release** = Fast (90ms) = **Always use for benchmarking!**

**Always include `--release` when benchmarking or timing!**
