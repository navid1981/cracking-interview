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

# 2. Build signed app
npm run tauri build

# 3. Verify signature
codesign -dv src-tauri/target/release/bundle/macos/CrackingInterview.app

# 4. Create ZIP for notarization
cd src-tauri/target/release/bundle/macos
ditto -c -k --keepParent CrackingInterview.app CrackingInterview.zip

# 5. Submit for notarization
xcrun notarytool submit CrackingInterview.zip \
  --apple-id navid.vaziri@outlook.com \
  --team-id Q5T584Q932 \
  --password YOUR-APP-PASSWORD \
  --wait

# 6. Staple ticket
xcrun stapler staple CrackingInterview.app

# 7. Verify notarization
spctl -a -vv CrackingInterview.app