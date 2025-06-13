
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
- **Poll tracking**: Logs poll creation, votes, and expiry
- **Event tracking**: Monitors Discord scheduled events and user interest/RSVPs
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
echo "DATABASE_URL=mysql://root:password@localhost/sentinel" >> .env

# Install sqlx-cli (if not already installed)
cargo install sqlx-cli --no-default-features --features mysql

# Run database migrations
sqlx migrate run

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

# Database commands
sqlx migrate run         # Apply pending migrations
sqlx migrate revert      # Revert last migration
sqlx migrate info        # Show migration status
sqlx migrate add <name>  # Create new migration
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

### Database Migrations

The project uses sqlx migrations for database schema management:
- Migrations are stored in the `migrations/` directory
- Each migration has an up (`.sql`) and down (`.down.sql`) file
- Migrations are automatically applied when calling `db.run_migrations()`
- The initial schema migration creates all necessary tables and indexes

---

## Database Schema (MariaDB)

The database schema is managed through sqlx migrations. The full schema definition can be found in:
- `migrations/20250612195032_initial_schema.sql` - Complete initial schema
- `migrations/20250612195032_initial_schema.down.sql` - Rollback migration

### Main Tables Overview:

**User Management:**
- `users` - Discord user profiles with usernames, handles, and nicknames
- `command_whitelist` - Users authorized for moderation commands
- `super_user_whitelist` - Users with admin privileges

**Message & Communication:**
- `message_logs` - All message content with edit tracking
- `message_attachments` - Media attachment metadata and local paths
- `voice_logs` - Voice channel activity (join/leave/switch)
- `forum_logs` - Thread and forum post creation
- `dm_logs` - Direct messages to the bot
- `bot_response_logs` - Bot command responses

**Member Tracking:**
- `member_status_logs` - User presence and activity tracking
- `nickname_logs` - Nickname change history
- `member_logs` - Join/leave events
- `channel_logs` - Channel modifications and audit trail

**Interactive Features:**
- `poll_logs`, `poll_answers`, `poll_votes` - Discord poll tracking
- `event_logs`, `event_interests`, `event_update_logs` - Discord event tracking
- `snort_counter`, `user_snort_cooldowns` - Snort command tracking

**Media & Recommendations:**
- `media_recommendations` - Extracted media mentions from messages
- `media_scan_checkpoint` - Scan progress tracking
- `user_watchlist` - Personal media watchlists
- `global_watchlist`, `global_watchlist_votes` - Community watchlist

**System:**
- `system_settings` - Configurable bot settings
- `channel_scan_history` - Historical message scan progress
- `meme_folders` - Meme organization folders

---

## Background Jobs

Sentinel runs several scheduled jobs using `tokio_cron_scheduler`:

1. **User Sync Job** (every 12 hours):
   - Updates the `users` table with current usernames, discriminators, handles, and nicknames
   - Syncs all members across all guilds

2. **Media Cleanup Job** (daily at 3 AM):
   - Deletes cached media files older than 31 days
   - Only runs if media caching is enabled

3. **Discord Logs Cleanup** (daily at 4 AM):
   - Removes logs older than 31 days to manage database size
   - Cleans up: member status logs, nickname logs, voice logs
   - Also removes old poll votes (for closed polls) and event data
   - Keeps database performant by preventing unbounded growth

4. **Channel History Scan** (hourly):
   - Scans up to 5 unscanned channels per run
   - Retrieves historical messages (up to 10,000 per channel)
   - Helps capture messages sent before the bot joined

5. **Poll Expiry Check** (hourly):
   - Marks expired polls as closed
   - Ensures poll results are finalized when time expires

6. **Media Recommendations Scan** (every 30 minutes):
   - Scans message logs for media mentions (anime, TV shows, games, YouTube videos)
   - Uses pattern matching to identify recommendations
   - Tracks confidence scores and URLs
   - Incremental scanning from last checkpoint

This keeps logs cross-referenced with accurate identity metadata for auditing or AI training.

---

## Commands

All commands are implemented as Discord slash commands:

### Slash Commands

| Command                          | Description                             | Access           |
|----------------------------------|-----------------------------------------|------------------|
| `/help`                          | Show available commands                 | Anyone           |
| `/kick <user> [reason]`          | Kick user from all connected servers    | Whitelisted only |
| `/ban <user> [reason]`           | Ban user from all connected servers     | Whitelisted only |
| `/timeout <user> <duration> [reason]` | Timeout user in all servers (1-40320 mins) | Whitelisted only |
| `/cache [action]`                | Toggle/check media caching (on/off/status) | Whitelisted only |
| `/whitelist <action> <user>`     | Manage command whitelist (add/remove)   | Super users only |
| `/snort`                         | Snort brightdust! Tracks global count   | Anyone           |
| `/watchlist [action]`            | Manage personal media watchlist         | Anyone           |
| `/global [action]`               | Manage global community watchlist       | Anyone           |

