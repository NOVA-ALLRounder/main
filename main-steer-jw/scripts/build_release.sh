#!/bin/bash

# Steer Agent - Release Build Script

echo "🚀 [Steer Agent] Starting Release Build Process..."
echo "---------------------------------------------------"

# Get script directory to ensure correct paths
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$SCRIPT_DIR/.."
APPS_ROOT="$PROJECT_ROOT/apps"

# 1. Build Frontend
echo "📦 [1/3] Building Web Frontend..."
cd "$APPS_ROOT/web"
npm install
npm run build
if [ $? -ne 0 ]; then
    echo "❌ Frontend build failed."
    exit 1
fi

# 2. Build Tauri Desktop App
echo "🦀 [2/3] Building Desktop App (Rust + Tauri)..."
cd "$APPS_ROOT/desktop"
# Ensure tauri-cli is available or use cargo tauri
if command -v cargo-tauri &> /dev/null; then
    cargo tauri build
else
    cargo install tauri-cli --version "^2.0.0" --locked
    cargo tauri build
fi

if [ $? -ne 0 ]; then
    echo "❌ Desktop build failed."
    exit 1
fi

# 3. Locate Artifacts
echo "🎉 [3/3] Build Complete!"
echo "---------------------------------------------------"
echo "✅ Release Bundle Location:"
find src-tauri/target/release/bundle -maxdepth 2 -type d 2>/dev/null || echo "   (Check apps/desktop/src-tauri/target/release/bundle)"

echo "---------------------------------------------------"
echo "👉 You can now distribute the app from the folder above."
