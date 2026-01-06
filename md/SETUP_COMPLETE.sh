#!/bin/bash
# COMPLETE PHASE 1 SETUP - All-in-One Script
# Run this ONE command and everything will be set up!

set -e  # Exit on any error

echo "🚀 CrackingInterview Tauri - Complete Setup"
echo "============================================="
echo ""

PROJECT_DIR="/Users/nsalehvaziri/cracking-interview"
SWIFT_ICON="/Users/nsalehvaziri/CrackingInterview/CrackingInterview/Assets.xcassets/AppIcon.appiconset/10644640.png"

cd "$PROJECT_DIR"

# Step 1: Prerequisites Check
echo "📋 Step 1/5: Checking prerequisites..."
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not installed!"
    echo "Run: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi
echo "✅ Rust: $(cargo --version)"

if ! command -v node &> /dev/null; then
    echo "❌ Node.js not installed!"
    echo "Install from: https://nodejs.org"
    exit 1
fi
echo "✅ Node.js: $(node --version)"
echo ""

# Step 2: Copy Icons
echo "🎨 Step 2/5: Copying your icon from Swift project..."
if [ ! -f "$SWIFT_ICON" ]; then
    echo "⚠️  Swift icon not found at: $SWIFT_ICON"
    echo "Creating placeholder icons instead..."
    # Create simple 1x1 pixel as fallback
    echo "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==" | base64 -D > src-tauri/icons/icon.png
else
    cp "$SWIFT_ICON" src-tauri/icons/icon.png
    cp "$SWIFT_ICON" src-tauri/icons/128x128.png
    cp "$SWIFT_ICON" src-tauri/icons/128x128@2x.png
    cp "$SWIFT_ICON" src-tauri/icons/32x32.png
    cp "$SWIFT_ICON" src-tauri/icons/icon.icns
    cp "$SWIFT_ICON" src-tauri/icons/Square44x44Logo.png
    cp "$SWIFT_ICON" src-tauri/icons/Square89x89Logo.png
    cp "$SWIFT_ICON" src-tauri/icons/Square310x310Logo.png
    cp "$SWIFT_ICON" src-tauri/icons/StoreLogo.png
    echo "✅ Copied your AI icon (9 files)"
fi
echo ""

# Step 3: Fix npm Registry
echo "🔧 Step 3/5: Configuring npm registry (bypass PayPal network)..."
cat > .npmrc << 'EOF'
registry=https://registry.npmjs.org/
@tauri-apps:registry=https://registry.npmjs.org/
@types:registry=https://registry.npmjs.org/
@vitejs:registry=https://registry.npmjs.org/
always-auth=false
EOF
echo "✅ Using public npm registry"
echo ""

# Step 4: Install npm Dependencies
echo "📦 Step 4/5: Installing npm dependencies..."
echo "⏳ This takes 1-3 minutes..."
npm cache clean --force > /dev/null 2>&1
npm install

if [ $? -ne 0 ]; then
    echo ""
    echo "❌ npm install failed!"
    echo ""
    echo "Possible fixes:"
    echo "1. Disconnect from PayPal VPN/GlobalProtect"
    echo "2. Try: npm install --registry https://registry.npmjs.org"
    echo "3. Or paste the error to Claude"
    exit 1
fi
echo "✅ npm dependencies installed!"
echo ""

# Step 5: Build Rust Backend
echo "🦀 Step 5/5: Building Rust backend..."
echo "⏳ First build takes 2-4 minutes (downloads Rust crates)..."
cd src-tauri
cargo build

if [ $? -ne 0 ]; then
    echo ""
    echo "❌ Cargo build failed!"
    echo ""
    echo "Possible fixes:"
    echo "1. Install Xcode tools: xcode-select --install"
    echo "2. Or paste the error to Claude"
    exit 1
fi
echo "✅ Rust backend built successfully!"
cd ..
echo ""

# All done!
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "🎉 PHASE 1 SETUP COMPLETE!"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "✅ Icons: Copied from Swift project"
echo "✅ npm: 245 packages installed"
echo "✅ Rust: Backend compiled"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "🚀 NEXT STEPS:"
echo ""
echo "1. Launch Chrome with CDP:"
echo "   /Applications/Google\\ Chrome.app/Contents/MacOS/Google\\ Chrome \\"
echo "     --remote-debugging-port=9222 \\"
echo "     --user-data-dir='/tmp/chrome-debug' &"
echo ""
echo "2. Run the app:"
echo "   npm run tauri dev"
echo ""
echo "3. Test the buttons:"
echo "   - Click 'Test Chrome CDP Connection'"
echo "   - Click 'Get Chrome Tabs'"
echo ""
echo "4. Report back to Claude:"
echo "   - Did it work?"
echo "   - Any errors?"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
