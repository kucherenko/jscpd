#!/usr/bin/env bash
# publish-crates.sh — publish all cpd crates to crates.io in dependency order
#
# Usage:
#   scripts/publish-crates.sh [--dry-run] [--token <TOKEN>]
#
# Must be run from the rust/ directory.
#
# Steps:
#   1. Sync version from package.json to all Cargo.toml files
#   2. Temporarily remove publish = false from all crates (working tree only)
#   3. Publish crates in dependency order with index-wait between each
#   4. Restore working tree (git checkout)
#
# Environment:
#   CARGO_REGISTRY_TOKEN — crates.io API token (or pass --token)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRY_RUN=""
TOKEN_FLAG=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --token)
      TOKEN_FLAG="--token"
      export CARGO_REGISTRY_TOKEN="$2"
      shift 2
      ;;
    --help|-h)
      echo "Usage: scripts/publish-crates.sh [--dry-run] [--token <TOKEN>]"
      echo ""
      echo "Publish all cpd crates to crates.io in dependency order."
      echo ""
      echo "Options:"
      echo "  --dry-run    Show what would be published without actually publishing"
      echo "  --token      Pass crates.io API token (alternative to CARGO_REGISTRY_TOKEN env)"
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

cd "$RUST_DIR"

PUBLISH_ORDER=("cpd-core" "cpd-tokenizer" "cpd-reporter" "cpd-finder" "jscpd")
CRATE_VERSIONS=("0.1.0" "0.1.0" "0.1.0" "0.1.0" "5.0.0")

WAIT_SECONDS=30
WAIT_MAX_ATTEMPTS=30

log() {
  echo ":: publish-crates :: $*"
}

log "Working directory: $RUST_DIR"
log "Publish order: ${PUBLISH_ORDER[*]}"

log "Step 1/4: Syncing version to Cargo.toml files and regenerating lock file"
if [ -z "$DRY_RUN" ]; then
  node scripts/sync-version.mjs
  cargo generate-lockfile --quiet
  log "  Cargo.lock regenerated"
else
  log "  [dry-run] Would run: node scripts/sync-version.mjs && cargo generate-lockfile"
fi

log "Step 2/4: Removing publish = false from all Cargo.toml files"
CRATE_TOML_FILES=(
  crates/cpd-core/Cargo.toml
  crates/cpd-tokenizer/Cargo.toml
  crates/cpd-finder/Cargo.toml
  crates/cpd-reporter/Cargo.toml
  crates/cpd/Cargo.toml
)

if [ -z "$DRY_RUN" ]; then
  for f in "${CRATE_TOML_FILES[@]}"; do
    if grep -q '^publish = false' "$f"; then
      if sed --version 2>/dev/null | grep -q GNU; then
        sed -i '/^publish = false$/d' "$f"
      else
        sed -i '' '/^publish = false$/d' "$f"
      fi
      log "  Removed publish = false from $f"
    else
      log "  No publish = false in $f (already removed?)"
    fi
  done
else
  for f in "${CRATE_TOML_FILES[@]}"; do
    log "  [dry-run] Would remove publish = false from $f"
  done
fi

log "Step 3/4: Publishing crates in dependency order"

for i in "${!PUBLISH_ORDER[@]}"; do
  crate="${PUBLISH_ORDER[$i]}"
  crate_version="${CRATE_VERSIONS[$i]}"
  log "  Publishing ${crate}@${crate_version}..."

  if [ -z "$DRY_RUN" ]; then
    cargo publish -p "$crate" --allow-dirty $TOKEN_FLAG
  else
    log "  [dry-run] Would run: cargo publish -p $crate --allow-dirty"
  fi

  if [ "$crate" != "${PUBLISH_ORDER[-1]}" ]; then
    log "  Waiting for ${crate}@${crate_version} to appear on crates.io index..."
    attempt=0
    while [ $attempt -lt $WAIT_MAX_ATTEMPTS ]; do
      attempt=$((attempt + 1))
      if [ -z "$DRY_RUN" ]; then
        if cargo info "${crate}@${crate_version}" >/dev/null 2>&1; then
          log "  ${crate}@${crate_version} is available on crates.io (attempt $attempt/$WAIT_MAX_ATTEMPTS)"
          break
        fi
      else
        log "  [dry-run] Would poll: cargo info ${crate}@${crate_version}"
        break
      fi
      log "  Attempt $attempt/$WAIT_MAX_ATTEMPTS: not yet indexed, waiting ${WAIT_SECONDS}s..."
      sleep $WAIT_SECONDS
    done

    if [ $attempt -ge $WAIT_MAX_ATTEMPTS ]; then
      log "  WARNING: ${crate} may not be indexed yet after $WAIT_MAX_ATTEMPTS attempts"
      log "  Continuing anyway — dependent crates may fail to resolve"
    fi
  fi
done

log "Step 4/4: Restoring working tree"
if [ -z "$DRY_RUN" ]; then
  git checkout -- .
  log "  Working tree restored"
else
  log "  [dry-run] Would run: git checkout -- ."
fi

log "Done! All crates published to crates.io."
if [ -z "$DRY_RUN" ]; then
  log "Verify: cargo info jscpd@${CRATE_VERSIONS[-1]}"
fi