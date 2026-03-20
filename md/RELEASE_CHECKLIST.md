# Release Checklist

## Code & Configuration ✅ COMPLETE

- [x] Removed all `console.log` and `invoke('frontend_log')` debug statements
- [x] Kept `console.error` for critical error handling
- [x] Fixed all linter errors and warnings
- [x] Removed unused variables and imports
- [x] No hardcoded secrets or test data in codebase
- [x] package.json: version 1.0.0, author, license
- [x] tauri.conf.json: correct bundle ID, macOS 11.0+, targets: app + dmg
- [x] Edge functions deployed to production (no `-test` suffix)

## Backend & Services ✅ COMPLETE

- [x] Supabase edge functions deployed (ai-proxy, create-checkout, create-billing-portal, notification, stripe-webhook)
- [x] Stripe webhook URL pointing to production endpoint
- [x] Stripe production keys in Supabase secrets
- [x] Database tables verified (users, api_usage, RLS policies)
- [x] Domain & email configured (crackinginterview.org, support@crackinginterview.org)

## Build & Signing ✅ COMPLETE

- [x] Apple Developer enrollment (Cracking Interview LLC)
- [x] Developer ID Application certificate created (Team ID: 7JTN2XW63J)
- [x] Production build completed with code signing
- [x] Hardened runtime enabled
- [x] Audio helper binary (`audio_recorder_bin`) bundled and signed with Developer ID
- [x] DMG generated (8.5MB)
- [x] Signature verified (full Apple authority chain)

## Notarization ⏳ PENDING

- [x] Submitted to Apple notary service
- [ ] Status: Accepted
- [ ] Ticket stapled to DMG
- [ ] Verified with `spctl -a -vv`

## Testing (After Notarization)

- [ ] DMG opens without security warnings
- [ ] Test on a different Mac
- [ ] Sign up / Sign in works
- [ ] Free user: 3 AI calls, then paywall
- [ ] Pro user: subscription flow via Stripe
- [ ] All 6 default prompts work
- [ ] Custom prompt create / edit / rename / delete
- [ ] Text extraction works
- [ ] Screenshot capture works
- [ ] Audio recording works (warm-up + file recording)
- [ ] Live transcription works (stream mode via Deepgram)
- [ ] Chrome CDP connects
- [ ] All global hotkeys functional
- [ ] Stealth mode (hide from screen sharing)
- [ ] Always on top works
- [ ] Settings save correctly
- [ ] "Remember me" pre-fills credentials
- [ ] Terms/Privacy links open in default browser

## Distribution

- [ ] Upload DMG to GitHub release (v1.0.0)
- [ ] Update download links on crackinginterview.org
- [ ] Test download and install from link

## Website ✅ COMPLETE

- [x] Homepage (index.html) - features, pricing, download buttons
- [x] Privacy Policy (privacy.html)
- [x] Terms of Service (terms.html)
- [x] User Guide (guide.html) - with app screenshots
- [x] Shared dark theme (style.css)
- [x] App icon and branding
