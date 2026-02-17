# 🚀 QUICK REFERENCE - Release Day Commands

## 📋 Pre-Flight Checklist

```bash
# Verify you're ready
✅ LLC certificate created in Xcode
✅ Code is clean and tested
✅ Version number is correct (1.0.0)
```

---

## 🏗️ BUILD COMMANDS

### Clean Build (Fresh Start)
```bash
cd /Users/nsalehvaziri/cracking-interview
rm -rf src-tauri/target/release
npm run tauri build
```

**Wait for:**
```
Finished release [optimized] target(s) in 2m 34s
   Bundling CrackingInterview.app...
   Bundling CrackingInterview_1.0.0_x64.dmg...
```

**Output Location:**
```
src-tauri/target/release/bundle/macos/CrackingInterview.app
src-tauri/target/release/bundle/dmg/CrackingInterview_1.0.0_x64.dmg
```

---

## ✅ VERIFY SIGNATURE

### Check Code Signature
```bash
codesign -dv --verbose=4 src-tauri/target/release/bundle/macos/CrackingInterview.app
```

**Look for:**
```
Authority=Developer ID Application: Cracking Interview LLC (Q5T584Q932)
TeamIdentifier=Q5T584Q932
```

✅ If you see "Cracking Interview LLC", you're good!

### Check Entitlements
```bash
codesign -d --entitlements - src-tauri/target/release/bundle/macos/CrackingInterview.app
```

---

## 🔐 NOTARIZE (Automated)

### Run Notarization Script
```bash
./notarize.sh
```

**Expected Output:**
```
🔐 Notarizing CrackingInterview...
📦 Step 1: Creating ZIP...
✅ ZIP created

🚀 Step 2: Submitting to Apple...
   (This usually takes 5-15 minutes)
   
✅ Successfully received submission info
✅ Status: Accepted

✅ Notarization successful!
🎫 Step 3: Stapling ticket...
✅ Ticket stapled!

🔍 Step 4: Verifying...
CrackingInterview.app: accepted
source=Notarized Developer ID

🎉 SUCCESS!
```

---

## 🔐 MANUAL NOTARIZATION (If Script Fails)

### Step 1: Create ZIP
```bash
cd src-tauri/target/release/bundle/macos
ditto -c -k --keepParent CrackingInterview.app CrackingInterview.zip
```

### Step 2: Submit to Apple
```bash
xcrun notarytool submit CrackingInterview.zip \
  --apple-id navid.vaziri@outlook.com \
  --team-id Q5T584Q932 \
  --password yvdy-dbhj-dpmh-ajcp \
  --wait
```

### Step 3: Staple Ticket
```bash
xcrun stapler staple CrackingInterview.app
```

### Step 4: Verify
```bash
spctl -a -vv CrackingInterview.app
```

**Should show:**
```
CrackingInterview.app: accepted
source=Notarized Developer ID
```

---

## 🐛 TROUBLESHOOTING

### "Certificate not found"
```bash
# List available certificates
security find-identity -v -p codesigning

# Should show your LLC certificate
# If not, go back to Xcode and create it
```

### "Notarization failed"
```bash
# Get detailed error log
xcrun notarytool log [submission-id] \
  --apple-id navid.vaziri@outlook.com \
  --team-id Q5T584Q932 \
  --password yvdy-dbhj-dpmh-ajcp
```

### Build fails
```bash
# Clean everything
rm -rf node_modules
rm -rf src-tauri/target
npm install
npm run tauri build
```

### "Command not found: xcrun"
```bash
# Install Xcode Command Line Tools
xcode-select --install
```

---

## 📦 CREATE DISTRIBUTION PACKAGE

### Option 1: Use DMG (Recommended)
```bash
# Already created by Tauri!
cd src-tauri/target/release/bundle/dmg
ls -lh CrackingInterview_1.0.0_x64.dmg
```

### Option 2: Create ZIP
```bash
cd src-tauri/target/release/bundle/macos
zip -r CrackingInterview-1.0.0-macOS.zip CrackingInterview.app
```

---

## 🧪 TEST ON CLEAN SYSTEM

### Verify App Works
```bash
# On a different Mac (or after clean install):
1. Download the DMG/ZIP
2. Open and drag to Applications
3. Launch CrackingInterview
4. Verify no security warnings
5. Test all features
```

