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

Tauri has built-in support for universal macOS binaries. A single command builds for both ARM (Apple Silicon) and Intel, combines them with `lipo`, and produces the `.app` bundle automatically.

During the build, the Rust build script (`src-tauri/build.rs`) automatically compiles the Swift audio helper (`src-tauri/resources/audio_recorder.swift`) into a native binary (`audio_recorder_bin`) using `xcrun swiftc`. This binary is then bundled inside the `.app` at `Contents/Resources/audio_recorder_bin` via `tauri.conf.json`'s `bundle.resources` config. This eliminates the need for end users to install Xcode Command Line Tools.

```bash
# One-time setup: add Intel target to Rust
rustup target add x86_64-apple-darwin

# Build universal binary (ARM + Intel in one .app)
npm run tauri build -- --target universal-apple-darwin --bundles app
```

This produces:
- `.app` at `src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app`
  - Includes `Contents/Resources/audio_recorder_bin` (pre-compiled Swift audio helper)

Verify the binary is truly universal:

```bash
lipo -info src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app/Contents/MacOS/cracking-interview
# Expected: Architectures in the fat file: x86_64 arm64
```

Verify the audio helper is bundled:

```bash
ls -la src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app/Contents/Resources/audio_recorder_bin
# Should exist and be executable
```

> **Why `--bundles app`?** We only build the `.app` bundle here because we create our own DMG in Step 3 with a drag-to-Applications layout. Tauri's auto-generated DMG doesn't include an Applications folder shortcut.

---

## Step 2: Code Sign the .app Bundle

Tauri's build may use ad-hoc signing. We need to re-sign with your Developer ID, hardened runtime, and secure timestamp (all required for notarization).

**Important:** The bundled `audio_recorder_bin` must be signed **individually before** signing the `.app`. The `--deep` flag on the `.app` does not reliably reach binaries inside `Contents/Resources/`. If the audio helper is unsigned, notarization will fail with "The binary is not signed" / "no secure timestamp" / "no hardened runtime enabled".

