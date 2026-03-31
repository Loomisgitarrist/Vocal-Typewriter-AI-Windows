#!/bin/bash
set -e

SOURCE_IMG="app_icon_source.png"
ICONSET_DIR="icon.iconset"

rm -rf "$ICONSET_DIR"
mkdir -p "$ICONSET_DIR"

# Crop to square (center) 768x768 first
sips --cropToHeightWidth 768 768 "$SOURCE_IMG" --out temp_square.png

# Function to resize and ensure PNG format
resize_icon() {
    local size=$1
    local name=$2
    sips -z $size $size -s format png temp_square.png --out "${ICONSET_DIR}/${name}"
}

resize_icon 16   "icon_16x16.png"
resize_icon 32   "icon_16x16@2x.png"
resize_icon 32   "icon_32x32.png"
resize_icon 64   "icon_32x32@2x.png"
resize_icon 128  "icon_128x128.png"
resize_icon 256  "icon_128x128@2x.png"
resize_icon 256  "icon_256x256.png"
resize_icon 512  "icon_256x256@2x.png"
resize_icon 512  "icon_512x512.png"
resize_icon 1024 "icon_512x512@2x.png"

# Convert to icns
echo "Converting to icns..."
iconutil -c icns "$ICONSET_DIR" -o src-tauri/icons/icon.icns

# Cleanup
rm -rf "$ICONSET_DIR" temp_square.png

echo "Icon generated successfully!"