**User Autocomplete**: All commands that target users (`/kick`, `/ban`, `/timeout`, `/whitelist`) provide autocomplete suggestions from the database. Start typing a username, handle, or nickname to see matching users.

**Global Watchlist Autocomplete**: The `/global vote` command now uses autocomplete for item selection instead of numeric IDs. Start typing part of an item's title to see suggestions showing the emoji, title, media type, and current net votes.

**Snort Cooldown**: The `/snort` command has a per-user cooldown (default: 30 seconds). Each user can only snort once per cooldown period, but multiple users can snort simultaneously.

**Watchlist Features**: The `/watchlist` command provides personal media tracking:
- `/watchlist view [all]` - View your personal watchlist or top community recommendations (use "all" to see community picks)
- `/watchlist add <type> <title> [url] [priority]` - Add media to your watchlist with optional URL and priority (1-100)
- `/watchlist remove <type> <title>` - Remove an item from your watchlist
- `/watchlist priority <type> <title> <new_priority>` - Update priority of an existing item
- `/watchlist export <data> <format> [days]` - Export your watchlist or recommendations
  - Data options: `watchlist` (your personal list), `recommendations` (community picks), or `global` (global watchlist)
  - Format options: `CSV`, `JSON`, or `Markdown`
  - Days: For recommendations, specify how many days of data to include (1-365, default: 30)

**Global Watchlist Features**: The `/global` command provides collaborative media tracking:
- `/global view [type]` - View the global community watchlist
  - Optional type filter: `anime`, `tv_show`, `movie`, `game`, `youtube`, `music`, `other`, or `all`
  - Items are sorted by net votes (upvotes - downvotes)
  - Shows item ID, type, title, votes, description, URL, and who added it
- `/global add <type> <title> [url] [description]` - Add media to the global watchlist
  - Automatically upvotes the item you add
  - Duplicate titles of the same type update the existing entry
- `/global vote <item> <vote>` - Vote on global watchlist items
  - Item selection uses autocomplete - start typing to search by title
  - Vote options: `upvote`, `downvote`, or `remove` (to remove your vote)
  - Items with more net votes appear higher in the list
- `/global search <query>` - Search the global watchlist by title or description

### Legacy DM Support

The bot still processes DM commands for backward compatibility, but slash commands are the preferred method.

**Super User DM Features**:
- Send media attachments (images/videos/GIFs) directly to the bot via DM
- Bot creates a multi-select poll to organize media into meme folders
- Automatically scans `memes/` directory for existing folders
- Option to create new folders by selecting "📁 Create new folder"
- Files are saved with UUID filenames to prevent conflicts
- Multiple folders can be selected to save the same meme in multiple locations

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

**Configurable Settings**: System settings stored in database:
- `cache_media`: Enable/disable media caching (default: 'true')
- `snort_cooldown_seconds`: Global cooldown for /snort command (default: '30')

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
    | GatewayIntents::GUILD_MESSAGE_POLLS
    | GatewayIntents::GUILD_SCHEDULED_EVENTS
    | GatewayIntents::GUILD_PRESENCES
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

## Cross-Compilation for Raspberry Pi 5

The project includes support for cross-compiling to ARM64 (aarch64) architecture:

### Setup
- `.cargo/config.toml`: Contains linker configuration for aarch64 targets
- `build-pi5.sh`: Native cross-compilation script (requires gcc-aarch64-linux-gnu)
- `build-pi5-docker.sh`: Docker-based build (no tools installation required)

### Building
```bash
# Method 1: Native tools
./build-pi5.sh

# Method 2: Docker
./build-pi5-docker.sh
```

### Deployment
The scripts produce a binary at:
- Native: `target/aarch64-unknown-linux-gnu/release/sentinel`
- Docker: `./sentinel-pi5`

Deploy with:
```bash
scp <binary> pi@<ip>:~/sentinel
scp .env pi@<ip>:~/
```

---

## Database Migration Guidelines

When making database schema changes:
1. **Create a new migration**: `sqlx migrate add <descriptive_name>`
2. **Write the up migration**: Add your schema changes to the `.sql` file
3. **Write the down migration**: Add rollback logic to the `.down.sql` file
4. **Test locally**: Run `sqlx migrate run` and verify changes
5. **Test rollback**: Run `sqlx migrate revert` to ensure it works
6. **Never modify existing migrations** that have been applied to production

## Next Development Steps

- Implement command routing with `serenity::framework::standard`
- Slash command support
- Rich embed response for `/help` alert
- Admin web dashboard for viewing logs and managing whitelist
- Integration layer to export logs for Claude consumption
