# Sentinel Discord Bot

A comprehensive Discord moderation and logging bot built with Rust and Serenity framework (v0.12), designed for transparency, auditability, and data retention in environments like AI model training platforms.

## Quick Start

```bash
# Clone the repository
git clone https://github.com/yourusername/sentinel.git
cd sentinel

# Set up environment
echo "DISCORD_TOKEN=your_bot_token_here" > .env
echo "DATABASE_URL=mysql://root:password@localhost/sentinel" >> .env

# Run the bot
cargo run
```

## Features

### Core Logging & Tracking
- **Complete Message Logging**: All messages, edits, and deletions tracked with timestamps
- **Historical Message Scanning**: Background job retrieves messages sent before bot joined (up to 10,000 per channel)
- **Media Attachment Caching**: Downloads and stores all media locally with automatic 31-day cleanup (toggleable)
- **Voice Activity Tracking**: Logs joins, leaves, and channel switches
- **Forum/Thread Monitoring**: Tracks thread creation and content
- **Poll Tracking**: Logs Discord polls creation, votes, and expiry with automatic closure
- **Event Tracking**: Monitors Discord scheduled events, user RSVPs, and all event changes

### Member & Presence Tracking
- **User Database**: Maintains records of all server users with metadata
- **Member Presence Tracking**: Logs status changes (online/idle/dnd/offline) and activities
- **Member Join/Leave Tracking**: Records when users join or leave servers
- **Nickname Change Detection**: Tracks all nickname modifications with timestamps
- **Channel Audit Logging**: Monitors channel creation, deletion, and modifications

### Moderation System
- **Slash Command Based**: Modern Discord integration with autocomplete
- **Cross-Guild Moderation**: Commands work across ALL guilds where bot is present
- **Smart User Search**: Find users by username, @handle, or server nickname
- **Whitelist System**: Hierarchical permissions (super users, whitelisted users, regular users)
- **Detailed Logging**: All bot actions and responses tracked in database

### Data Management
- **Automatic Data Cleanup**: Daily job removes logs older than 31 days to manage database size
- **Background Jobs**: User sync, media cleanup, poll expiry, and historical scanning
- **Structured Logging**: JSON file logs with daily rotation plus database storage

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

## Environment Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `DISCORD_TOKEN` | Your Discord bot token | `MTIz...abc` |
| `DATABASE_URL` | MariaDB/MySQL connection string | `mysql://user:pass@localhost/sentinel` |
| `RUST_LOG` | Logging level (optional) | `info`, `debug`, `trace` |

## Commands

All commands are implemented as Discord slash commands with autocomplete support:

### Slash Commands

| Command | Description | Access Level |
|---------|-------------|--------------|
| `/help` | Show available commands | Everyone |
| `/kick <user> [reason]` | Kick user from all guilds | Whitelisted only |
| `/ban <user> [reason]` | Ban user from all guilds | Whitelisted only |
| `/timeout <user> <duration> [reason]` | Timeout user (1-40320 minutes) | Whitelisted only |
| `/cache [on\|off\|status]` | Toggle/check media caching | Whitelisted only |
| `/whitelist <add\|remove> <user>` | Manage command whitelist | Super users only |
| `/snort` | Snort brightdust! (with memes) | Everyone |

**Features**: 
- **User Autocomplete**: Start typing a username, handle, or nickname to see suggestions
- **Cross-Guild Execution**: Moderation commands work across ALL guilds where bot is present
- **Per-User Cooldowns**: Each user has independent cooldown for fun commands
- **Smart Search**: Finds users by username, @handle, or server nickname
- **Detailed Results**: Shows success/failure for each guild with specific error messages
- **Meme Integration**: `/snort` command includes random meme attachments

### Legacy DM Support

The bot still accepts DM commands for backward compatibility, but slash commands are preferred.

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
- Manage Events (for event tracking)

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

## Configuration

### Snort Command

The `/snort` command has per-user cooldowns and meme support:

```sql
-- View current cooldown
SELECT setting_value FROM system_settings WHERE setting_key = 'snort_cooldown_seconds';

-- Change cooldown (in seconds)
UPDATE system_settings SET setting_value = '60' WHERE setting_key = 'snort_cooldown_seconds';
```

**Meme Setup**: Place image files in `memes/snort/` directory (jpg, png, gif, webp supported)

### Data Retention

The bot automatically cleans up old data after 31 days. This includes:
- Member status logs
- Nickname change logs
- Voice activity logs
- Closed poll votes
- Past event data

## Background Jobs

The bot runs several automated tasks:

1. **User Sync** (every 12 hours) - Updates user metadata
2. **Media Cleanup** (daily at 3 AM) - Removes cached files older than 31 days
3. **Log Cleanup** (daily at 4 AM) - Removes database logs older than 31 days
4. **Channel History Scan** (hourly) - Retrieves historical messages
5. **Poll Expiry Check** (hourly) - Closes expired polls

## Database Schema

The bot uses MariaDB/MySQL with the following main tables:

### Core Tables
- `users` - Discord user profiles with usernames, handles, and nicknames
- `message_logs` - All message content with edit tracking
- `message_attachments` - Media attachment metadata and local paths
- `voice_logs` - Voice channel activity (join/leave/switch)
- `forum_logs` - Thread and forum post creation

### Tracking Tables
- `member_status_logs` - User presence and activity tracking
- `nickname_logs` - Nickname change history
- `channel_logs` - Channel modifications and audit trail
- `dm_logs` - Direct messages to the bot
- `bot_response_logs` - Bot command responses

### Poll & Event Tables
- `poll_logs` - Discord poll metadata
- `poll_answers` - Poll answer options
- `poll_votes` - User votes on polls
- `event_logs` - Discord scheduled events
- `event_interests` - User RSVPs for events
- `event_update_logs` - Event modification history

### System Tables
- `command_whitelist` - Users authorized for moderation
- `super_user_whitelist` - Users with admin privileges
- `system_settings` - Configurable bot settings
- `snort_counter` - Global snort statistics
- `user_snort_cooldowns` - Per-user cooldown tracking
- `channel_scan_history` - Historical message scan progress

## Development

```bash
cargo fmt       # Format code
cargo clippy    # Run linter
cargo test      # Run tests
cargo doc --open # Generate documentation
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

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Serenity](https://github.com/serenity-rs/serenity) Discord library
- Uses [SQLx](https://github.com/launchbadge/sqlx) for database operations
- Scheduling powered by [tokio-cron-scheduler](https://github.com/mvniekerk/tokio-cron-scheduler)