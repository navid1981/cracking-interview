# Release Progress Tracker

## Current Status: Notarization Pending

---

## Completed Steps

### Step 1: Apple Developer Enrollment ✅
- LLC: Cracking Interview LLC
- Enrollment completed and payment processed ($99)
- Apple ID: navid.vaziri@outlook.com

### Step 2: Certificate Created in Xcode ✅
- Type: Developer ID Application
- Identity: `Developer ID Application: Cracking Interview LLC (7JTN2XW63J)`
- Hash: `2AAD596E9F1E575D550A37778BBE5EF07D7A2699`
- Team ID: `7JTN2XW63J`

**Verified with:**
```bash
security find-identity -v -p codesigning
```

### Step 3: Signed Build Completed ✅
- Built with: `APPLE_SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" npm run tauri build -- --target universal-apple-darwin`
- App location: `src-tauri/target/universal-apple-darwin/release/bundle/macos/CrackingInterview.app`
- DMG location: `src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg`
- Architecture: **Universal (x86_64 + arm64)** — works on both Intel and Apple Silicon Macs

**Signature verified:**
```
Authority=Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
Authority=Developer ID Certification Authority
Authority=Apple Root CA
```

**Hardened runtime confirmed:**
```
flags=0x10000(runtime)
```

### Step 4: Notarization ⏳ WAITING
- Submitted: Feb 21, 2026
- Status: **In Progress** (expected ~6 days for new Developer ID accounts)
- Submission IDs:
  - `7928a4a6-f45d-4a48-8c43-0a8cbf926157`
  - `377570e8-7162-4bc6-a875-f43b0499b506`

**Note:** First-time notarization from a new Developer ID account can take up to 6 days (confirmed by Apple Developer Forums). Future notarizations will take 5-15 minutes.

**Check status:**
```bash
xcrun notarytool history \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp
```

---

## Remaining Steps (After Notarization Accepted)

### Step 5: Staple Notarization Ticket
```bash
# Staple to DMG
xcrun stapler staple src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg

# Verify
spctl -a -vvv --type install src-tauri/target/universal-apple-darwin/release/bundle/dmg/CrackingInterview_1.0.0_universal.dmg
```

### Step 6: Verify on Clean Mac
- Transfer DMG to a different Mac
- Open and install
- Confirm no security warnings appear
- Test all features work

### Step 7: Distribute
- Upload DMG to GitHub release or website
- Update download links on crackinginterview.org
- Share with users

---

## Credentials

```
Apple ID:          navid.vaziri@outlook.com
Team ID:           7JTN2XW63J
App Password:      yvdy-dbhj-dpmh-ajcp
Signing Identity:  Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
Bundle ID:         com.crackinginterview.app
Version:           1.0.0
```

## Apple Developer Support
- Phone: 1-800-633-2152
- Web: https://developer.apple.com/contact/