```bash
APP_PATH="src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app"
IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)"

# 2a. Sign the audio helper binary first
codesign --force \
  --sign "$IDENTITY" \
  --options runtime \
  --timestamp \
  "$APP_PATH/Contents/Resources/audio_recorder_bin"

# 2b. Sign the .app bundle (includes all nested frameworks)
codesign --force --deep \
  --sign "$IDENTITY" \
  --options runtime \
  --timestamp \
  "$APP_PATH"
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
codesign -dv --verbose=2 "$APP_PATH" 2>&1 | grep -E "Authority|flags|Timestamp"

# Also verify the audio helper is signed
codesign -dv "$APP_PATH/Contents/Resources/audio_recorder_bin" 2>&1 | grep "Authority"
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
| "The binary is not signed" | Sign `audio_recorder_bin` individually, then re-sign `.app` (Step 2) |
| "No secure timestamp" | Add `--timestamp` flag to `codesign` |
| "Hardened runtime not enabled" | Add `--options runtime` flag to `codesign` |
| "Signature is invalid" | Re-sign the `.app` after building |
| `audio_recorder_bin` fails notarization | Sign the helper binary **before** signing the `.app` (Step 2a) |

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
APP_PATH="src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app"
IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)"

# 1. Build universal binary (also compiles Swift audio helper via build.rs)
npm run tauri build -- --target universal-apple-darwin --bundles app

# 2a. Sign the bundled audio helper binary (MUST be before signing .app)
codesign --force --sign "$IDENTITY" --options runtime --timestamp \
  "$APP_PATH/Contents/Resources/audio_recorder_bin"

# 2b. Sign the .app
codesign --force --deep --sign "$IDENTITY" --options runtime --timestamp "$APP_PATH"

# 3. Prepare DMG contents
WORK_DIR="/tmp/cracking-dmg-build"
rm -rf "$WORK_DIR" && mkdir -p "$WORK_DIR/dmg-content"
cp -R "$APP_PATH" "$WORK_DIR/dmg-content/"
ln -s /Applications "$WORK_DIR/dmg-content/Applications"

# 4. Create DMG
DMG_OUT="src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg"
mkdir -p "$(dirname "$DMG_OUT")"
hdiutil create -volname "CrackingInterview" -srcfolder "$WORK_DIR/dmg-content" \
  -ov -format UDZO -imagekey zlib-level=9 "$DMG_OUT"

# 5. (Optional) Style DMG window — see Step 3c above

# 6. Sign DMG
codesign --force --sign "$IDENTITY" --timestamp "$DMG_OUT"

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

> **Automated:** All of the above is automated by `bash scripts/build-macos.sh`

---

## Audio Helper Architecture

The app uses a Swift helper binary (`audio_recorder_bin`) for system audio capture via ScreenCaptureKit. This architecture is required because ScreenCaptureKit is a Swift/Objective-C API that cannot be called directly from Rust.

### How It Works

1. **Build time** (`src-tauri/build.rs`): Compiles `src-tauri/resources/audio_recorder.swift` → `src-tauri/resources/audio_recorder_bin` using `xcrun swiftc`
2. **Bundle** (`tauri.conf.json`): Bundles `resources/audio_recorder_bin` into `.app/Contents/Resources/audio_recorder_bin`
3. **Code signing** (`scripts/build-macos.sh`): Signs the helper binary individually with Developer ID + hardened runtime before signing the `.app`
4. **Runtime** (`src-tauri/src/audio.rs`): `find_helper_binary()` locates the signed binary inside the bundle. Both the warm-up recorder (`audio.rs`) and live transcription streamer (`transcription.rs`) use this same function
5. **Fallback** (dev only): If the bundled binary is not found, falls back to runtime compilation via `xcrun swiftc` to `/tmp/` (requires Xcode CLT — developer machines only)

### Why the Binary Must Be Signed

macOS ScreenCaptureKit checks the "responsible process" when a helper binary uses its XPC service. An unsigned binary spawned by the app will have its XPC connection interrupted (`SCStreamErrorDomain Code=-3805`). The signed binary in the bundle inherits the app's Screen Recording permission because macOS attributes it to `CrackingInterview.app` (the responsible process).

### Key Files

| File | Purpose |
|------|---------|
| `src-tauri/resources/audio_recorder.swift` | Swift source — ScreenCaptureKit audio capture |
| `src-tauri/build.rs` | Compiles Swift source at build time |
| `src-tauri/tauri.conf.json` | Bundles compiled binary into `.app` |
| `src-tauri/src/audio.rs` | `find_helper_binary()` — locates bundled binary; warm-up & recording |
| `src-tauri/src/transcription.rs` | Live transcription — uses `find_helper_binary()` for stream mode |
| `scripts/build-macos.sh` | Signs `audio_recorder_bin` before `.app` in build pipeline |

---

## Troubleshooting

### Audio Recording: "application connection being interrupted" (SCStreamErrorDomain -3805)
- The `audio_recorder_bin` is unsigned or missing from the bundle
- Verify the binary exists: `ls -la CrackingInterview.app/Contents/Resources/audio_recorder_bin`
- Verify it is signed: `codesign -dv CrackingInterview.app/Contents/Resources/audio_recorder_bin 2>&1 | grep Authority`
- If missing, rebuild with latest code. If unsigned, re-run Step 2 (sign helper before .app)
- Also ensure CrackingInterview.app has **Screen Recording** permission in System Settings

### Audio Recording: No transcription in dev mode
- When running `npm run tauri dev`, the audio helper binary falls back to `/tmp/` (unsigned)
- Run from **Terminal.app** (not IDE terminals) — Terminal must have Screen Recording permission
- Alternatively, add the Tauri dev binary to Screen Recording: `src-tauri/target/debug/cracking-interview`

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
