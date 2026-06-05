#!/usr/bin/env bash
# publish-npm.sh — build, package, and publish all cpd npm packages
#
# Usage:
#   scripts/publish-npm.sh [--dry-run] [--target <target>] [--all] [--provenance]
#
# Modes:
#   --all            Build and publish all 6 platform packages, then publish main
#   --target <key>   Build and publish a single platform package
#   (default)        Build and publish for the current platform only
#
# Targets: linux-x64-gnu, linux-arm64-gnu, linux-x64-musl,
#          darwin-arm64, darwin-x64, windows-x64-msvc
#
# With --all, builds that fail (e.g. missing cross-compiler) are skipped.
#   Already-published platform packages are also skipped.
#   The main cpd package is only published if all 6 are on npm.
#
# Environment:
#   NPM_TOKEN — npm auth token (or already logged in via npm login)

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRY_RUN=""
TARGET_FLAG=""
ALL_TARGETS=""
PROVENANCE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --target)
      TARGET_FLAG="$2"
      shift 2
      ;;
    --all)
      ALL_TARGETS=1
      shift
      ;;
    --provenance)
      PROVENANCE=1
      shift
      ;;
    --help|-h)
      echo "Usage: scripts/publish-npm.sh [--dry-run] [--target <target>] [--all] [--provenance]"
      echo ""
      echo "Build and publish cpd npm packages."
      echo ""
      echo "Options:"
      echo "  --all              Build and publish all 6 platform packages"
      echo "  --target <target>  Build and publish a single platform package"
      echo "  --dry-run          Show what would be published without actually publishing"
      echo "  --provenance       Add npm provenance (requires GitHub Actions OIDC)"
      echo ""
      echo "Available targets: linux-x64-gnu, linux-arm64-gnu, linux-x64-musl,"
      echo "                   darwin-arm64, darwin-x64, windows-x64-msvc"
      echo ""
      echo "Note: --all will skip targets that fail to build (missing cross-compiler)."
      echo "  Already-published packages are also skipped."
      echo "  The main cpd package is only published if all 6 are on npm."
      exit 0
      ;;
    *)
      echo "Unknown option: $1" >&2
      exit 1
      ;;
  esac
done

cd "$RUST_DIR"

VERSION=$(node -p "require('./package.json').version")
ALL_TARGET_KEYS="linux-x64-gnu linux-arm64-gnu linux-x64-musl darwin-arm64 darwin-x64 windows-x64-msvc"

PROVENANCE_FLAG=""
if [ -n "$PROVENANCE" ]; then
  PROVENANCE_FLAG="--provenance"
fi

log() {
  echo ":: publish-npm :: $*"
}

npm_exists() {
  local package="$1" ver="$2"
  local status
  status=$(curl -so /dev/null -w '%{http_code}' \
    -H 'Accept: application/json' \
    -H 'User-Agent: cpd-publish-script' \
    "https://registry.npmjs.org/${package}/${ver}" 2>/dev/null)
  [ "$status" = "200" ]
}

detect_current_target() {
  node -e "
    const { getPlatformKey } = require('./platform-map.js');
    const key = getPlatformKey();
    if (!key) { process.exit(1); }
    process.stdout.write(key);
  "
}

build_and_publish() {
  local TARGET_KEY="$1"
  local RUST_TARGET PACKAGE_NAME BINARY_NAME
  RUST_TARGET="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].rustTarget")"
  PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].packageName")"

  BINARY_NAME="cpd"
  if [ "$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].os")" = "win32" ]; then
    BINARY_NAME="cpd.exe"
  fi

  if npm_exists "$PACKAGE_NAME" "$VERSION"; then
    log "  $PACKAGE_NAME@$VERSION already published, skipping"
    return 0
  fi

  log "Building $PACKAGE_NAME ($RUST_TARGET)"

  if [ -z "$DRY_RUN" ]; then
    if ! rustup target list --installed 2>/dev/null | grep -q "$RUST_TARGET"; then
      log "  Installing rust target $RUST_TARGET..."
      rustup target add "$RUST_TARGET"
    fi

    if ! cargo build --release --locked --target "$RUST_TARGET" -p jscpd 2>&1; then
      log "  BUILD FAILED: $PACKAGE_NAME — skipping (missing cross-compiler?)"
      return 1
    fi

    if [ ! -f "target/$RUST_TARGET/release/$BINARY_NAME" ]; then
      log "  BUILD FAILED: binary not found at target/$RUST_TARGET/release/$BINARY_NAME"
      return 1
    fi
  else
    log "  [dry-run] Would run: cargo build --release --locked --target $RUST_TARGET -p jscpd"
  fi

  local PACKAGE_DIR=""
  if [ -z "$DRY_RUN" ]; then
    PACKAGE_DIR="$(node scripts/npm-prebuilt-package.mjs \
      --target "$TARGET_KEY" \
      --bin-dir "target/$RUST_TARGET/release" \
      --out-dir "target/npm-prebuilt")"
    if [ -z "$PACKAGE_DIR" ]; then
      log "  PACKAGE FAILED: npm-prebuilt-package.mjs returned empty path"
      return 1
    fi
    log "  Package created at $PACKAGE_DIR"
  else
    log "  [dry-run] Would run: node scripts/npm-prebuilt-package.mjs --target $TARGET_KEY --bin-dir target/$RUST_TARGET/release --out-dir target/npm-prebuilt"
  fi

  log "Publishing $PACKAGE_NAME@$VERSION"
  if [ -z "$DRY_RUN" ]; then
    npm publish "$PACKAGE_DIR" --access public $PROVENANCE_FLAG
    log "  Published $PACKAGE_NAME@$VERSION"
  else
    log "  [dry-run] Would run: npm publish $PACKAGE_DIR --access public $PROVENANCE_FLAG"
  fi

  return 0
}

