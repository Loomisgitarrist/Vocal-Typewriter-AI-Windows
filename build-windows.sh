#!/bin/bash
# Windows Build Script for Vocal Typewriter AI
# Builds both x64 and ARM64 Windows versions

set -e

echo "================================================"
echo "Vocal Typewriter AI - Windows Build Script"
echo "================================================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print status
print_status() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Check if running on Windows (via WSL or native)
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" || -n "$WSL_DISTRO_NAME" ]]; then
    print_status "Windows environment detected"
else
    print_warning "Not running in Windows environment. This script is meant for Windows."
    print_warning "For macOS, use the build-windows-cross.sh script instead."
    exit 1
fi

# Check prerequisites
print_status "Checking prerequisites..."

if ! command -v cargo &> /dev/null; then
    print_error "Rust/Cargo not found. Install from https://rustup.rs"
    exit 1
fi

if ! command -v bun &> /dev/null; then
    print_status "Installing bun..."
    if command -v powershell &> /dev/null; then
        powershell -c "irm bun.sh/install.ps1 | iex"
    else
        print_error "PowerShell not found. Please install bun manually."
        exit 1
    fi
fi

# Add Windows targets
print_status "Adding Windows build targets..."
rustup target add x86_64-pc-windows-msvc || true
rustup target add aarch64-pc-windows-msvc || true

# Install dependencies
print_status "Installing Node.js dependencies..."
bun install

# Create output directory
OUTPUT_DIR="release-windows"
mkdir -p "$OUTPUT_DIR"

# Build x64 version
print_status "Building Windows x64 version..."
echo "================================================"
set CARGO_BUILD_JOBS=2
bun tauri build --target x86_64-pc-windows-msvc

# Copy x64 artifacts
print_status "Copying x64 build artifacts..."
cp "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/"*.msi "$OUTPUT_DIR/" 2>/dev/null || true
cp "src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/"*.exe "$OUTPUT_DIR/" 2>/dev/null || true

# Build ARM64 version
print_status "Building Windows ARM64 version..."
echo "================================================"
bun tauri build --target aarch64-pc-windows-msvc

# Copy ARM64 artifacts
print_status "Copying ARM64 build artifacts..."
cp "src-tauri/target/aarch64-pc-windows-msvc/release/bundle/msi/"*.msi "$OUTPUT_DIR/" 2>/dev/null || true
cp "src-tauri/target/aarch64-pc-windows-msvc/release/bundle/nsis/"*.exe "$OUTPUT_DIR/" 2>/dev/null || true

# Summary
echo ""
echo "================================================"
print_status "Build Complete!"
echo "================================================"
echo ""
echo "Output files in: $OUTPUT_DIR/"
echo ""
ls -lh "$OUTPUT_DIR/" 2>/dev/null || echo "No output files found"
echo ""
echo "Build artifacts:"
echo "  - MSI installers (Windows Installer)"
echo "  - EXE installers (NSIS)"
echo "  - Both x64 and ARM64 architectures"
echo ""
print_status "Done!"
