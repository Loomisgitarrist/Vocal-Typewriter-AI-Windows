#!/bin/bash

# Exit on error
set -e

echo "🍎 Preparing for Mac App Store Build (Universal)..."

# 1. Define paths
ENTITLEMENTS_PATH="src-tauri/Entitlements.plist"
ENTITLEMENTS_APPSTORE_PATH="src-tauri/Entitlements.AppStore.plist"
ENTITLEMENTS_BACKUP_PATH="src-tauri/Entitlements.plist.backup"

# 2. Backup existing entitlements (Direct Download version)
echo "📦 Backing up Direct Download entitlements..."
cp "$ENTITLEMENTS_PATH" "$ENTITLEMENTS_BACKUP_PATH"

# 3. Swap in App Store entitlements (Sandboxed)
echo "🔒 Applying App Store Sandbox entitlements..."
cp "$ENTITLEMENTS_APPSTORE_PATH" "$ENTITLEMENTS_PATH"

# 4. Run the Build (Universal: Intel + Silicon)
echo "🔨 Building App (Universal)..."
# Using --target universal-apple-darwin to create a single app bundle supporting both architectures
npm run tauri build -- --target universal-apple-darwin

# 5. Restore original entitlements
echo "♻️  Restoring Direct Download entitlements..."
mv "$ENTITLEMENTS_BACKUP_PATH" "$ENTITLEMENTS_PATH"

echo "✅ Build Complete!"
echo "   You can find the build in src-tauri/target/universal-apple-darwin/release/bundle/macos/"
echo "   Note: Use package_with_temp_keychain.sh to sign and package it for the App Store."
