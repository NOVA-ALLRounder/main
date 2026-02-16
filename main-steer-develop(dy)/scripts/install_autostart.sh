#!/bin/bash

# Configuration
AGENT_NAME="com.antigravity.agent"
PLIST_PATH="$HOME/Library/LaunchAgents/$AGENT_NAME.plist"
CORE_DIR="$(cd "$(dirname "$0")/../core" && pwd)"
BINARY_PATH="$CORE_DIR/target/release/core"
LOG_DIR="$HOME/.antigravity/logs"

# Colors
GREEN='\033[0;32m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 Installing Local OS Agent as a Background Service...${NC}"

# 1. Build Release Binary
echo "📦 Building core binary..."
cd "$CORE_DIR" || exit
cargo build --release

if [ ! -f "$BINARY_PATH" ]; then
    echo "❌ Build failed. Binary not found."
    exit 1
fi

# 2. Create Log Directory
mkdir -p "$LOG_DIR"

# 3. Create LaunchAgent plist
echo "📝 Creating LaunchAgent plist..."
cat <<EOF > "$PLIST_PATH"
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>$AGENT_NAME</string>
    <key>ProgramArguments</key>
    <array>
        <string>$BINARY_PATH</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>$LOG_DIR/stdout.log</string>
    <key>StandardErrorPath</key>
    <string>$LOG_DIR/stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
        <key>PATH</key>
        <string>$PATH:/usr/local/bin:/opt/homebrew/bin</string>
    </dict>
</dict>
</plist>
EOF

# 4. Load Service
echo "🔄 Loading service..."
launchctl unload "$PLIST_PATH" 2>/dev/null
launchctl load "$PLIST_PATH"

echo -e "${GREEN}✅ Success! The Agent will now start automatically on login.${NC}"
echo "   - View Logs: tail -f $LOG_DIR/stdout.log"
echo "   - Web Dashboard: http://localhost:5680"
