#!/bin/bash

# Docker-based build script for cross-compiling Sentinel for Raspberry Pi 5
# This doesn't require installing cross-compilation tools on your system

set -e

echo "🐳 Building Sentinel for Raspberry Pi 5 using Docker..."

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    echo "❌ Error: Docker not found!"
    echo "Please install Docker: https://docs.docker.com/get-docker/"
    exit 1
fi

# Create a temporary Dockerfile for cross-compilation
cat > Dockerfile.pi5 << 'EOF'
FROM rust:latest

# Install cross-compilation tools
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    g++-aarch64-linux-gnu \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Add the ARM64 target
RUN rustup target add aarch64-unknown-linux-gnu

# Set up the working directory
WORKDIR /app

# Copy the source code
COPY . .

# Set environment variables for cross-compilation
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
ENV CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc
ENV CXX_aarch64_unknown_linux_gnu=aarch64-linux-gnu-g++
ENV PKG_CONFIG_SYSROOT_DIR=/usr/aarch64-linux-gnu

# Build the project
RUN cargo build --release --target=aarch64-unknown-linux-gnu

# The binary will be in /app/target/aarch64-unknown-linux-gnu/release/sentinel
EOF

# Build the Docker image
echo "📦 Building Docker image..."
docker build -f Dockerfile.pi5 -t sentinel-pi5-builder .

# Extract the binary from the container
echo "📤 Extracting binary from container..."
docker create --name sentinel-extract sentinel-pi5-builder
docker cp sentinel-extract:/app/target/aarch64-unknown-linux-gnu/release/sentinel ./sentinel-pi5
docker rm sentinel-extract

# Clean up
rm Dockerfile.pi5

echo "✅ Build successful!"
echo "📄 Binary location: ./sentinel-pi5"

# Show binary info
echo ""
echo "📊 Binary information:"
file sentinel-pi5
ls -lh sentinel-pi5

echo ""
echo "📋 To deploy to your Raspberry Pi 5:"
echo "  scp sentinel-pi5 pi@<your-pi-ip>:~/sentinel"
echo "  scp .env pi@<your-pi-ip>:~/"
echo "  scp -r migrations pi@<your-pi-ip>:~/"
echo ""
echo "Then on your Pi:"
echo "  # Install sqlx-cli if not already installed"
echo "  cargo install sqlx-cli --no-default-features --features mysql"
echo "  # Run migrations"
echo "  sqlx migrate run"
echo "  # Make binary executable and run"
echo "  chmod +x sentinel"
echo "  ./sentinel"