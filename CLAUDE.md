
# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

---

## Project Overview

Sentinel is a Discord bot written in Rust using the Serenity framework (v0.12). It serves as a full-spectrum moderation and logging tool designed for use in environments where transparency, privacy, and auditability are essential—such as AI model training platforms like Claude.

Key features include:

- Logging all Discord activity (messages, voice, threads, forums)
- **Media attachment caching**: Downloads and stores all media locally (toggleable)
- **Member presence tracking**: Status changes (online/idle/dnd/offline) and activities
- **Member join/leave tracking**: Logs when users join or leave servers
- **Nickname monitoring**: Tracks all nickname changes with timestamps
- **Channel audit logs**: Creation, deletion, and modifications (name, topic, permissions)
- **Smart command handling**: Suggests correct commands for misspellings
- Tracking all server users with metadata syncing
- Background job to keep usernames, nicknames, and handles current
- Whitelist-restricted moderation commands sent via Direct Message
- Automatic cleanup of cached media older than 31 days
- Data stored in a MariaDB database

---

## Quick Start

```bash
# Set up environment
echo "DISCORD_TOKEN=your_bot_token_here" > .env

# Run the bot
cargo run

# Before committing any changes
cargo fmt && cargo clippy
```

---

## Development Commands

```bash
cargo build              # Build the project
cargo run                # Run the Discord bot
cargo check              # Quick compilation check
cargo fmt                # Format code
cargo clippy             # Run linter
cargo test               # Run tests
cargo doc --open         # Generate and view documentation
```

---

## Architecture & Key Patterns

### Current Structure

- **Event-driven**: `Handler` struct implements `EventHandler` trait
- **Async runtime**: Uses Tokio for concurrent operations
- **Gateway intents**: Uses `GUILD_MESSAGES`, `GUILD_VOICE_STATES`, `GUILD_MEMBERS`, `MESSAGE_CONTENT`, `GUILD_MESSAGE_REACTIONS`, `DIRECT_MESSAGES`, and `GUILD_MESSAGE_TYPING` as needed

### Core Functional Areas

- **Message Logging**: All messages are logged to `message_logs` in MariaDB
- **Voice Events**: Joins, leaves, and switches are tracked in `voice_logs`
- **Forum and Thread Monitoring**: Captured in `forum_logs`
- **User Tracking**: All server users stored in `users`, updated daily
- **DM Commands**: `/kick`, `/ban`, `/timeout`, `/help` parsed from private messages
- **Whitelist Enforcement**: Moderation commands allowed only for `command_whitelist` users

---

## Database Schema (MariaDB)

```sql
-- Track all Discord users
CREATE TABLE users (
  id INT PRIMARY KEY AUTO_INCREMENT,
  discord_user_id BIGINT UNIQUE NOT NULL,
  username VARCHAR(255),
  discriminator VARCHAR(10),
  global_handle VARCHAR(255),
  nickname VARCHAR(255),
  last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- Moderator whitelist
CREATE TABLE command_whitelist (
  id INT PRIMARY KEY AUTO_INCREMENT,
  discord_user_id BIGINT UNIQUE NOT NULL,
  added_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Super user whitelist
CREATE TABLE super_user_whitelist (
  id INT PRIMARY KEY AUTO_INCREMENT,
  discord_user_id BIGINT UNIQUE NOT NULL,
  added_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Message logs
CREATE TABLE message_logs (
  id INT PRIMARY KEY AUTO_INCREMENT,
  message_id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  channel_id BIGINT NOT NULL,
  content TEXT,
  timestamp DATETIME,
  edited BOOLEAN DEFAULT FALSE
);

-- Voice channel activity
CREATE TABLE voice_logs (
  id INT PRIMARY KEY AUTO_INCREMENT,
  user_id BIGINT NOT NULL,
  channel_id BIGINT NOT NULL,
  action ENUM('join', 'leave', 'switch'),
  timestamp DATETIME
);

-- Forum and thread events
CREATE TABLE forum_logs (
  id INT PRIMARY KEY AUTO_INCREMENT,
  thread_id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  title TEXT,
  content TEXT,
  created_at DATETIME
);

-- Direct message logs
CREATE TABLE dm_logs (
  id INT PRIMARY KEY AUTO_INCREMENT,
  message_id BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  content TEXT,
  command VARCHAR(50),
  timestamp DATETIME
);

-- Bot response logs
CREATE TABLE bot_response_logs (
  id INT PRIMARY KEY AUTO_INCREMENT,
  user_id BIGINT NOT NULL,
  command VARCHAR(50),
  response_type VARCHAR(50),
  response_content TEXT,
  success BOOLEAN DEFAULT TRUE,
  timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
);
```

---

## Background Jobs

Sentinel runs a scheduled job (using `tokio_cron_scheduler` or a custom Tokio loop) to update the `users` table:

- Maps `discord_user_id` to:
  - Username
  - Discriminator
  - Global handle (`@handle`)
  - Server-specific nickname
- Runs at regular intervals (e.g., every 12 or 24 hours)