if [ -n "$ALL_TARGETS" ]; then
  log "Publishing all platform packages for v$VERSION"
  FAILED_TARGETS=""
  SUCCEEDED_TARGETS=""

  for TARGET_KEY in $ALL_TARGET_KEYS; do
    PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].packageName")"
    log "--- $PACKAGE_NAME ---"

    if build_and_publish "$TARGET_KEY"; then
      SUCCEEDED_TARGETS="$SUCCEEDED_TARGETS $PACKAGE_NAME"
    else
      FAILED_TARGETS="$FAILED_TARGETS $PACKAGE_NAME"
    fi
  done

  echo ""
  log "Checking all platform packages are live on npm..."
  MISSING=0
  OK=0
  for TARGET_KEY in $ALL_TARGET_KEYS; do
    PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].packageName")"
    if npm_exists "$PACKAGE_NAME" "$VERSION"; then
      log "  $PACKAGE_NAME@$VERSION OK"
      OK=$((OK + 1))
    else
      log "  $PACKAGE_NAME@$VERSION MISSING"
      MISSING=$((MISSING + 1))
    fi
  done

  echo ""
  if [ "$MISSING" -gt 0 ]; then
    log "WARNING: $MISSING platform package(s) missing from npm (needed for main package)"
    log "  Published:$SUCCEEDED_TARGETS"
    if [ -n "$FAILED_TARGETS" ]; then
      log "  Failed builds:$FAILED_TARGETS"
    fi
    log "  Publish the missing packages manually with: scripts/publish-npm.sh --target <target>"
    log "  Or use CI: git push origin HEAD:release/rust"
    log ""
    log "Skipping main cpd package publish ($MISSING missing)."
    exit 1
  fi

  log "All $OK platform packages verified. Publishing main cpd package..."
  if [ -z "$DRY_RUN" ]; then
    if npm_exists "cpd" "$VERSION"; then
      log "  cpd@$VERSION already published, skipping"
    else
      npm publish --access public $PROVENANCE_FLAG
      log "  Published cpd@$VERSION"
    fi
  else
    log "  [dry-run] Would run: npm publish --access public $PROVENANCE_FLAG"
  fi

  log "Done! cpd@$VERSION published with all platform packages."

elif [ -n "$TARGET_FLAG" ]; then
  PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_FLAG}'].packageName")"
  log "Publishing single target: $PACKAGE_NAME@$VERSION"
  if ! build_and_publish "$TARGET_FLAG"; then
    log "FAILED: $PACKAGE_NAME"
    exit 1
  fi
  echo ""
  log "Done! $PACKAGE_NAME@$VERSION published."
  echo ""
  log "To publish all platforms: scripts/publish-npm.sh --all"
  log "To publish the main package, all 6 platform packages must be live first."
else
  TARGET_KEY="$(detect_current_target)"
  if [ -z "$TARGET_KEY" ]; then
    log "ERROR: Could not detect current platform"
    exit 1
  fi
  PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].packageName")"
  log "Publishing current platform: $PACKAGE_NAME@$VERSION"
  if ! build_and_publish "$TARGET_KEY"; then
    log "FAILED: $PACKAGE_NAME"
    exit 1
  fi
  echo ""
  log "Done! $PACKAGE_NAME@$VERSION published."
  echo ""
  log "To publish all platforms: scripts/publish-npm.sh --all"
  log "To publish the main package, all 6 platform packages must be live first."
fi