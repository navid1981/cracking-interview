# Build Instructions - macOS and Windows

## ✅ macOS Build - DONE!

**Location:**
```
/Users/nsalehvaziri/cracking-interview/src-tauri/target/release/bundle/dmg/CrackingInterview_1.0.0_aarch64.dmg
```

**Size:** 4.0 MB
**Format:** .dmg installer for macOS (Apple Silicon)

**To test:**
```bash
open /Users/nsalehvaziri/cracking-interview/src-tauri/target/release/bundle/dmg/
```

Double-click the DMG to test the installer!

---

## 🪟 Windows Build - Two Options

### Option 1: Build on Windows Machine/VM (Recommended)

**You need:**
- Windows 10/11 PC or VM
- Or Windows VM on your Mac (Parallels, VMware, VirtualBox)

**Steps on Windows:**
```bash
# 1. Install prerequisites
# - Install Rust: https://rustup.rs
# - Install Node.js: https://nodejs.org
# - Install Visual Studio Build Tools

# 2. Copy project to Windows
# (via USB, cloud, or shared folder)

# 3. Build
cd cracking-interview
npm install
npm run tauri build

# Output:
# CrackingInterview_1.0.0_x64.msi (Windows installer)
```

---

### Option 2: Cross-Compile from macOS (Advanced)

**This is tricky and not officially supported by Tauri.**

Tauri doesn't support cross-compilation from macOS to Windows because:
- Windows .msi requires Windows-specific tools
- Code signing needs Windows
- Testing needs actual Windows

**Not recommended for now.**

---

## 🎯 Recommended Approach for Testing

### For macOS Testing:
✅ **Already built!** Use the DMG:
```
CrackingInterview_1.0.0_aarch64.dmg
```

### For Windows Testing:

**Option A: Use Windows VM on your Mac**
1. Install Parallels Desktop or VirtualBox
2. Create Windows 11 VM
3. Copy project folder to VM
4. Install Rust + Node.js in VM
5. Run `npm run tauri build` in VM

**Option B: Use a Windows Machine**
1. Borrow a Windows laptop/PC
2. Copy project (USB drive or cloud)
3. Install Rust + Node.js
4. Run `npm run tauri build`

**Option C: GitHub Actions (Automated)**
I can set up CI/CD to build Windows version automatically!

---

## 💡 Quick Windows VM Setup

If you want to test on Windows, here's the fastest way:

### Using Parallels (Paid, $99/year):
```bash
# Install Parallels
brew install --cask parallels

# Download Windows 11 ARM
# Create VM
# Install in 20 minutes
```

### Using UTM (Free):
```bash
# Install UTM
brew install --cask utm

# Download Windows 11 ARM ISO
# Create VM
# Install in 30 minutes
```

---

## 🚀 GitHub Actions for Automatic Windows Build

Want me to set up CI/CD? I can create `.github/workflows/build.yml` that:
- Builds macOS .dmg automatically
- Builds Windows .msi automatically
- Uploads both as release assets

**Every commit = fresh installers for both platforms!**

Say "yes" and I'll set it up!

---

## 📦 Current Status

✅ **macOS installer:** Ready at `src-tauri/target/release/bundle/dmg/CrackingInterview_1.0.0_aarch64.dmg`

⏳ **Windows installer:** Need Windows machine to build

---

## 🎯 What Do You Want to Do?

**Option 1:** Test macOS DMG now (already built!)
**Option 2:** Set up Windows VM to build Windows version
**Option 3:** Set up GitHub Actions for automatic builds
**Option 4:** Continue development, worry about Windows builds later

**Your preference?** 🚀
