# Sentinel Discord Bot

A full-spectrum Discord moderation and logging bot designed for transparency and auditability.

## Features

- **Complete Message Logging**: All messages, edits, and deletions are tracked
- **Media Attachment Caching**: Downloads and stores all media attachments locally (toggleable)
- **Voice Activity Tracking**: Logs joins, leaves, and channel switches
- **Forum/Thread Monitoring**: Tracks thread creation and content
- **User Database**: Maintains records of all server users with metadata
- **DM-Based Moderation**: Anonymous moderation commands via DMs
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

## DM Commands

Send these commands via Direct Message to the bot:

- `/help` - Show command list or send mod alert
- `/kick <user_id> <guild_id> [reason]` - Kick a user
- `/ban <user_id> <guild_id> [reason]` - Ban a user  
- `/timeout <user_id> <guild_id> <minutes> [reason]` - Timeout a user (max 28 days)
- `/cache [on|off]` - Toggle media caching on or off (whitelisted only)

## Whitelist Management

To add users to the moderation whitelist, insert their Discord user ID into the `command_whitelist` table:

```sql
INSERT INTO command_whitelist (discord_user_id) VALUES (123456789012345678);
```

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

## Development

```bash
cargo fmt       # Format code
cargo clippy    # Run linter
cargo test      # Run tests
```