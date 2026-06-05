#!/usr/bin/env bash
# publish-npm.sh — build, package, and publish cpd to npm
#
# Usage:
#   scripts/publish-npm.sh [--dry-run] [--target <target>]
#
# Without --target, builds and publishes only for the current platform.
# With --target, cross-compiles for the specified target
#   (requires cross-compilation toolchain to be installed).
#
# Targets: linux-x64-gnu, linux-arm64-gnu, linux-x64-musl,
#          darwin-arm64, darwin-x64, windows-x64-msvc
#
# Steps:
#   1. Build the Rust binary for the target
#   2. Create the platform npm package
#   3. Publish the platform package to npm
#   4. Publish the main cpd package to npm (after all platforms)
#
# Environment:
#   NPM_TOKEN — npm auth token (or already logged in via npm login)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

DRY_RUN=""
TARGET_FLAG=""

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
    --help|-h)
      echo "Usage: scripts/publish-npm.sh [--dry-run] [--target <target>]"
      echo ""
      echo "Build and publish cpd npm packages."
      echo ""
      echo "Options:"
      echo "  --dry-run          Show what would be published without actually publishing"
      echo "  --target <target>  Cross-compile for a specific target"
      echo ""
      echo "Available targets: linux-x64-gnu, linux-arm64-gnu, linux-x64-musl,"
      echo "                   darwin-arm64, darwin-x64, windows-x64-msvc"
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
  local os cpu libc
  os="$(node -p "process.platform")"
  cpu="$(node -p "process.arch")"
  if [ "$os" = "linux" ]; then
    libc="$(node -p "
      const r = process.report.getReport();
      r.header && r.header.glibcVersionRuntime ? 'glibc' : 'musl'
    ")"
  else
    libc=""
  fi
  node -e "
    const { getPlatformKey } = require('./platform-map.js');
    const key = getPlatformKey();
    if (!key) { process.exit(1); }
    process.stdout.write(key);
  "
}

if [ -n "$TARGET_FLAG" ]; then
  TARGET_KEY="$TARGET_FLAG"
else
  TARGET_KEY="$(detect_current_target)"
  if [ -z "$TARGET_KEY" ]; then
    log "ERROR: Could not detect current platform"
    exit 1
  fi
fi

RUST_TARGET="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].rustTarget")"
PACKAGE_NAME="$(node -p "require('./npm/prebuilt-targets.json')['${TARGET_KEY}'].packageName")"

log "Version:      $VERSION"
log "Target:       $TARGET_KEY"
log "Rust target:  $RUST_TARGET"
log "Package:      $PACKAGE_NAME"

log "Step 1/3: Building Rust binary for $RUST_TARGET"
if [ -z "$DRY_RUN" ]; then
  if ! rustup target list --installed 2>/dev/null | grep -q "$RUST_TARGET"; then
    log "  Installing rust target $RUST_TARGET..."
    rustup target add "$RUST_TARGET"
  fi
  cargo build --release --locked --target "$RUST_TARGET" -p jscpd
  log "  Build complete"
else
  log "  [dry-run] Would run: cargo build --release --locked --target $RUST_TARGET -p jscpd"
fi

log "Step 2/3: Creating platform package $PACKAGE_NAME"
if [ -z "$DRY_RUN" ]; then
  PACKAGE_DIR="$(node scripts/npm-prebuilt-package.mjs \
    --target "$TARGET_KEY" \
    --bin-dir "target/$RUST_TARGET/release" \
    --out-dir "target/npm-prebuilt")"
  log "  Package created at $PACKAGE_DIR"
else
  log "  [dry-run] Would run: node scripts/npm-prebuilt-package.mjs --target $TARGET_KEY --bin-dir target/$RUST_TARGET/release --out-dir target/npm-prebuilt"
fi

log "Step 3/3: Publishing $PACKAGE_NAME@$VERSION"
if [ -z "$DRY_RUN" ]; then
  if npm_exists "$PACKAGE_NAME" "$VERSION"; then
    log "  $PACKAGE_NAME@$VERSION already published, skipping"
  else
    npm publish "$PACKAGE_DIR" --access public --provenance
    log "  Published $PACKAGE_NAME@$VERSION"
  fi
else
  log "  [dry-run] Would run: npm publish $PACKAGE_DIR --access public --provenance"
fi

echo ""
log "Platform package published: $PACKAGE_NAME@$VERSION"
echo ""
log "To publish the main cpd package, all 6 platform packages must be published first."
log "Check with:  npm view cpd-linux-x64-gnu@$VERSION && npm view cpd-linux-arm64-gnu@$VERSION && npm view cpd-linux-x64-musl@$VERSION && npm view cpd-darwin-arm64@$VERSION && npm view cpd-darwin-x64@$VERSION && npm view cpd-windows-x64-msvc@$VERSION"
echo ""
log "Once all platform packages are live, publish the main package with:"
log "  cd rust && npm publish --access public --provenance"