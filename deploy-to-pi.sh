#!/bin/bash

# Deployment script for Sentinel to Raspberry Pi 5

set -e

# Check if PI_HOST is set
if [ -z "$1" ]; then
    echo "Usage: ./deploy-to-pi.sh <pi-username>@<pi-ip-address>"
    echo "Example: ./deploy-to-pi.sh pi@192.168.1.100"
    exit 1
fi

PI_HOST=$1

echo "🚀 Deploying Sentinel to $PI_HOST..."

# Check if binary exists
if [ -f "target/aarch64-unknown-linux-gnu/release/sentinel" ]; then
    BINARY="target/aarch64-unknown-linux-gnu/release/sentinel"
elif [ -f "sentinel-pi5" ]; then
    BINARY="sentinel-pi5"
else
    echo "❌ No compiled binary found!"
    echo "Please run ./build-pi5.sh or ./build-pi5-docker.sh first"
    exit 1
fi

# Check if .env exists
if [ ! -f ".env" ]; then
    echo "⚠️  Warning: .env file not found"
    echo "Please create a .env file with your DISCORD_TOKEN and DATABASE_URL"
fi

# Create remote directory
echo "📁 Creating directory on Pi..."
ssh $PI_HOST "mkdir -p ~/sentinel-bot"

# Copy files
echo "📤 Copying files..."
scp $BINARY $PI_HOST:~/sentinel-bot/sentinel
if [ -f ".env" ]; then
    scp .env $PI_HOST:~/sentinel-bot/
fi

# Set permissions
echo "🔧 Setting permissions..."
ssh $PI_HOST "chmod +x ~/sentinel-bot/sentinel"

# Create systemd service
echo "🔧 Creating systemd service..."
ssh $PI_HOST "sudo tee /etc/systemd/system/sentinel.service > /dev/null" << 'EOF'
[Unit]
Description=Sentinel Discord Bot
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi/sentinel-bot
ExecStart=/home/pi/sentinel-bot/sentinel
Restart=on-failure
RestartSec=10
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
EOF

# Reload systemd and enable service
echo "🔄 Reloading systemd..."
ssh $PI_HOST "sudo systemctl daemon-reload"
ssh $PI_HOST "sudo systemctl enable sentinel"

echo "✅ Deployment complete!"
echo ""
echo "📋 Next steps on your Pi:"
echo "  Start the bot:    sudo systemctl start sentinel"
echo "  Check status:     sudo systemctl status sentinel"
echo "  View logs:        sudo journalctl -u sentinel -f"
echo "  Stop the bot:     sudo systemctl stop sentinel"
echo "  Restart the bot:  sudo systemctl restart sentinel"