#!/bin/bash
set -e

KEYCHAIN_NAME="build.keychain"
KEYCHAIN_PASS="build"
P12_PASS="1234"

# Paths
# Find the app bundle dynamically
# Prioritize Universal, then aarch64, then x86_64
APP_PATH=$(find src-tauri/target -path "*/universal-apple-darwin/*/bundle/macos/Vocal Typewriter.app" -type d | head -n 1)

if [ -z "$APP_PATH" ]; then
    APP_PATH=$(find src-tauri/target -name "Vocal Typewriter.app" -type d | grep "release/bundle/macos" | head -n 1)
fi

if [ -z "$APP_PATH" ]; then
    echo "❌ Error: Could not find 'Vocal Typewriter.app' in src-tauri/target/**/release/bundle/macos/"
    echo "Please run 'bun tauri build' first."
    exit 1
fi

echo "✅ Found App Bundle at: $APP_PATH"

ENTITLEMENTS_PATH="src-tauri/Entitlements.AppStore.plist"
PKG_OUTPUT="VocalTypewriter.pkg"

# Identities
APP_CERT="signing/mac_app.cer"
INSTALLER_CERT="signing/mac_installer.cer"
PRIVATE_KEY="Certificates/privateKey.key"
PROVISIONING_PROFILE="signing/Vocal_Typewriter_App_Store.provisionprofile"

APP_IDENTITY="3rd Party Mac Developer Application: Loomis Connell Green (555HHF3KGU)"
INSTALLER_IDENTITY="3rd Party Mac Developer Installer: Loomis Connell Green (555HHF3KGU)"

# Cleanup function
cleanup() {
    echo "Cleaning up..."
    security delete-keychain "$KEYCHAIN_NAME" || true
    rm -f mac_app.pem mac_installer.pem full_cert_bundle.p12 AppleWWDRCAG3.cer
}
trap cleanup EXIT

echo "Patching App Bundle..."
# Update Info.plist with required keys
plutil -replace LSMinimumSystemVersion -string "10.15" "$APP_PATH/Contents/Info.plist"
plutil -replace LSApplicationCategoryType -string "public.app-category.productivity" "$APP_PATH/Contents/Info.plist"

# Update Icon
echo "Updating Icon..."
cp "src-tauri/icons/icon.icns" "$APP_PATH/Contents/Resources/icon.icns"

# Embed Provisioning Profile
echo "Embedding Provisioning Profile..."
cp "$PROVISIONING_PROFILE" "$APP_PATH/Contents/embedded.provisionprofile"

# Remove quarantine attributes
echo "Removing quarantine attributes..."
xattr -d com.apple.quarantine "$APP_PATH/Contents/embedded.provisionprofile" || true
xattr -d com.apple.quarantine "$APP_PATH/Contents/Resources/icon.icns" || true
xattr -d com.apple.quarantine "$APP_PATH/Contents/Info.plist" || true
xattr -rc "$APP_PATH"

echo "Verifying quarantine removal..."
xattr -l "$APP_PATH/Contents/embedded.provisionprofile"


echo "Creating temporary keychain..."
security create-keychain -p "$KEYCHAIN_PASS" "$KEYCHAIN_NAME"
security set-keychain-settings -t 3600 -u "$KEYCHAIN_NAME"
# Add to search list so codesign/productbuild can find it
security list-keychains -d user -s "$KEYCHAIN_NAME" $(security list-keychains -d user | sed 's/"//g')

echo "Downloading Apple WWDR Intermediate Certificate (G3)..."
curl -s -o AppleWWDRCAG3.cer https://www.apple.com/certificateauthority/AppleWWDRCAG3.cer
echo "Importing Apple WWDR Intermediate Certificate..."
security import AppleWWDRCAG3.cer -k "$KEYCHAIN_NAME" -T /usr/bin/codesign -T /usr/bin/productbuild

echo "Converting certificates..."
openssl x509 -inform DER -in "$APP_CERT" -out mac_app.pem
openssl x509 -inform DER -in "$INSTALLER_CERT" -out mac_installer.pem

echo "Creating P12 bundle..."
# Bundle private key and both certs (using -legacy for macOS Keychain compatibility)
openssl pkcs12 -export -legacy -inkey "$PRIVATE_KEY" -in mac_app.pem -certfile mac_installer.pem -out full_cert_bundle.p12 -passout pass:"$P12_PASS" -name "SpeakToMeLG_Identities"

echo "Importing identities..."
security import full_cert_bundle.p12 -k "$KEYCHAIN_NAME" -P "$P12_PASS" -T /usr/bin/codesign -T /usr/bin/productbuild

echo "Setting partition list..."
# Allow standard tools to access the key without prompt
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASS" "$KEYCHAIN_NAME"

echo "Listing identities in temporary keychain..."
security find-identity -p codesigning "$KEYCHAIN_NAME"

echo "Signing App Bundle..."
codesign --force --options runtime --deep --sign "$APP_IDENTITY" --entitlements "$ENTITLEMENTS_PATH" "$APP_PATH" --keychain "$KEYCHAIN_NAME"

echo "Verifying App Signature..."
codesign -dv --verbose=4 "$APP_PATH"

echo "Building Installer Package..."
# Note: Removed --product flag as we want default requirements for now, or we can add it back if needed.
# Using --component to package the app.
productbuild --component "$APP_PATH" /Applications --sign "$INSTALLER_IDENTITY" "$PKG_OUTPUT" --keychain "$KEYCHAIN_NAME"

echo "Done! Package created at $PKG_OUTPUT"
