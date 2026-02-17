# Release Cleanup Plan for CrackingInterview

## Summary
Found **46+ debug log statements** in `src/App.tsx` that should be removed for production.

## Logs to Remove

### Category 1: Development Debug Logs (Remove All)
- `console.log('[App] ...')` - Authentication/subscription logs
- `console.log('🎯 ...')` - Settings open/close logs
- `console.log('📏 ...')` - Window resize logs  
- `console.log('💾 ...')` - Size save logs
- `console.log('✅ ...')` - Success confirmation logs
- `console.log('⚙️ ...')` - Settings logs
- `console.log('[Subscribe] ...')` - Subscription flow logs
- `console.log('[Announcement] ...')` - Announcement fetch logs
- `console.log('🔥 ...')` - Audio warm-up logs
- `console.log('❄️ ...')` - Audio cool-down logs
- `console.log('ℹ️  ...')` - Info logs
- `console.log('🔄 ...')` - Restore logs
- `console.log('Auth state changed:', event)` - Auth state logs
- `invoke('frontend_log', {...})` - All Tauri frontend logging calls (22 instances)

### Category 2: Non-Critical Warnings (Remove - Already Handled in UI)
- `console.warn('Failed to persist AI config:', e)` ✅ DONE
- `console.warn('Failed to persist input mode:', e)` ✅ DONE  
- `console.warn('Failed to listen for hotkey solve events:', e)` - Already shows error message
- `console.warn('Failed to load hotkeys:', e)` - Already shows in UI
- `console.warn('❌ resize_window command failed:', e)` - Non-critical
- `console.warn('Failed to read size after resize:', e)` - Non-critical
- `console.warn('Failed to get display thumbnail...')` - Non-critical, continues without thumbnail
- `console.warn('Failed to get thumbnail for tab...')` - Non-critical, continues without thumbnail
- `console.warn('⚠️ Audio warm-up failed:', e)` - Non-critical
- `console.warn('Failed to listen for hotkey solve events:', e)` - Non-critical

### Category 3: Critical Error Logs (KEEP)
- `console.error` statements for user-facing errors
- Error handling that affects user experience

## Files to Clean

### Priority 1: Main App
- [x] `src/App.tsx` - 46+ log statements

### Priority 2: Components (Need to check)
- [x] `src/components/AIResponseDisplay.tsx` ✅ Clean
- [x] `src/components/TabDropdown.tsx` ✅ Clean
- [x] `src/components/AuthScreen.tsx` ✅ Clean
- [x] `src/components/PromptEditor.tsx` ✅ Clean
- [x] `src/components/PromptListView.tsx` ✅ Clean

### Priority 3: Services (Need to check)
- [x] `src/services/supabase.ts` ✅ Clean
- [x] `src/services/prompts.ts` ✅ Clean

## Other Cleanup Tasks

### Remove Commented Code
- [x] Search for `//` blocks that are commented out ✅ None found
- [x] Remove old dead code ✅ None found

### Remove Unused Imports
- [x] Run linter to detect unused imports ✅ None found
- [x] Clean up unnecessary dependencies ✅ All clean

### Security Audit
- [x] Supabase anon key - OK (public key, safe to expose) ✅
- [x] Check for any hardcoded test data ✅ None found
- [x] Verify no email/user data in logs ✅ Clean
- [x] Review API error messages don't leak sensitive info ✅ All good

## Configuration Updates

### package.json
```json
{
  "name": "cracking-interview",
  "version": "1.0.0",  // Update before release
  "description": "AI-powered coding interview practice tool",
  "author": "Your Name",
  "license": "MIT"  // Or your license
}
```

### Tauri Config (src-tauri/tauri.conf.json)
- Set `bundle.identifier` correctly
- Configure code signing
- Set up auto-updater (if needed)
- Configure app icons

## Testing Checklist
- [ ] Test with fresh macOS install
- [ ] Test free user signup → 3 AI calls → paywall
- [ ] Test pro user subscription flow
- [ ] Test all hotkeys (text, screenshot, audio, scroll, move, visibility, quit)
- [ ] Test stealth mode (Command+Shift+H)
- [ ] Test audio recording
- [ ] Test all 6 default prompts
- [ ] Test custom prompts (create, edit, duplicate, delete)
- [ ] Test announcement system
- [ ] Test Chrome CDP connection
- [ ] Test display capture (pro only)

## Build & Distribution (For Real Release)

### Pre-Build
1. Update Supabase edge functions to production (remove "-test" suffix)
2. Update version number in package.json
3. Update changelog/release notes

### Build
```bash
npm run tauri build
```

### Post-Build
1. Code signing (macOS: Apple Developer Certificate)
2. Notarization (macOS required for distribution)
3. Create DMG installer
4. Test on clean system
5. Upload to distribution platform

## Estimated Effort
- Code cleanup: 1-2 hours
- Testing: 2-3 hours  
- Build setup: 1-2 hours
- Total: 4-7 hours

## Questions for You
1. Should I proceed with removing all debug logs in App.tsx now?
2. Do you want me to check all component files for logs too?
3. What's your target release date?
4. Do you need help setting up code signing?

