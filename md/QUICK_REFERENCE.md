# Quick Reference - Commands

## Build

```bash
cd /Users/nsalehvaziri/cracking-interview

# Clean build
rm -rf src-tauri/target/universal-apple-darwin/release

# Build universal (Intel + Apple Silicon) signed app + DMG
APPLE_SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" \
  npm run tauri build -- --target universal-apple-darwin
```

**Output:**
```
src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app
src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg
```

**Verify universal binary:**
```bash
lipo -archs src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app/Contents/MacOS/cracking-interview
# Expected: x86_64 arm64
```

---

## Verify Signature

```bash
# Check signing authority
codesign -dv --verbose=2 src-tauri/target/release/bundle/macos/CrackingInterview.app 2>&1 | grep Authority

# Check hardened runtime
codesign -dv --verbose=2 src-tauri/target/release/bundle/macos/CrackingInterview.app 2>&1 | grep -i "runtime\|flags"

# Check entitlements
codesign -d --entitlements - src-tauri/target/release/bundle/macos/CrackingInterview.app

# List certificates
security find-identity -v -p codesigning
```

---

## Notarization

### Submit DMG
```bash
xcrun notarytool submit src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp \
  --wait
```

### Submit .app as ZIP (alternative)
```bash
ditto -c -k --keepParent \
  src-tauri/target/release/bundle/macos/CrackingInterview.app \
  /tmp/CrackingInterview.zip

xcrun notarytool submit /tmp/CrackingInterview.zip \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp \
  --wait
```

### Check Status
```bash
xcrun notarytool history \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp
```

### Get Error Log
```bash
xcrun notarytool log <SUBMISSION-ID> \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp
```

### Staple Ticket
```bash
# Staple to DMG
xcrun stapler staple src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg

# Or staple to .app
xcrun stapler staple src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app

# Verify
spctl -a -vv src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app
```

---

## Troubleshooting

### "Certificate not found"
```bash
security find-identity -v -p codesigning
# If empty, create certificate in Xcode > Settings > Accounts > Manage Certificates
```

### Build fails
```bash
rm -rf node_modules src-tauri/target
npm install
npm run tauri build
```

### "Command not found: xcrun"
```bash
xcode-select --install
```

---

## Credentials

```
Apple ID:          navid.vaziri@outlook.com
Team ID:           7JTN2XW63J
App Password:      yvdy-dbhj-dpmh-ajcp
Signing Identity:  Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
Certificate Hash:  2AAD596E9F1E575D550A37778BBE5EF07D7A2699
Bundle ID:         com.crackinginterview.app
Version:           1.0.0
Min macOS:         11.0 (Big Sur)
```