This keeps logs cross-referenced with accurate identity metadata for auditing or AI training.

---

## Moderation Commands

Only accepted via **Direct Messages** to preserve anonymity:

| Command                          | Description                             | Access           |
|----------------------------------|-----------------------------------------|------------------|
| `/kick <@user> [reason]`         | Removes user from all connected servers | Whitelisted only |
| `/ban <@user> [reason]`          | Bans user from all connected servers    | Whitelisted only |
| `/timeout <@user> <mins> [reason]` | Temporarily mutes user in all servers   | Whitelisted only |
| `/cache [on|off]`                | Toggle media caching                    | Whitelisted only |
| `/whitelist <add|remove> <@user>`| Manage command whitelist                | Super users only |
| `/help [message]`                | Sends mod alert (sender is attached)    | Anyone           |

User permissions are validated against the `command_whitelist` and `super_user_whitelist` tables.

**Permission Hierarchy**:
1. **Super Users**: All commands + whitelist management
2. **Whitelisted Users**: All moderation commands
3. **Regular Users**: `/help` command only

**User Identification**: Commands now accept Discord handles instead of user IDs:
- Username: `john` or `@john`
- Username with discriminator: `john#1234`
- Server nickname: `Johnny`
- The bot searches all guilds to find matching users

**Cross-Guild Moderation**: All moderation commands work across ALL guilds where the bot is present:
1. Search for the user by handle across all guilds
2. Apply the moderation action to all applicable guilds
3. Report back with detailed results per guild

---

## Serenity-Specific Patterns

1. **Message Handling**: Always check for errors when sending messages
```rust
if let Err(why) = msg.channel_id.say(&ctx.http, "content").await {
    // Handle error
}
```

2. **Context Usage**: The `Context` provides access to HTTP client, cache, and shard manager
```rust
ctx.http     // For API calls
ctx.cache    // For cached data
ctx.shard    // For shard information
```

3. **Gateway Intents**: Only request what you need to minimize resource usage
```rust
GatewayIntents::GUILDS
    | GatewayIntents::GUILD_MESSAGES
    | GatewayIntents::MESSAGE_CONTENT
    | GatewayIntents::GUILD_VOICE_STATES
    | GatewayIntents::GUILD_MEMBERS
    | GatewayIntents::DIRECT_MESSAGES
```

---

## Important Considerations

### Discord API Limits

- **Rate Limits**: Serenity handles most rate limiting automatically
- **Message Size**: Max 2000 characters per message
- **Embed Limits**: Max 25 fields, 6000 total characters
- **Bulk Delete**: Can only delete messages < 14 days old

### Media Caching

- **Storage**: Files organized in `./media_cache/` by type (images, videos, audio, documents, other)
- **Naming**: Files renamed with UUIDs to avoid collisions
- **Toggle**: Can be enabled/disabled via `/cache` command or database setting
- **Cleanup**: Automatic deletion of files older than 31 days
- **Database**: Tracks all attachments with local paths when cached

### Performance Tips

- Use cache when possible instead of HTTP requests
- Minimize gateway intents
- Implement connection pooling for MariaDB via `sqlx`
- Use indexed fields for fast log retrieval
- Consider sharding for large-scale deployments
- Media caching can be disabled to save disk space

### Security

- `.env` file for sensitive config (never commit this)
- Validate whitelist status before executing mod commands
- Sanitize user inputs when logging content
- Consider encrypting or anonymizing exported logs

---

## Logging

### Console & File Logging

- **Console**: Pretty-printed logs with ANSI colors
- **File**: JSON-formatted logs in `logs/sentinel.log` with daily rotation
- **Location**: Log files stored in `logs/` directory (gitignored)
- **Format**: Structured JSON with timestamps, thread info, and metadata

### Database Logging

All interactions are logged to database:
- **DM Messages**: Stored in `dm_logs` table
- **Bot Responses**: Stored in `bot_response_logs` table with success/failure status
- **All Events**: Messages, voice, presence, joins/leaves, etc.

## Debugging

Enable debug logging by setting environment variable:
```bash
RUST_LOG=debug cargo run              # Debug everything
RUST_LOG=serenity=debug cargo run     # Debug only Serenity
RUST_LOG=sentinel=trace cargo run     # Trace your bot's code
```

Common issues:
- "Missing Access": Check bot permissions in Discord server
- "Missing Intent": Ensure gateway intents match your operations
- "Invalid Token": Verify DISCORD_TOKEN in .env file
- WebSocket disconnections: Usually transient, Serenity auto-reconnects

---

## Testing Approach

For Discord bots, consider:

1. **Unit tests**: Validate command parsing and message content
2. **Integration tests**: Simulate Discord events using mock contexts
3. **Manual testing**: Use a separate testing server
4. **Test bot token**: Isolate development from production

---

## Next Development Steps

- Implement command routing with `serenity::framework::standard`
- Slash command support
- Rich embed response for `/help` alert
- Admin web dashboard for viewing logs and managing whitelist
- Integration layer to export logs for Claude consumption
