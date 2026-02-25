in tauri.conf.json
  "build": {
    "frontedDist": "../dist"
  },

run -> npm run build(to create /dist)
-> npm tauri dev


cd /Users/nsalehvaziri/cracking-interview
npm install
npm run build
cd src-tauri
cargo build --release
cargo update
cargo tree
./target/release/cracking-interview

# 1. Check your certificates
security find-identity -v -p codesigning
output:
2AAD596E9F1E575D550A37778BBE5EF07D7A2699 "Developer ID Application: Cracking Interview LLC (7JTN2XW63J)"

# 2. Build signed app
APPLE_SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)" npm run tauri build

# 3. Verify signature
codesign -dv --verbose=2 src-tauri/target/release/bundle/macos/CrackingInterview.app 2>&1 | grep Authority
output:
Authority=Developer ID Application: Cracking Interview LLC (7JTN2XW63J)
Authority=Developer ID Certification Authority
Authority=Apple Root CA

# 4. Create ZIP for notarization
cd src-tauri/target/release/bundle/macos
ditto -c -k --keepParent CrackingInterview.app CrackingInterview.zip

# 5. Submit for notarization
xcrun notarytool submit src-tauri/target/release/bundle/dmg/CrackingInterview_1.0.0_aarch64.dmg \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp \
  --wait

# 6. Staple ticket
xcrun stapler staple CrackingInterview.app

# 7. Verify notarization
spctl -a -vv CrackingInterview.app


------------
history:
xcrun notarytool history \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp

log details:
xcrun notarytool log <SUBMISSION-ID> \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp

xcrun notarytool log 5072adc2-464e-4766-818d-a787cf034640 \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp

----zip creation:
ditto -c -k --keepParent \
  src-tauri/target/release/bundle/macos/CrackingInterview.app \
  /tmp/CrackingInterview.zip

# Submit the ZIP
xcrun notarytool submit /tmp/CrackingInterview.zip \
  --apple-id navid.vaziri@outlook.com \
  --team-id 7JTN2XW63J \
  --password yvdy-dbhj-dpmh-ajcp \
  --wait

------
Apple Developer Support (best option):
Phone: 1-800-633-2152 (US)
Online: https://developer.apple.com/contact/
Hours: Monday–Friday, 5am–5pm PST
When you contact them, say something like:
> "I recently enrolled as a new Developer ID member for Cracking Interview LLC (Team ID: 7JTN2XW63J). I submitted my app for notarization over 24 hours ago and the status is still 'In Progress' with no log available. My submission IDs are:
> - 7928a4a6-f45d-4a48-8c43-0a8cbf926157 (submitted Feb 21)
> - 377570e8-7162-4bc6-a875-f43b0499b506 (submitted Feb 21)
>
> Can you check if there's an issue or expedite the review?"
-----------------------

Option 1: Universal Binary (Recommended)
Build a single DMG that works on both ARM and Intel Macs:
# Add Intel target (one-time setup)
rustup target add x86_64-apple-darwin
# Build universal binary
APPLE_SIGNING_IDENTITY="Developer ID Application: Cracking Interview LLC (7JTN2XW63J)"
npm run tauri build -- --target universal-apple-darwin
This produces one DMG containing a "fat binary" with both architectures. Users don't have to think about which version to download.
Downside: DMG will be roughly 2x the size (~17MB instead of ~8.5MB). Still small.
Option 2: Two Separate DMGs
Build one for each architecture:
# ARM (you already have this)npm run tauri build -- --target aarch64-apple-darwin# Intelnpm run tauri build -- --target x86_64-apple-darwin
Then offer two download buttons on your website: "Download for Mac (Apple Silicon)" and "Download for Mac (Intel)".
Downside: Confusing for non-technical users who don't know their chip type.


--------------------
Easiest method — Call the Comptroller:
Phone: 800-252-5555
Say: "I need to update the NAICS code on my account from 519190 to 511210"