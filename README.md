# Sentinel Discord Bot

A full-spectrum Discord moderation and logging bot designed for transparency and auditability.

## Features

- **Complete Message Logging**: All messages, edits, and deletions are tracked
- **DM Logging**: All direct messages to the bot are logged to database
- **Bot Response Logging**: All bot responses to commands are logged to database
- **File Logging**: All logs written to daily rotating files in JSON format
- **Media Attachment Caching**: Downloads and stores all media attachments locally (toggleable)
- **Voice Activity Tracking**: Logs joins, leaves, and channel switches
- **Forum/Thread Monitoring**: Tracks thread creation and content
- **User Database**: Maintains records of all server users with metadata
- **Member Presence Tracking**: Logs status changes (online/idle/dnd/offline) and activities
- **Member Join/Leave Tracking**: Logs when users join or leave servers
- **Nickname Change Detection**: Tracks all nickname modifications with timestamps
- **Channel Audit Logging**: Monitors channel creation, deletion, and modifications (name, topic, permissions)
- **DM-Based Moderation**: Anonymous moderation commands via DMs with smart command suggestions
- **Whitelist System**: Only authorized users can use moderation commands
- **Background Sync**: Automatic user data synchronization every 12 hours
- **Media Cleanup**: Automatic deletion of cached media older than 31 days

## Setup

1. **Database Setup**
   ```bash
   # Create a MariaDB/MySQL database
   mysql -u root -p
   CREATE DATABASE sentinel;
   ```

2. **Environment Configuration**
   ```bash
   cp .env.example .env
   # Edit .env with your values:
   # DISCORD_TOKEN=your_bot_token_here
   # DATABASE_URL=mysql://user:password@localhost/sentinel
   ```

3. **Build and Run**
   ```bash
   cargo build --release
   cargo run
   ```

## Commands

### DM Commands

Send these commands via Direct Message to the bot:

- `/help` - Show command list
- `/kick <@user> [reason]` - Kick a user from all guilds (whitelisted only)
- `/ban <@user> [reason]` - Ban a user from all guilds (whitelisted only)
- `/timeout <@user> <minutes> [reason]` - Timeout a user in all guilds (max 28 days, whitelisted only)
- `/cache [on|off]` - Toggle media caching on or off (whitelisted only)
- `/whitelist <add|remove> <@user>` - Manage command whitelist (super users only)

### Slash Commands

Available in server channels:

- `/snort` - Snort some brightdust! (tracks global count with configurable cooldown)

**Note**: 
- Moderation commands now work across ALL guilds where the bot is present
- Use Discord handles instead of user IDs (e.g., `@username`, `username#1234`, or server nicknames)
- The bot will search for users by their username, global handle, or server nickname
- Results will show success/failure for each guild
- Invalid commands will receive suggestions for the most likely intended command
- Common misspellings and aliases are recognized (e.g., "mute" suggests "/timeout")

## Whitelist Management

### Super Users
Super users have full access to all moderation commands and can manage the regular command whitelist. They cannot be removed from the whitelist by other super users.

To add a super user:
```sql
INSERT INTO super_user_whitelist (discord_user_id) VALUES (123456789012345678);
```

### Regular Whitelist
Regular whitelisted users can use moderation commands but cannot manage the whitelist.

To add users to the moderation whitelist:
- **Via SQL**: 
  ```sql
  INSERT INTO command_whitelist (discord_user_id) VALUES (123456789012345678);
  ```
- **Via Bot Command** (super users only): `/whitelist add @username`

### Permission Hierarchy
1. **Super Users**: All moderation commands + whitelist management
2. **Whitelisted Users**: All moderation commands
3. **Regular Users**: `/help` command only

## Required Bot Permissions

- Read Messages
- Send Messages
- Manage Messages
- Read Message History
- View Channels
- Connect (for voice tracking)
- Moderate Members (for timeouts)
- Kick Members
- Ban Members
- View Server Insights (for presence tracking)

## Media Caching

The bot can download and store media attachments locally. This feature:

- Is toggleable via the `/cache` command or database setting
- Organizes files by type (images, videos, audio, documents, other)
- Automatically cleans up files older than 31 days
- Stores files in `./media_cache/` (excluded from git)

To enable/disable programmatically:
```sql
UPDATE system_settings SET setting_value = 'true' WHERE setting_key = 'cache_media';
```

## Slash Command Configuration

### Snort Cooldown

The `/snort` command has a configurable global cooldown (default: 30 seconds):

```sql
-- View current cooldown
SELECT setting_value FROM system_settings WHERE setting_key = 'snort_cooldown_seconds';

-- Change cooldown (in seconds)
UPDATE system_settings SET setting_value = '60' WHERE setting_key = 'snort_cooldown_seconds';
```

## Development

```bash
cargo fmt       # Format code
cargo clippy    # Run linter
cargo test      # Run tests
```

## Cross-Compilation for Raspberry Pi 5

The Raspberry Pi 5 uses ARM64 (aarch64) architecture. We provide two methods for cross-compilation:

### Method 1: Using Native Cross-Compilation Tools

1. **Install cross-compilation tools** (Ubuntu/Debian):
   ```bash
   sudo apt-get update
   sudo apt-get install gcc-aarch64-linux-gnu g++-aarch64-linux-gnu
   ```

2. **Add the Rust target**:
   ```bash
   rustup target add aarch64-unknown-linux-gnu
   ```

3. **Build using the provided script**:
   ```bash
   ./build-pi5.sh
   ```

### Method 2: Using Docker (No tools installation required)

If you have Docker installed, you can build without installing cross-compilation tools:

```bash
./build-pi5-docker.sh
```

### Deploying to Raspberry Pi 5

After building, deploy the binary:

```bash
# Copy the binary and config
scp target/aarch64-unknown-linux-gnu/release/sentinel pi@<your-pi-ip>:~/
# Or if using Docker build:
scp sentinel-pi5 pi@<your-pi-ip>:~/sentinel

# Copy the environment file
scp .env pi@<your-pi-ip>:~/

# SSH to your Pi
ssh pi@<your-pi-ip>

# Make executable and run
chmod +x sentinel
./sentinel
```

### Running as a Service on Raspberry Pi

Create a systemd service on your Pi:

```bash
sudo nano /etc/systemd/system/sentinel.service
```

Add the following content:
```ini
[Unit]
Description=Sentinel Discord Bot
After=network.target

[Service]
Type=simple
User=pi
WorkingDirectory=/home/pi
ExecStart=/home/pi/sentinel
Restart=on-failure
RestartSec=10
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
```

Enable and start the service:
```bash
sudo systemctl enable sentinel
sudo systemctl start sentinel
sudo systemctl status sentinel
```