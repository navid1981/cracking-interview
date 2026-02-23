# Release Documentation

## Files

| File | Purpose |
|------|---------|
| **LLC_APPROVAL_GUIDE.md** | Release progress tracker with current status and remaining steps |
| **QUICK_REFERENCE.md** | Copy-paste commands for build, signing, notarization |
| **COMPLETE_RELEASE_ROADMAP.md** | Full roadmap from code to launch with phase tracking |
| **RELEASE_CHECKLIST.md** | Comprehensive checklist with completion status |
| **RELEASE_NOTES_v1.0.0.md** | User-facing release notes for v1.0.0 |

## Current Status

- **Build & Signing:** Complete
- **Notarization:** Waiting for Apple (~6 days for new Developer ID)
- **Next step:** Staple ticket and verify after notarization accepted

## Quick Commands

```bash
# Check notarization status
xcrun notarytool history \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp

# After accepted - staple ticket
xcrun stapler staple src-tauri/target/release/bundle/dmg/CrackingInterview_1.0.0_aarch64.dmg

# Rebuild (if needed)
APPLE_SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" npm run tauri build
```
