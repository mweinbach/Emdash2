#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
REPO_ROOT=$(cd "${SCRIPT_DIR}/../.." && pwd)

GHOSTTY_REPO="https://github.com/ghostty-org/ghostty.git"
# Pinned to current upstream main HEAD (Dec 31, 2025).
GHOSTTY_COMMIT="f32d54bedbc772ea3ae5c5ff081ffd97aab7cf40"

GHOSTTY_DIR="${SCRIPT_DIR}/.ghostty"
PATCH_FILE="${SCRIPT_DIR}/patches/ghostty-wasm-api.patch"
OUTPUT_FILE="${REPO_ROOT}/src/assets/wasm/ghostty-vt.wasm"

if ! command -v zig >/dev/null 2>&1; then
  echo "Zig not found. Install Zig 0.15.2+ and retry."
  exit 1
fi

if ! command -v git >/dev/null 2>&1; then
  echo "Git not found. Install Git and retry."
  exit 1
fi

if [ ! -d "${GHOSTTY_DIR}/.git" ]; then
  echo "Cloning Ghostty..."
  git clone --depth 1 "${GHOSTTY_REPO}" "${GHOSTTY_DIR}"
fi

pushd "${GHOSTTY_DIR}" >/dev/null

# Ensure we are on the pinned commit.
git fetch --depth 1 origin "${GHOSTTY_COMMIT}"
git checkout -q "${GHOSTTY_COMMIT}"

# Apply patch.
echo "Applying Ghostty WASM API patch..."
git apply --check "${PATCH_FILE}"
git apply "${PATCH_FILE}"

# Build wasm.
echo "Building ghostty-vt.wasm..."
zig build lib-vt -Dtarget=wasm32-freestanding -Doptimize=ReleaseSmall

mkdir -p "$(dirname "${OUTPUT_FILE}")"
cp "${GHOSTTY_DIR}/zig-out/bin/ghostty-vt.wasm" "${OUTPUT_FILE}"

# Revert patch to keep clone clean.
echo "Cleaning patch..."
git apply -R "${PATCH_FILE}"
rm -f include/ghostty/vt/terminal.h
rm -f src/terminal/c/terminal.zig

popd >/dev/null

echo "Wrote ${OUTPUT_FILE}"
