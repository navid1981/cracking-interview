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