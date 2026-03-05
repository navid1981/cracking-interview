# macOS Build, Code Signing & Notarization Guide

Complete step-by-step guide to build a universal (ARM + Intel) DMG with proper installer layout, code signing, and Apple notarization.

---

## Prerequisites

- **Apple Developer Account** enrolled as Organization or Individual
- **Developer ID Application certificate** installed in Keychain
- **App-specific password** generated at [appleid.apple.com](https://appleid.apple.com) (Account > Sign-In and Security > App-Specific Passwords)
- **Xcode Command Line Tools** installed (`xcode-select --install`)
- **Intel target** added to Rust (one-time): `rustup target add x86_64-apple-darwin`

### Your Credentials

| Item | Value |
|------|-------|
| Certificate | `Developer ID Application: Cracking Interview LLC (7JTN2XW63J)` |
| Apple ID | `navid.vaziri@outlook.com` |
| Team ID | `7JTN2XW63J` |
| App-specific password | *(stored in Keychain or noted privately)* |

---

## Step 1: Build Universal Binary

Tauri has built-in support for universal macOS binaries. A single command builds for both ARM (Apple Silicon) and Intel, combines them with `lipo`, and produces the `.app` bundle automatically:

```bash
# One-time setup: add Intel target to Rust
rustup target add x86_64-apple-darwin

# Build universal binary (ARM + Intel in one .app)
npm run tauri build -- --target universal-apple-darwin
```

This produces:
- `.app` at `src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app`
- A DMG at `src-tauri/target/universal-apple-darwin/release/bundle/dmg/` (but we'll recreate this DMG ourselves in Step 3 to add the drag-to-Applications layout)

Verify the binary is truly universal:

```bash
lipo -info src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app/Contents/MacOS/cracking-interview
# Expected: Architectures in the fat file: x86_64 arm64
```

> **Why not use Tauri's DMG?** Tauri's auto-generated DMG just shows the `.app` icon. It doesn't include an Applications folder shortcut for the standard drag-to-install experience. We recreate the DMG in Step 3.

---

## Step 2: Code Sign the .app Bundle

Tauri's build may use ad-hoc signing. We need to re-sign with your Developer ID, hardened runtime, and secure timestamp (all required for notarization):

```bash
codesign --force --deep \
  --sign "Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" \
  --options runtime \
  --timestamp \
  src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app
```

### Flags Explained

| Flag | Purpose |
|------|---------|
| `--force` | Re-sign even if already signed |
| `--deep` | Sign all nested frameworks and binaries |
| `--options runtime` | Enable hardened runtime (required for notarization) |
| `--timestamp` | Include secure timestamp from Apple's server (required for notarization) |

### Verify the Signature

```bash
codesign -dv --verbose=2 src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app 2>&1 | grep -E "Authority|flags|Timestamp"
```

Expected output:

```
CodeDirectory v=20500 size=... flags=0x10000(runtime) ...
Authority=Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
Authority=Developer ID Certification Authority
Authority=Apple Root CA
Timestamp=...
```

Confirm all three checks:
- `flags=0x10000(runtime)` — hardened runtime enabled
- `Authority=Developer ID Application: ...` — proper signing identity
- `Timestamp=...` — secure timestamp present

---

## Step 3: Create DMG with Drag-to-Applications Layout

### 3a: Prepare DMG Contents

```bash
WORK_DIR="/tmp/cracking-dmg-build"
rm -rf "$WORK_DIR"
mkdir -p "$WORK_DIR/dmg-content"

# Copy the signed .app
cp -R src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app \
  "$WORK_DIR/dmg-content/"

# Create Applications symlink (this is what enables drag-to-install)
ln -s /Applications "$WORK_DIR/dmg-content/Applications"
```

### 3b: Create Compressed DMG

```bash
DMG_OUT="src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg"
mkdir -p src-tauri/target/universal-apple-darwin/release/bundle/dmg

hdiutil create \
  -volname "CrackingInterview" \
  -srcfolder "$WORK_DIR/dmg-content" \
  -ov \
  -format UDZO \
  -imagekey zlib-level=9 \
  "$DMG_OUT"
```

### 3c: Style the DMG Window (Icon Layout)

This step makes the DMG open with a nice window showing the app on the left and Applications folder on the right (like Cursor's installer).

Convert to read-write, apply layout, then convert back:

```bash
DMG_RW="$WORK_DIR/CrackingInterview_rw.dmg"

# Convert to read-write
hdiutil convert "$DMG_OUT" -format UDRW -o "$DMG_RW"

# Mount
MOUNT_POINT=$(hdiutil attach "$DMG_RW" -readwrite -noverify -noautoopen \
  | grep "/Volumes/" | tail -1 | awk -F'\t' '{print $NF}' | xargs)

echo "Mounted at: $MOUNT_POINT"
```

Apply Finder window styling via AppleScript:

```bash
# IMPORTANT: Replace "CrackingInterview" with the actual mounted volume name
# shown above (could be "CrackingInterview 2" if another volume is mounted)

osascript <<'APPLESCRIPT'
tell application "Finder"
    tell disk "CrackingInterview"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {200, 200, 720, 520}
        set viewOptions to the icon view options of container window
        set arrangement of viewOptions to not arranged
        set icon size of viewOptions to 96
        set background color of viewOptions to {65535, 65535, 65535}
        set position of item "CrackingInterview.app" of container window to {130, 150}
        set position of item "Applications" of container window to {390, 150}
        close
        open
    end tell
end tell
APPLESCRIPT
```

Finalize — detach, convert back to compressed read-only:

```bash
sync
hdiutil detach "$MOUNT_POINT"

rm -f "$DMG_OUT"
hdiutil convert "$DMG_RW" -format UDZO -imagekey zlib-level=9 -o "$DMG_OUT"
```

---

## Step 4: Sign the DMG

The DMG itself also needs to be signed:

```bash
codesign --force \
  --sign "Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" \
  --timestamp \
  "$DMG_OUT"
```

Verify:

```bash
codesign -dv "$DMG_OUT" 2>&1
# Should show: Authority=Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
```

---

## Step 5: Submit for Notarization

```bash
xcrun notarytool submit "$DMG_OUT" \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password "YOUR_APP_SPECIFIC_PASSWORD" \
  --wait
```

The `--wait` flag blocks until Apple finishes processing (typically 2-10 minutes).

Expected result: `status: Accepted`

### If Notarization Fails

Check the detailed log:

```bash
xcrun notarytool log <SUBMISSION_ID> \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password "YOUR_APP_SPECIFIC_PASSWORD"
```

Common errors and fixes:

| Error | Fix |
|-------|-----|
| "The binary is not signed" | Re-run `codesign` on the `.app` (Step 2) |
| "No secure timestamp" | Add `--timestamp` flag to `codesign` |
| "Hardened runtime not enabled" | Add `--options runtime` flag to `codesign` |
| "Signature is invalid" | Re-sign the `.app` after building |

### Check Notarization History

```bash
xcrun notarytool history \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password "YOUR_APP_SPECIFIC_PASSWORD"
```

---

## Step 6: Staple the Notarization Ticket

Stapling embeds the notarization ticket in the DMG so users can verify offline (without internet):

```bash
xcrun stapler staple "$DMG_OUT"
# Expected: "The staple and validate action worked!"
```

### Verify

```bash
spctl -a -vv "$DMG_OUT"
```

> **Note:** `spctl` may show "rejected" for DMG files — this is normal. The important part is "the code is valid" and the correct Developer ID. `spctl` is designed for `.app` bundles. The real verification is that `stapler staple` succeeded.

---

## Step 7: Copy to Website for Distribution

```bash
cp "$DMG_OUT" /path/to/your/website/CrackingInterview_1.0.0_universal.dmg
```

---

## Quick Reference: Full Pipeline

After the one-time setup (`rustup target add x86_64-apple-darwin`), here's the complete flow:

```bash
# 1. Build universal binary
npm run tauri build -- --target universal-apple-darwin

# 2. Code sign the .app
codesign --force --deep \
  --sign "Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" \
  --options runtime --timestamp \
  src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app

# 3. Prepare DMG contents
WORK_DIR="/tmp/cracking-dmg-build"
rm -rf "$WORK_DIR" && mkdir -p "$WORK_DIR/dmg-content"
cp -R src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app "$WORK_DIR/dmg-content/"
ln -s /Applications "$WORK_DIR/dmg-content/Applications"

# 4. Create DMG
DMG_OUT="src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg"
hdiutil create -volname "CrackingInterview" -srcfolder "$WORK_DIR/dmg-content" \
  -ov -format UDZO -imagekey zlib-level=9 "$DMG_OUT"

# 5. (Optional) Style DMG window — see Step 3c above

# 6. Sign DMG
codesign --force \
  --sign "Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" \
  --timestamp "$DMG_OUT"

# 7. Notarize
xcrun notarytool submit "$DMG_OUT" \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password "YOUR_APP_SPECIFIC_PASSWORD" \
  --wait

# 8. Staple
xcrun stapler staple "$DMG_OUT"

# 9. Copy to website
cp "$DMG_OUT" /path/to/your/website/CrackingInterview_1.0.0_universal.dmg
```

---

## Troubleshooting

### Notarization Stuck "In Progress"
- Typically completes in 2-10 minutes. If stuck for hours:
  - Ensure you're not on a VPN or corporate proxy (e.g., Zscaler)
  - Check [Apple Developer System Status](https://developer.apple.com/system-status/)
  - Submit a new DMG — old submissions cannot be cancelled

### DMG Opens Without Drag-to-Applications Layout
- Tauri's auto-generated DMG doesn't include the Applications shortcut
- Rebuild the DMG following Step 3

### "CrackingInterview is damaged and can't be opened"
- The DMG wasn't notarized or the ticket wasn't stapled
- Re-run Steps 5 and 6

### App Not Working on Intel Macs
- Verify the binary is truly universal: `lipo -info .../Contents/MacOS/cracking-interview`
- Should show: `Architectures in the fat file: x86_64 arm64`
