#!/bin/bash

# Build script for cross-compiling Sentinel for Raspberry Pi 5 (ARM64/aarch64)

set -e

echo "🔧 Building Sentinel for Raspberry Pi 5 (aarch64)..."

# Check if cross-compilation tools are installed
if ! command -v aarch64-linux-gnu-gcc &> /dev/null; then
    echo "❌ Error: aarch64-linux-gnu-gcc not found!"
    echo "Please install cross-compilation tools:"
    echo "  sudo apt-get update"
    echo "  sudo apt-get install gcc-aarch64-linux-gnu"
    echo "  sudo apt-get install g++-aarch64-linux-gnu"
    exit 1
fi

# Add the target if not already added
if ! rustup target list | grep -q "aarch64-unknown-linux-gnu (installed)"; then
    echo "📦 Adding aarch64-unknown-linux-gnu target..."
    rustup target add aarch64-unknown-linux-gnu
fi

# Set environment variables for cross-compilation
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
export CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++

# Build the project
echo "🚀 Building release binary..."
cargo build --release --target=aarch64-unknown-linux-gnu

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "📄 Binary location: target/aarch64-unknown-linux-gnu/release/sentinel"
    
    # Show binary info
    echo ""
    echo "📊 Binary information:"
    file target/aarch64-unknown-linux-gnu/release/sentinel
    ls -lh target/aarch64-unknown-linux-gnu/release/sentinel
else
    echo "❌ Build failed!"
    exit 1
fi

echo ""
echo "📋 To deploy to your Raspberry Pi 5:"
echo "  scp target/aarch64-unknown-linux-gnu/release/sentinel pi@<your-pi-ip>:~/"
echo "  scp .env pi@<your-pi-ip>:~/"
echo ""
echo "Then on your Pi:"
echo "  chmod +x sentinel"
echo "  ./sentinel"