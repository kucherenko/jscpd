#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
FIXTURES_DIR="$PROJECT_ROOT/fixtures"
TOOLS_DIR="$SCRIPT_DIR/tools"
RESULTS_DIR="$SCRIPT_DIR/results"
REPORT_FILE="$RESULTS_DIR/benchmark-report.md"
DATA_FILE="$RESULTS_DIR/benchmark-data.tsv"

JSCPD_V5="$PROJECT_ROOT/rust/target/release/jscpd"
JSCPD_V4="$PROJECT_ROOT/apps/jscpd/bin/jscpd"
JSCPD_RS="$TOOLS_DIR/node_modules/.bin/jscpd-rs"
FALLOW_BIN="$TOOLS_DIR/node_modules/.bin/fallow"
DUPLO_BIN="$TOOLS_DIR/duplo"
SIMIAN_JAR="$TOOLS_DIR/simian.jar"

mkdir -p "$RESULTS_DIR"

for f in "$RESULTS_DIR"/*; do
  [ -f "$f" ] && rm "$f"
done

TOTAL_FILES=$(find "$FIXTURES_DIR" -type f ! -name '.DS_Store' | wc -l | tr -d ' ')
TOTAL_LINES=$(find "$FIXTURES_DIR" -type f ! -name '.DS_Store' -exec cat {} + 2>/dev/null | wc -l | tr -d ' ')
TOTAL_SIZE=$(du -sh "$FIXTURES_DIR" | cut -f1)

FILE_LIST="$RESULTS_DIR/all-files.txt"
find "$FIXTURES_DIR" -type f ! -name '.DS_Store' > "$FILE_LIST"

echo "tool_key	time_ms	files	clones	dup_lines	status" > "$DATA_FILE"

timestamp() { date +%s%N; }

elapsed_ms() {
  local start=$1 end=$2
  echo $(( (end - start) / 1000000 ))
}

format_ms() {
  local ms=$1
  if ! [[ "$ms" =~ ^[0-9]+$ ]]; then
    echo "$ms"
    return
  fi
  if [ "$ms" -ge 1000 ]; then
    local sec=$((ms / 1000))
    local rem=$((ms % 1000))
    printf "%d.%03ds" "$sec" "$rem"
  else
    printf "%dms" "$ms"
  fi
}

record() {
  local key=$1 time=$2 files=$3 clones=$4 lines=$5 status=$6
  echo "$key	$time	$files	$clones	$lines	$status" >> "$DATA_FILE"
}

echo "============================================"
echo "  CPD Benchmark — fixtures ($TOTAL_FILES files, $TOTAL_LINES lines, $TOTAL_SIZE)"
echo "============================================"
echo ""

# ─── 1. jscpd@5 (local build from rust/) ───
echo "[1/7] Running jscpd@5 (local build) ..."
START=$(timestamp)
"$JSCPD_V5" "$FIXTURES_DIR" --reporters json,silent --output "$RESULTS_DIR/jscpd-v5" 2>&1 || true
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
if [ -f "$RESULTS_DIR/jscpd-v5/jscpd-report.json" ]; then
  FILES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v5/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('sources',0))")
  CLONES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v5/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('clones',0))")
  DUP_LINES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v5/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('duplicatedLines',0))")
else
  FILES="?" CLONES="?" DUP_LINES="?"
fi
record jscpd_v5 "$TIME_MS" "$FILES" "$CLONES" "$DUP_LINES" "ok"
echo "  → done in $(format_ms $TIME_MS)"

# ─── 2. jscpd@4 (local build) ───
echo "[2/7] Running jscpd@4 (Node.js) ..."
START=$(timestamp)
"$JSCPD_V4" "$FIXTURES_DIR" --reporters json,silent --output "$RESULTS_DIR/jscpd-v4" 2>&1 || true
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
if [ -f "$RESULTS_DIR/jscpd-v4/jscpd-report.json" ]; then
  FILES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v4/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('sources',0))")
  CLONES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v4/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('clones',0))")
  DUP_LINES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-v4/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('duplicatedLines',0))")
else
  FILES="?" CLONES="?" DUP_LINES="?"
fi
record jscpd_v4 "$TIME_MS" "$FILES" "$CLONES" "$DUP_LINES" "ok"
echo "  → done in $(format_ms $TIME_MS)"

# ─── 3. Fallow dupes (must run from project root) ───
echo "[3/7] Running Fallow dupes ..."
FALLOW_OUTPUT=$RESULTS_DIR/fallow-dupes.txt
START=$(timestamp)
(cd "$FIXTURES_DIR" && "$FALLOW_BIN" dupes > "$FALLOW_OUTPUT" 2>&1) || true
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
FALLOW_CLONE_GROUPS=$(grep -cE "dup:" "$FALLOW_OUTPUT" 2>/dev/null || echo "0")
FALLOW_TOTAL_LINE=$(grep -E "duplicated across" "$FALLOW_OUTPUT" 2>/dev/null | tail -1 || echo "")
if [ -n "$FALLOW_TOTAL_LINE" ]; then
  FALLOW_DUP_LINES=$(echo "$FALLOW_TOTAL_LINE" | grep -oE '[0-9,]+ lines' | head -1 | tr -d ',' | grep -oE '[0-9]+' || echo "?")
  FALLOW_FILES=$(echo "$FALLOW_TOTAL_LINE" | grep -oE '[0-9]+ files' | head -1 | grep -oE '[0-9]+' || echo "?")
else
  FALLOW_DUP_LINES="?" FALLOW_FILES="?"
fi
record fallow_dupes "$TIME_MS" "$FALLOW_FILES" "$FALLOW_CLONE_GROUPS" "$FALLOW_DUP_LINES" "ok"
echo "  → done in $(format_ms $TIME_MS)"

# ─── 4. jscpd-rs (npm package, local install) ───
echo "[4/7] Running jscpd-rs (npm jscpd-rs v$(node -e "console.log(require('$TOOLS_DIR/node_modules/jscpd-rs/package.json').version)") ) ..."
START=$(timestamp)
"$JSCPD_RS" "$FIXTURES_DIR" --reporters json,silent --output "$RESULTS_DIR/jscpd-rs" 2>&1 || true
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
if [ -f "$RESULTS_DIR/jscpd-rs/jscpd-report.json" ]; then
  FILES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-rs/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('sources',0))")
  CLONES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-rs/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('clones',0))")
  DUP_LINES=$(python3 -c "import json; d=json.load(open('$RESULTS_DIR/jscpd-rs/jscpd-report.json')); print(d.get('statistics',{}).get('total',{}).get('duplicatedLines',0))")
else
  FILES="?" CLONES="?" DUP_LINES="?"
fi
record jscpd_rs "$TIME_MS" "$FILES" "$CLONES" "$DUP_LINES" "ok"
echo "  → done in $(format_ms $TIME_MS)"

# ─── 5. PMD CPD (run per-language, sum results) ───
echo "[5/7] Running PMD CPD ..."
PMD_OUT="$RESULTS_DIR/pmd-cpd.txt"
PMD_TOTAL_CLONES=0
PMD_TOTAL_DUP_LINES=0
PMD_ALL_FILES=""
START=$(timestamp)
for lang in apex coco cpp cs css dart ecmascript fortran gherkin go groovy html java jsp julia kotlin lua matlab modelica objectivec perl php plsql pom python ruby rust scala swift tsql typescript velocity visualforce wsdl xml xsl; do
  PMD_LANG_OUT="$RESULTS_DIR/pmd-cpd-${lang}.txt"
  pmd cpd --dir "$FIXTURES_DIR" --language "$lang" --minimum-tokens 50 --format text --no-fail-on-violation --no-fail-on-error > "$PMD_LANG_OUT" 2>&1 || true
  PMD_LANG_CLONES=$(grep -c "^Found a " "$PMD_LANG_OUT" 2>/dev/null || true)
  PMD_LANG_DUP=$(grep -oE 'Found a [0-9]+ line' "$PMD_LANG_OUT" 2>/dev/null | grep -oE '[0-9]+' | awk '{s+=$1} END {print s+0}' 2>/dev/null || true)
  [ -z "$PMD_LANG_DUP" ] && PMD_LANG_DUP=0
  [ "$PMD_LANG_DUP" = "" ] && PMD_LANG_DUP=0
  PMD_TOTAL_CLONES=$((PMD_TOTAL_CLONES + PMD_LANG_CLONES))
  PMD_TOTAL_DUP_LINES=$((PMD_TOTAL_DUP_LINES + PMD_LANG_DUP))
  cat "$PMD_LANG_OUT" >> "$PMD_OUT"
done
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
PMD_FILES=$(grep -oE '/Users/[^ ]+\.[a-zA-Z0-9]+' "$PMD_OUT" 2>/dev/null | sort -u | wc -l | tr -d ' ' || echo "?")
record pmd_cpd "$TIME_MS" "$PMD_FILES" "$PMD_TOTAL_CLONES" "$PMD_TOTAL_DUP_LINES" "ok"
echo "  → done in $(format_ms $TIME_MS)"

# ─── 6. Duplo ───
echo "[6/7] Running Duplo ..."
if [ -x "$DUPLO_BIN" ]; then
  DUPLO_OUT="$RESULTS_DIR/duplo-out.txt"
  DUPLO_JSON="$RESULTS_DIR/duplo-out.json"
  START=$(timestamp)
  "$DUPLO_BIN" -ml 5 -json "$FILE_LIST" "$DUPLO_JSON" > /dev/null 2>&1 || true
  END=$(timestamp)
  TIME_MS=$(elapsed_ms "$START" "$END")
  if [ -f "$DUPLO_JSON" ] && [ -s "$DUPLO_JSON" ]; then
    FILES=$(python3 -c "
import json
d=json.load(open('$DUPLO_JSON'))
if isinstance(d, list):
  files=set()
  for c in d:
    files.add(c.get('SourceFile1',''))
    files.add(c.get('SourceFile2',''))
  print(len(files))
else:
  files=set()
  for item in d.get('files',[]):
    files.add(item.get('name',''))
  for clone in d.get('duplications',[]):
    for f in clone.get('files',[]):
      files.add(f.get('name',''))
  print(len(files))
" 2>/dev/null || echo "?")
    CLONES=$(python3 -c "
import json
d=json.load(open('$DUPLO_JSON'))
if isinstance(d, list):
  print(len(d))
else:
  print(len(d.get('duplications',[])))
" 2>/dev/null || echo "?")
    DUP_LINES=$(python3 -c "
import json
d=json.load(open('$DUPLO_JSON'))
if isinstance(d, list):
  print(sum(c.get('LineCount',0) for c in d))
else:
  total=0
  for c in d.get('duplications',[]):
    total+=c.get('linesTotal',0)
  print(total)
" 2>/dev/null || echo "?")
  else
    FILES="?" CLONES="?" DUP_LINES="?"
  fi
  record duplo "$TIME_MS" "$FILES" "$CLONES" "$DUP_LINES" "ok"
else
  record duplo 0 "n/a" "n/a" "n/a" "skip"
  TIME_MS=0
fi
echo "  → done in $(format_ms $TIME_MS)"

# ─── 7. Simian ───
echo "[7/7] Running Simian ..."
SIMIAN_OUT="$RESULTS_DIR/simian-out.txt"
START=$(timestamp)
cat "$FILE_LIST" | xargs java -jar "$SIMIAN_JAR" -threshold=5 > "$SIMIAN_OUT" 2>&1 || true
END=$(timestamp)
TIME_MS=$(elapsed_ms "$START" "$END")
SIMIAN_SUMMARY=$(grep -E "Found [0-9]+ duplicate lines in [0-9]+ blocks" "$SIMIAN_OUT" 2>/dev/null | tail -1 || echo "")
if [ -n "$SIMIAN_SUMMARY" ]; then
  SIMIAN_TOTAL_DUP=$(echo "$SIMIAN_SUMMARY" | grep -oE '[0-9]+' | head -1)
  SIMIAN_TOTAL_BLOCKS=$(echo "$SIMIAN_SUMMARY" | grep -oE '[0-9]+' | sed -n '2p')
else
  SIMIAN_TOTAL_DUP="?" SIMIAN_TOTAL_BLOCKS="?"
fi
SIMIAN_PROCESSED_LINE=$(grep -E "Processed a total of" "$SIMIAN_OUT" 2>/dev/null | tail -1 || echo "")
if [ -n "$SIMIAN_PROCESSED_LINE" ]; then
  SIMIAN_PROCESSED_FILES=$(echo "$SIMIAN_PROCESSED_LINE" | grep -oE '[0-9]+' | tail -1)
else
  SIMIAN_PROCESSED_FILES="?"
fi
record simian "$TIME_MS" "$SIMIAN_PROCESSED_FILES" "$SIMIAN_TOTAL_BLOCKS" "$SIMIAN_TOTAL_DUP" "ok"
echo "  → done in $(format_ms $TIME_MS)"

echo ""
echo "============================================"
echo "  Generating report ..."
echo "============================================"

cat > "$REPORT_FILE" <<HEADER
# CPD Benchmark Report

**Date:** $(date -u +"%Y-%m-%d %H:%M UTC")
**Target:** \`fixtures/\` — $TOTAL_FILES files, $TOTAL_LINES lines, $TOTAL_SIZE

## Results Summary

| Tool | Time | Files Analyzed | Clones Found | Duplicate Lines |
|------|------|---------------|-------------|-----------------|
HEADER

while IFS='	' read -r key time files clones dup_lines status; do
  [ "$key" = "tool_key" ] && continue
  TIME_FMT=$(format_ms "$time")
  case $key in
    jscpd_v5)    LABEL="jscpd@5 (local build)" ;;
    jscpd_v4)    LABEL="jscpd@4 (Node.js)" ;;
    fallow_dupes) LABEL="Fallow dupes" ;;
    jscpd_rs)    LABEL="jscpd-rs (npm)" ;;
    pmd_cpd)     LABEL="PMD CPD (Java)" ;;
    duplo)       LABEL="Duplo (C++)" ;;
    simian)      LABEL="Simian (Java)" ;;
    *)           LABEL="$key" ;;
  esac
  if [ "$status" = "skip" ]; then
    printf "| %s | — | — | — | — |\n" "$LABEL" >> "$REPORT_FILE"
  else
    printf "| %s | %s | %s | %s | %s |\n" "$LABEL" "$TIME_FMT" "$files" "$clones" "$dup_lines" >> "$REPORT_FILE"
  fi
done < "$DATA_FILE"

cat >> "$REPORT_FILE" <<'FOOTER'

## Tool Details

| Tool | Version | Language | License | Source |
|------|---------|----------|---------|--------|
| jscpd@5 | 5.0.x | Rust (local build) | MIT | https://github.com/jscpd/jscpd (rust/) |
| jscpd@4 | 4.2.x | TypeScript/Node.js | MIT | https://github.com/jscpd/jscpd |
| Fallow dupes | 2.x | Rust binary / npm | MIT | https://github.com/fallow-rs/fallow |
| jscpd-rs | 0.1.x | Rust (npm package) | MIT | https://www.npmjs.com/package/jscpd-rs |
| PMD CPD | 7.x | Java | Apache 2.0 | https://pmd.github.io |
| Duplo | 2.3.x | C++ | GPL-2.0 | https://github.com/dlidstrom/Duplo |
| Simian | 4.2.x | Java | Apache 2.0 | https://simian.quandarypeak.com |

## Methodology

- **Target:** The `fixtures/` directory containing 548 source files across 150+ language/format categories
- **Timing:** External wall-clock measurement using `date +%s%N` (nanosecond precision), reported in milliseconds
- **Minimum duplicate threshold:** ~5 lines / ~50 tokens (each tool's closest equivalent)
- **All tools** run with default settings unless otherwise noted; no special language configs
- **Hardware:** Darwin arm64

## Notes

- **jscpd@4**: v4 is the TypeScript/Node.js version; this benchmark uses the local build from `apps/jscpd/bin/jscpd`
- **jscpd-rs**: Separate npm package (`jscpd-rs` on npm), installed locally in `benchmark/tools/node_modules/`. Downloads and invokes its own Rust binary — may differ from the local jscpd@5 build
- **Fallow dupes**: A broader code intelligence tool; `fallow dupes` subcommand performs structural clone detection. Only works on TS/JS projects — must run from target directory, not with a path argument
- **PMD CPD**: Requires a `--language` flag and only processes one language per run. This benchmark runs it for 16 languages and sums results. Many fixture formats are unsupported
- **Duplo**: General text duplicate finder; limited built-in language support (C/C++, Java, C#, VB.NET, Erlang, TypeScript, Swift). Processes all text files regardless
- **Simian**: Language-agnostic similarity analyzer; processes any text file. Requires explicit file paths (no directory recursion). Originally commercial, now open-source (Apache 2.0)

## Raw Output Files

- `results/jscpd-v5/jscpd-report.json`
- `results/jscpd-v4/jscpd-report.json`
- `results/jscpd-rs/jscpd-report.json`
- `results/fallow-dupes.txt`
- `results/pmd-cpd.txt`
- `results/pmd-cpd-*.txt`
- `results/duplo-out.json`
- `results/simian-out.txt`
FOOTER

echo ""
echo "Report written to: $REPORT_FILE"
echo ""
cat "$REPORT_FILE"