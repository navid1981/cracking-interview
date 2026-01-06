# How to Run CrackingInterview Tauri App

## 🚀 Quick Start

### From Terminal:
```bash
cd /Users/nsalehvaziri/cracking-interview
npm run tauri dev
```

**Window opens in 5-10 seconds!**

---

### From Cursor IDE:
1. Open integrated terminal (Ctrl+`)
2. Run: `npm run tauri dev`

---

## ⚡ Commands Explained

### Development Mode (Hot Reload):
```bash
npm run tauri dev
```
- Opens app window
- Frontend auto-reloads on file changes
- Backend recompiles on Rust changes (2-5 seconds)
- Best for development

### Build Production Version:
```bash
npm run tauri build
```
- Creates .dmg installer
- Optimized and minified
- Ready to distribute

### Just Build Frontend:
```bash
npm run build
```
- Only builds React app
- Creates `dist/` folder

---

## 🔄 When to Restart

**Frontend changes (src/*.tsx, src/*.css):**
- Auto-reloads! No restart needed ✅

**Backend changes (src-tauri/src/*.rs):**
- Recompiles automatically (wait 2-5s) ✅

**Config changes (tauri.conf.json, Cargo.toml):**
- Stop app (Ctrl+C)
- Run `npm run tauri dev` again

---

## 🛑 How to Stop

- Press `Ctrl+C` in Terminal
- Or close the app window

---

## ✅ App is Running Now!

I just started it for you in Terminal. Check your screen! 🎉
