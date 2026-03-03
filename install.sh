#!/bin/bash
# install.sh - Install dark-pattern-blocker daemon and native messaging host

set -e

echo "=== Dark Pattern Blocker Installer ==="

# Build release binaries
echo "Building release binaries..."
cargo build --release

# Install daemon
echo "Installing daemon to /usr/local/bin/dark-pattern..."
sudo cp target/release/dark-pattern /usr/local/bin/dark-pattern
sudo chmod +x /usr/local/bin/dark-pattern

# Install native messaging host
echo "Installing native host to /usr/local/bin/dark-pattern-host..."
sudo cp target/release/dark-pattern-host /usr/local/bin/dark-pattern-host
sudo chmod +x /usr/local/bin/dark-pattern-host

# Install GUI
echo "Installing GUI to /usr/local/bin/dark-pattern-gui..."
sudo cp target/release/dark-pattern-gui /usr/local/bin/dark-pattern-gui
sudo chmod +x /usr/local/bin/dark-pattern-gui

# Create config directory
echo "Creating config directory..."
sudo mkdir -p /usr/local/etc/dark-pattern
sudo mkdir -p /var/root/.local/share/dark-pattern

# Create default config if it doesn't exist
if [ ! -f /usr/local/etc/dark-pattern/config.toml ]; then
    echo "Creating default config..."
    sudo tee /usr/local/etc/dark-pattern/config.toml > /dev/null << 'EOF'
hosts_file = "/etc/hosts"
socket_path = "/tmp/dark-pattern.sock"
pid_path = "/tmp/dark-pattern.pid"
db_path = "/var/root/.local/share/dark-pattern/blocker.db"
EOF
fi

# Install launchd plist
echo "Installing launchd daemon..."
sudo cp com.aaron.dark-pattern.plist /Library/LaunchDaemons/
sudo chown root:wheel /Library/LaunchDaemons/com.aaron.dark-pattern.plist
sudo chmod 644 /Library/LaunchDaemons/com.aaron.dark-pattern.plist

# Create log directory
sudo mkdir -p /usr/local/var/log

# Stop existing daemon if running
echo "Stopping existing daemon (if running)..."
sudo launchctl bootout system/com.aaron.dark-pattern 2>/dev/null || true

# Start daemon
echo "Starting daemon..."
sudo launchctl bootstrap system /Library/LaunchDaemons/com.aaron.dark-pattern.plist

# Verify daemon is running
sleep 2
if dark-pattern status 2>/dev/null; then
    echo "✅ Daemon is running!"
else
    echo "⚠️  Daemon may not be running. Check: tail /usr/local/var/log/dark-pattern.log"
fi

# Install native messaging host manifest for Chrome
EXTENSION_ID="${1:-YOUR_EXTENSION_ID_HERE}"
CHROME_NM_DIR="$HOME/Library/Application Support/Google/Chrome/NativeMessagingHosts"

echo ""
echo "Installing native messaging host manifest..."
mkdir -p "$CHROME_NM_DIR"

cat > "$CHROME_NM_DIR/com.aaron.dark_pattern.json" << EOF
{
  "name": "com.aaron.dark_pattern",
  "description": "Dark Pattern Blocker Native Messaging Host",
  "path": "/usr/local/bin/dark-pattern-host",
  "type": "stdio",
  "allowed_origins": [
    "chrome-extension://${EXTENSION_ID}/"
  ]
}
EOF

echo "✅ Native messaging host installed for Chrome"

# Firefox (optional)
FIREFOX_NM_DIR="$HOME/Library/Application Support/Mozilla/NativeMessagingHosts"
if [ -d "$HOME/Library/Application Support/Mozilla" ]; then
    mkdir -p "$FIREFOX_NM_DIR"
    cat > "$FIREFOX_NM_DIR/com.aaron.dark_pattern.json" << EOF
{
  "name": "com.aaron.dark_pattern",
  "description": "Dark Pattern Blocker Native Messaging Host",
  "path": "/usr/local/bin/dark-pattern-host",
  "type": "stdio",
  "allowed_extensions": [
    "dark-pattern-blocker@example.com"
  ]
}
EOF
    echo "✅ Native messaging host installed for Firefox"
fi

echo ""
echo "=== Installation Complete ==="
echo ""
echo "Next steps:"
echo "1. Load the extension in Chrome (chrome://extensions → Load unpacked)"
echo "2. Copy the extension ID from Chrome"
echo "3. Re-run: ./install.sh YOUR_EXTENSION_ID"
echo ""
echo "Useful commands:"
echo "  dark-pattern status          # Check session status"
echo "  dark-pattern start 25m       # Start 25 minute session"
echo "  dark-pattern list            # List blocked websites"
echo "  tail -f /usr/local/var/log/dark-pattern.log  # Watch logs"
echo "  tail -f /tmp/native-host.log                 # Watch native host logs"
