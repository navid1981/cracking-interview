#!/usr/bin/env bash
set -euo pipefail

# ── Constants ────────────────────────────────────────────────────────────────
SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)"
APPLE_ID="navid.vaziri@outlook.com"
TEAM_ID="7JTN2XW63J"
APP_NAME="CrackingInterview"
TAURI_CONF="src-tauri/tauri.conf.json"
WORK_DIR="/tmp/cracking-dmg-build"

# ── Helpers ──────────────────────────────────────────────────────────────────
step() { echo -e "\n\033[1;34m▶ Step $1: $2\033[0m"; }
ok()   { echo -e "  \033[1;32m✔ $1\033[0m"; }
fail() { echo -e "  \033[1;31m✘ $1\033[0m"; exit 1; }

# ── Pre-flight checks ───────────────────────────────────────────────────────
command -v npm     >/dev/null || fail "npm not found"
command -v cargo   >/dev/null || fail "cargo not found"
command -v codesign >/dev/null || fail "codesign not found (install Xcode CLT)"
command -v xcrun   >/dev/null || fail "xcrun not found (install Xcode CLT)"
command -v hdiutil >/dev/null || fail "hdiutil not found"

[[ -f "$TAURI_CONF" ]] || fail "$TAURI_CONF not found — run from project root"

VERSION=$(python3 -c "import json; print(json.load(open('$TAURI_CONF'))['version'])")
[[ -n "$VERSION" ]] || fail "Could not read version from $TAURI_CONF"

BUNDLE_DIR="src-tauri/target/universal-apple-darwin/release/bundle"
APP_PATH="$BUNDLE_DIR/macos/$APP_NAME.app"
DMG_DIR="$BUNDLE_DIR/dmg"
DMG_OUT="$DMG_DIR/${APP_NAME}_${VERSION}_universal.dmg"

echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  $APP_NAME v$VERSION — macOS Build & Sign Pipeline"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Prompt for app-specific password up front
echo ""
read -sp "Enter Apple app-specific password for notarization: " APP_PASSWORD
echo ""
[[ -n "$APP_PASSWORD" ]] || fail "Password cannot be empty"

# ── Step 1: Build universal binary ───────────────────────────────────────────
step 1 "Build universal binary (ARM + Intel)"

rustup target list --installed | grep -q x86_64-apple-darwin \
  || { echo "  Adding x86_64-apple-darwin target..."; rustup target add x86_64-apple-darwin; }

npm run tauri build -- --target universal-apple-darwin --bundles app
[[ -d "$APP_PATH" ]] || fail "Build failed — $APP_PATH not found"

lipo -info "$APP_PATH/Contents/MacOS/cracking-interview" | grep -q "x86_64 arm64" \
  || fail "Binary is not universal (expected x86_64 + arm64)"
ok "Universal .app built at $APP_PATH"

# ── Step 2: Code sign the .app ───────────────────────────────────────────────
step 2 "Code sign .app with Developer ID + hardened runtime"

# Sign the bundled audio helper binary first (must be signed individually
# before signing the .app, since --deep doesn't reach into Resources/)
AUDIO_HELPER="$APP_PATH/Contents/Resources/audio_recorder_bin"
if [[ -f "$AUDIO_HELPER" ]]; then
  echo "  Signing audio_recorder_bin..."
  codesign --force \
    --sign "$SIGNING_IDENTITY" \
    --options runtime \
    --timestamp \
    "$AUDIO_HELPER"
  ok "audio_recorder_bin signed"
fi

codesign --force --deep \
  --sign "$SIGNING_IDENTITY" \
  --options runtime \
  --timestamp \
  "$APP_PATH"

SIGN_CHECK=$(codesign -dv --verbose=2 "$APP_PATH" 2>&1)
echo "$SIGN_CHECK" | grep -q "flags=.*runtime"         || fail "Hardened runtime flag missing"
echo "$SIGN_CHECK" | grep -qF "$SIGNING_IDENTITY"       || fail "Wrong signing identity"
echo "$SIGN_CHECK" | grep -q "Timestamp="              || fail "Secure timestamp missing"
ok "Code signed and verified"

# ── Step 3: Create DMG with drag-to-Applications layout ─────────────────────
step 3 "Create DMG with Applications shortcut"

rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR/dmg-content"
cp -R "$APP_PATH" "$WORK_DIR/dmg-content/"
ln -s /Applications "$WORK_DIR/dmg-content/Applications"

mkdir -p "$DMG_DIR"
rm -f "$DMG_OUT"

hdiutil create \
  -volname "$APP_NAME" \
  -srcfolder "$WORK_DIR/dmg-content" \
  -ov \
  -format UDZO \
  -imagekey zlib-level=9 \
  "$DMG_OUT"
ok "Compressed DMG created"

# ── Step 4: Style the DMG window ─────────────────────────────────────────────
step 4 "Style DMG window (icon layout)"

DMG_RW="$WORK_DIR/${APP_NAME}_rw.dmg"
hdiutil convert "$DMG_OUT" -format UDRW -o "$DMG_RW"

MOUNT_POINT=$(hdiutil attach "$DMG_RW" -readwrite -noverify -noautoopen \
  | grep "/Volumes/" | tail -1 | awk -F'\t' '{print $NF}' | xargs)

VOLUME_NAME=$(basename "$MOUNT_POINT")
echo "  Mounted at: $MOUNT_POINT (volume: $VOLUME_NAME)"

osascript <<APPLESCRIPT
tell application "Finder"
    tell disk "$VOLUME_NAME"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {200, 200, 720, 520}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 96
        set background color of viewOptions to {65535, 65535, 65535}
        set position of item "$APP_NAME.app" of container window to {130, 150}
        set position of item "Applications" of container window to {390, 150}
        close
        open
    end tell
end tell
APPLESCRIPT

sync
hdiutil detach "$MOUNT_POINT"

rm -f "$DMG_OUT"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_OUT"
ok "DMG styled with drag-to-Applications layout"

# ── Step 5: Sign the DMG ─────────────────────────────────────────────────────
step 5 "Sign DMG"

codesign --force \
  --sign "$SIGNING_IDENTITY" \
  --timestamp \
  "$DMG_OUT"

DMG_SIGN_CHECK=$(codesign -dv --verbose=2 "$DMG_OUT" 2>&1) || true
echo "$DMG_SIGN_CHECK"
echo "$DMG_SIGN_CHECK" | grep -qF "Developer ID Application" \
  || fail "DMG signing verification failed"
ok "DMG signed"

# ── Step 6: Notarize ─────────────────────────────────────────────────────────
step 6 "Submit for Apple notarization (this may take several minutes)"

xcrun notarytool submit "$DMG_OUT" \
  --apple-id "$APPLE_ID" \
  --team-id "$TEAM_ID" \
  --password "$APP_PASSWORD" \
  --wait

ok "Notarization accepted"

# ── Step 7: Staple the notarization ticket ───────────────────────────────────
step 7 "Staple notarization ticket to DMG"

xcrun stapler staple "$DMG_OUT"
ok "Ticket stapled"

# ── Done ─────────────────────────────────────────────────────────────────────
rm -rf "$WORK_DIR"

DMG_SIZE=$(du -h "$DMG_OUT" | cut -f1 | xargs)
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Done! Signed & notarized DMG ready:"
echo "  $DMG_OUT ($DMG_SIZE)"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