### Test Checklist
```
□ App launches without warnings
□ Sign up / Sign in works
□ All hotkeys work
□ Screenshot capture works
□ Audio recording works
□ Chrome CDP connection works
□ Settings save correctly
□ Stealth mode works (Command+Shift+H)
□ Free tier: 3 calls work, then paywall
□ Pro tier: Subscribe via Stripe
```

---

## 🌐 UPLOAD TO GITHUB RELEASES

### Create Release
```bash
# 1. Go to GitHub repo
https://github.com/[your-username]/cracking-interview/releases

# 2. Click "Create a new release"

# 3. Fill in:
Tag:         v1.0.0
Title:       CrackingInterview v1.0.0 - First Release! 🎉
Description: Copy from RELEASE_NOTES_v1.0.0.md

# 4. Upload files:
- CrackingInterview_1.0.0_x64.dmg (recommended)
- CrackingInterview-1.0.0-macOS.zip (alternative)

# 5. Publish release!
```

### Share Download Link
```
Direct link:
https://github.com/[username]/cracking-interview/releases/download/v1.0.0/CrackingInterview_1.0.0_x64.dmg
```

---

## 📊 FILE SIZES (Approximate)

```
CrackingInterview.app:           ~50MB
CrackingInterview_1.0.0_x64.dmg: ~30MB (compressed)
CrackingInterview.zip:           ~45MB
```

---

## 🔑 YOUR CREDENTIALS (Quick Reference)

### Apple Developer
```
Enrollment ID:   Q5T584Q932
Apple ID:        navid.vaziri@outlook.com
Team ID:         Q5T584Q932
App Password:    yvdy-dbhj-dpmh-ajcp
LLC Name:        Cracking Interview LLC
```

### App Details
```
Product Name:    CrackingInterview
Bundle ID:       com.crackinginterview.app
Version:         1.0.0
Min macOS:       11.0 (Big Sur)
```

---

## ⏱️ TIMELINE (Day of Approval)

```
Time    | Task                      | Duration
--------|---------------------------|----------
0:00    | Receive approval email    | -
0:05    | Pay $99                   | 5 min
0:07    | Create certificate        | 2 min
0:12    | Build app                 | 5 min
0:17    | Run notarize.sh           | 15 min
0:32    | Test on clean Mac         | 10 min
0:42    | Create GitHub release     | 5 min
0:47    | Share with users!         | 🎉
```

**Total: ~45 minutes from approval to shipping!**

---

## 📞 SUPPORT CONTACTS

### Apple Developer Support
- **Phone:** 1-800-633-2152
- **Web:** https://developer.apple.com/contact/
- **Enrollment ID:** Q5T584Q932

### Emergency Commands
```bash
# Check certificate exists
security find-identity -v -p codesigning

# Verify app signature
codesign -dv src-tauri/target/release/bundle/macos/CrackingInterview.app

# Check notarization status
spctl -a -vv src-tauri/target/release/bundle/macos/CrackingInterview.app

# List all processes
ps aux | grep CrackingInterview
```

---

## 💡 TIPS

### Before Building
- ✅ Close all dev instances of the app
- ✅ Clear browser cache if testing web features
- ✅ Test in dev mode first: `npm run tauri dev`

### After Building
- ✅ Test on a different Mac if possible
- ✅ Have someone else try installing it
- ✅ Check that all links in release notes work

### For Future Releases
- Update version in: `package.json` AND `src-tauri/tauri.conf.json`
- Update: `RELEASE_NOTES_v[version].md`
- Tag in git: `git tag v1.0.1`
- Same process: build → notarize → release

---

## 🎯 ONE-LINER COMMANDS

```bash
# Full build + notarize
npm run tauri build && ./notarize.sh

# Verify everything
codesign -dv src-tauri/target/release/bundle/macos/CrackingInterview.app && \
spctl -a -vv src-tauri/target/release/bundle/macos/CrackingInterview.app

# Find DMG
find . -name "*.dmg" -type f

# Check app size
du -sh src-tauri/target/release/bundle/macos/CrackingInterview.app
```

---

**🚀 You've got this! Everything is ready to go!**

Just follow these commands when your LLC is approved, and you'll be shipping in 30 minutes! 🎉

---

*Keep this file handy - you'll refer to it on release day!*

