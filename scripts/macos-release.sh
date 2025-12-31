#!/usr/bin/env bash
set -euo pipefail

# Local macOS release build: build + sign + notarize + staple.
# Requires environment variables (do not commit secrets):
# - MACOS_SIGNING_IDENTITY (e.g. "Developer ID Application: ...")
# - APPLE_ID (e.g. name@example.com)
# - APPLE_TEAM_ID (e.g. ABCDE12345)
# - APPLE_APP_SPECIFIC_PASSWORD (recommended) OR APPLE_PASSWORD
# Optional:
# - NOTARIZE_PROFILE (keychain profile name for `xcrun notarytool`) OR
#   - APPLE_NOTARY_KEYCHAIN_PROFILE

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_DIR="$ROOT_DIR/src-tauri"
DMG_DIR="$TAURI_DIR/target/release/bundle/dmg"

MACOS_SIGNING_IDENTITY="${MACOS_SIGNING_IDENTITY:-}"
APPLE_ID="${APPLE_ID:-}"
APPLE_TEAM_ID="${APPLE_TEAM_ID:-}"
APPLE_APP_SPECIFIC_PASSWORD="${APPLE_APP_SPECIFIC_PASSWORD:-${APPLE_PASSWORD:-}}"
NOTARIZE_PROFILE="${NOTARIZE_PROFILE:-${APPLE_NOTARY_KEYCHAIN_PROFILE:-}}"

if [[ -z "$MACOS_SIGNING_IDENTITY" ]]; then
  echo "Missing env: MACOS_SIGNING_IDENTITY (must be a Developer ID Application identity for distribution)" >&2
  exit 1
fi

if [[ -z "$NOTARIZE_PROFILE" ]]; then
  if [[ -z "$APPLE_ID" || -z "$APPLE_TEAM_ID" || -z "$APPLE_APP_SPECIFIC_PASSWORD" ]]; then
    echo "Missing notarization creds. Provide NOTARIZE_PROFILE, or set APPLE_ID + APPLE_TEAM_ID + APPLE_APP_SPECIFIC_PASSWORD." >&2
    exit 1
  fi
fi

echo "Building (tauri build)..."
(cd "$ROOT_DIR" && bun run build)

DMG_PATH="$(ls -1t "$DMG_DIR"/*.dmg | head -n 1)"
echo "Built DMG: $DMG_PATH"

WORK_DIR="$(mktemp -d)"
cleanup() { rm -rf "$WORK_DIR"; }
trap cleanup EXIT

echo "Mounting DMG..."
MOUNT_INFO=$(hdiutil attach "$DMG_PATH" -nobrowse -noverify -noautoopen -plist)
MOUNT_POINT=$(echo "$MOUNT_INFO" | plutil -extract system-entities xml1 -o - - | xmllint --xpath 'string(//dict/key[.="mount-point"]/following-sibling::*[1])' -)

if [[ -z "$MOUNT_POINT" ]]; then
  echo "Failed to determine mount point for DMG" >&2
  exit 1
fi

APP_NAME=$(ls -1 "$MOUNT_POINT" | grep -m 1 -E '\\.app$' || true)

if [[ -z "$APP_NAME" ]]; then
  echo "Could not find .app inside mounted DMG at $MOUNT_POINT" >&2
  hdiutil detach "$MOUNT_POINT" -quiet || true
  exit 1
fi

APP_SRC="$MOUNT_POINT/$APP_NAME"
APP_DEST="$WORK_DIR/$APP_NAME"
echo "Copying app out of DMG..."
cp -R "$APP_SRC" "$APP_DEST"

echo "Detaching DMG..."
hdiutil detach "$MOUNT_POINT" -quiet

echo "Signing app with: $MACOS_SIGNING_IDENTITY"
codesign --force --options runtime --timestamp --deep --sign "$MACOS_SIGNING_IDENTITY" "$APP_DEST"

echo "Verifying signature..."
codesign --verify --deep --strict --verbose=2 "$APP_DEST"
spctl -a -vv "$APP_DEST" || true

ZIP_PATH="$WORK_DIR/app.zip"
echo "Creating notarization zip..."
/usr/bin/ditto -c -k --keepParent "$APP_DEST" "$ZIP_PATH"

echo "Submitting for notarization..."
if [[ -n "$NOTARIZE_PROFILE" ]]; then
  xcrun notarytool submit "$ZIP_PATH" --keychain-profile "$NOTARIZE_PROFILE" --wait
else
  xcrun notarytool submit "$ZIP_PATH" --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" --password "$APPLE_APP_SPECIFIC_PASSWORD" --wait
fi

echo "Stapling notarization ticket..."
xcrun stapler staple "$APP_DEST"

echo "Rebuilding DMG with notarized app..."
NOTARIZED_DMG="$DMG_DIR/$(basename "$DMG_PATH" .dmg)_notarized.dmg"
rm -f "$NOTARIZED_DMG"

DMG_STAGE="$WORK_DIR/dmg-stage"
mkdir -p "$DMG_STAGE"
cp -R "$APP_DEST" "$DMG_STAGE/"

hdiutil create -volname "Emdash2" -srcfolder "$DMG_STAGE" -ov -format UDZO "$NOTARIZED_DMG" >/dev/null

echo "Assessing notarized DMG..."
spctl -a -vv "$NOTARIZED_DMG"

echo "Done: $NOTARIZED_DMG"
