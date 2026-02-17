#!/bin/bash

# Kill all related processes
echo "🔴 Stopping all processes..."
pkill -9 -f "cracking-interview" 2>/dev/null
pkill -9 -f "tauri dev" 2>/dev/null
lsof -ti:1420 | xargs kill -9 2>/dev/null

# Wait a moment
sleep 2

# Show current visibility mode
echo ""
echo "📋 Current .env setting:"
grep APP_VISIBILITY .env

echo ""
echo "🚀 Starting app..."
npm run tauri dev

