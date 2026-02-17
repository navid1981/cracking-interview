# CrackingInterview Release Checklist

## ✅ Code Cleanup (COMPLETED)

- [x] Removed all `console.log` debug statements from all files
- [x] Removed all `console.warn` debug statements  
- [x] Removed all `invoke('frontend_log')` development logging
- [x] Kept `console.error` for critical error handling
- [x] Fixed all linter errors and warnings
- [x] Removed unused variables

**Files Cleaned:**
- `src/App.tsx` - Removed 46+ debug logs
- `src/components/AIResponseDisplay.tsx` - Removed 7 logs
- `src/components/SignInForm.tsx` - Removed 4 logs (kept 1 console.error)
- `src/services/supabase.ts` - Removed 21 logs (kept console.error)

## 📋 Pre-Release Configuration

### package.json
Current version: **1.0.0**

```json
{
  "name": "cracking-interview",
  "private": true,
  "version": "1.0.0",
  "type": "module"
}
```

**Before Real Release:**
- [ ] Update version number (e.g., 1.0.1, 1.1.0)
- [ ] Add proper author field
- [ ] Add description
- [ ] Add license

### Supabase Configuration
**Current:** Using `-test` suffix edge functions

**Before Real Release:**
- [ ] Deploy edge functions without `-test` suffix
- [ ] Update `SUPABASE_URL` if using different project
- [ ] Test all edge functions in production

### Tauri Configuration
Location: `src-tauri/tauri.conf.json`

**To Review:**
- [ ] Bundle identifier is correct
- [ ] App version matches package.json
- [ ] App icons are set for all platforms
- [ ] Code signing configuration (macOS/Windows)
- [ ] Auto-updater setup (optional)

## 🔒 Security Audit

### API Keys
- [x] **Supabase Anon Key** - ✅ SAFE (public key, designed to be exposed)
- [x] **No private keys** in codebase
- [x] **No hardcoded passwords** or secrets

### Data Privacy
- [x] No user emails logged to console
- [x] No subscription data logged
- [x] API errors don't leak sensitive info

### Edge Functions
- [x] Protected with RLS (Row Level Security)
- [x] Require authentication headers
- [x] No SQL injection vulnerabilities

## 🧪 Testing Checklist

### Authentication
- [ ] Sign up with new email
- [ ] Sign in with existing account  
- [ ] Sign out
- [ ] Session persistence after restart
- [ ] Remembered email works

### Free User Flow
- [ ] Can use 3 AI calls
- [ ] Blocked after 3 calls with paywall message
- [ ] Only allowed domains work (LeetCode, HackerRank, Codeforces)
- [ ] Cannot access Display Capture

### Pro User Flow
- [ ] Subscription checkout opens in browser
- [ ] After payment, subscription_status updates
- [ ] 150 AI calls per month limit
- [ ] Display Capture unlocked
- [ ] All AI models available

### Input Sources
- [ ] Chrome tab selection works
- [ ] Display capture works (Pro only)
- [ ] Audio recording works
- [ ] Screenshot capture works
- [ ] Text extraction works

### AI Features
- [ ] All 6 default prompts work correctly
- [ ] Custom prompt creation (max 3)
- [ ] Custom prompt editing
- [ ] Custom prompt duplication
- [ ] Custom prompt deletion
- [ ] Language selection (Java, Python, JS, C++, Swift)

### Hotkeys (macOS)
- [ ] `Command+1` - Extract text
- [ ] `Command+2` - Screenshot
- [ ] `Command+3` - Audio toggle
- [ ] `Command+Shift+H` - Toggle visibility (Stealth)
- [ ] `Command+Q` - Quit app
- [ ] Scroll hotkeys work
- [ ] Move window hotkeys work
- [ ] Can customize hotkeys in Settings

### Edge Cases
- [ ] Chrome not running → Opens Chrome automatically
- [ ] No input source selected → Shows error
- [ ] Network error → Shows error message
- [ ] Announcement system works
- [ ] Audio prompt warning when non-audio source selected
- [ ] Window resize on settings open/close

## 🚀 Build Process

### Pre-Build Steps
1. Clean the project:
```bash
npm run clean  # or rm -rf dist node_modules
npm install
```

2. Test development build:
```bash
npm run tauri dev
```

3. Create production build:
```bash
npm run tauri build
```

### macOS Build Output
Location: `src-tauri/target/release/bundle/`

Files generated:
- `.app` - Application bundle
- `.dmg` - Disk image installer (if configured)

### Code Signing (macOS)
**Required for distribution outside development machines**

1. Obtain Apple Developer Certificate
2. Configure in `tauri.conf.json`:
```json
{
  "bundle": {
    "macOS": {
      "signingIdentity": "Developer ID Application: Your Name (TEAM_ID)"
    }
  }
}
```

3. Notarize the app:
```bash
xcrun notarytool submit CrackingInterview.dmg \
  --apple-id your-apple-id@email.com \
  --team-id YOUR_TEAM_ID \
  --password app-specific-password \
  --wait
```

## 📦 Distribution

### Direct Download
- Upload `.dmg` to your website
- Provide installation instructions
- Include system requirements

### App Store (Optional)
- Requires additional Apple review process
- Different code signing requirements
- Sandboxing restrictions may apply

## 🐛 Known Issues & Limitations

### Environment-Specific
- **PayPal Proxy**: Some corporate networks may block Supabase/npm
- **SSL Inspection**: Corporate SSL inspection may cause auth issues (handled by Tauri backend bypass)

### Platform Support
- **macOS**: Fully supported ✅
- **Windows**: Not tested yet ⚠️
- **Linux**: Not configured ❌

## 📝 Release Notes Template

```markdown
# CrackingInterview v1.0.0

## 🎉 Features
- AI-powered coding interview practice
- Support for LeetCode, HackerRank, Codeforces
- 6 default prompt templates + 3 custom prompts
- Audio recording for verbal interview practice
- Chrome tab capture
- Display capture (Pro)
- Stealth mode (Command+Shift+H)
- Customizable hotkeys

## 💎 Free Tier
- 3 AI calls
- Text extraction
- Screenshot capture
- Audio recording
- Allowed domains: LeetCode, HackerRank, Codeforces

## ⭐ Pro Tier ($9.99/month)
- 150 AI calls per month
- Display capture
- All AI models (GPT-4, Claude, Gemini)
- All domains supported

## 🔧 System Requirements
- macOS 11.0 or later
- Chrome browser (for web scraping)

## 🚀 Installation
1. Download CrackingInterview.dmg
2. Open the DMG file
3. Drag CrackingInterview to Applications
4. Launch and sign in

## 📖 Documentation
Full documentation: [Your Docs URL]

## 🐛 Bug Reports
Report issues: [Your GitHub Issues URL]
```

## ✅ Final Checklist

Before shipping v1.0.0:

- [ ] All tests passed
- [ ] Build successful on clean machine
- [ ] Code signed and notarized (macOS)
- [ ] Release notes written
- [ ] Documentation updated
- [ ] Support email/contact set up
- [ ] Backup of Supabase database
- [ ] Monitoring/analytics configured (optional)

**🎯 You are here:** Code cleanup complete, ready for testing phase.

**Next steps:**
1. Test all features manually
2. Update Supabase to production (remove `-test`)
3. Configure code signing
4. Build release version
5. Test on clean macOS install
6. Ship! 🚀

