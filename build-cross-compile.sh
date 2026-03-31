#!/bin/bash
# Cross-compile Windows builds from macOS
# Requires: cargo-xwin, llvm, and Windows SDK

set -e

echo "================================================"
echo "Cross-Compile Windows Builds from macOS"
echo "================================================"
echo ""

# Check if cargo-xwin is installed
if ! command -v cargo-xwin &> /dev/null; then
    echo "Installing cargo-xwin..."
    cargo install cargo-xwin
fi

# Add Windows targets
rustup target add x86_64-pc-windows-msvc
rustup target add aarch64-pc-windows-msvc

cd "$(dirname "$0")"

echo "Installing frontend dependencies..."
bun install

echo "Building frontend..."
bun run build

echo ""
echo "================================================"
echo "Building Windows x64..."
echo "================================================"
cd src-tauri
cargo xwin build --target x86_64-pc-windows-msvc --release
cd ..

echo ""
echo "================================================"
echo "Building Windows ARM64..."
echo "================================================"
cd src-tauri
cargo xwin build --target aarch64-pc-windows-msvc --release
cd ..

echo ""
echo "================================================"
echo "Creating installers..."
echo "================================================"

mkdir -p release-windows

# Tauri bundle
cd src-tauri
cargo xwin tauri build --target x86_64-pc-windows-msvc || true
cargo xwin tauri build --target aarch64-pc-windows-msvc || true

echo "Build complete!"
echo "Note: Full Tauri bundling (MSI/NSIS) requires Windows."
echo "The .exe binaries are in src-tauri/target/*/release/"
