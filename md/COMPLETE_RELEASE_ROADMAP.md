# Complete Release Roadmap

## Current Status

**Last updated:** February 22, 2026

---

## Phase 1: Pre-Build Preparation ✅ COMPLETE

- [x] Code cleanup (52+ debug logs removed)
- [x] Security audit passed
- [x] package.json: version 1.0.0, author, license
- [x] tauri.conf.json: bundle identifier, macOS 11.0+ minimum
- [x] Edge functions deployed to production (removed `-test` suffix)
- [x] Stripe webhook URL updated to production
- [x] Stripe production mode configured
- [x] App tested in dev mode - all features working
- [x] Database verified (users, api_usage tables, RLS policies)

---

## Phase 2: Infrastructure ✅ COMPLETE

- [x] Domain configured: https://crackinginterview.org
- [x] Support email: support@crackinginterview.org
- [x] Website live with homepage, privacy policy, terms of service, user guide
- [x] Dark theme applied across all website pages
- [x] App screenshots integrated into guide page
- [x] Stripe production keys configured in Supabase secrets
- [x] All 5 Supabase edge functions deployed

---

## Phase 3: Apple Developer ✅ COMPLETE

- [x] LLC enrollment completed (Cracking Interview LLC)
- [x] Payment processed ($99/year)
- [x] Developer ID Application certificate created
- [x] Signing identity: `Developer ID Application: Cracking Interview LLC (7JTN2XW63J)`

---

## Phase 4: Build & Sign ✅ COMPLETE

- [x] Production build completed
- [x] App signed with LLC certificate
- [x] Hardened runtime enabled (`flags=0x10000(runtime)`)
- [x] DMG created (8.5MB, aarch64)
- [x] Signature verified (full Apple authority chain)

---

## Phase 5: Notarization ⏳ WAITING

- [x] DMG submitted to Apple notary service
- [x] .app ZIP submitted as alternative
- [ ] **Waiting for Apple to accept** (~6 days for new Developer ID)
- [ ] Staple notarization ticket to DMG
- [ ] Verify notarization (`spctl -a -vv`)

**Submission IDs:**
- `7928a4a6-f45d-4a48-8c43-0a8cbf926157`
- `377570e8-7162-4bc6-a875-f43b0499b506`

---

## Phase 6: Final Testing (After Notarization)

- [ ] Test notarized DMG opens without security warnings
- [ ] Test on a different Mac
- [ ] Verify all features work in production build
- [ ] Test subscription flow with real Stripe

---

## Phase 7: Distribution

- [ ] Upload DMG to GitHub release (v1.0.0)
- [ ] Update download links on crackinginterview.org
- [ ] Test download link works

---

## Phase 8: Launch

- [ ] Soft launch to beta testers
- [ ] Collect feedback and fix critical issues
- [ ] Public launch
- [ ] Post on social media / relevant communities
- [ ] Monitor Supabase logs for errors
- [ ] Respond to user feedback

---

## Cost Breakdown (Annual)

| Item | Cost |
|------|------|
| Apple Developer Program | $99/year |
| Supabase | Free tier (or $25/month for Pro) |
| Stripe | 2.9% + $0.30 per transaction |
| Domain | ~$10-15/year |
| **Total minimum** | **~$110/year + transaction fees** |
