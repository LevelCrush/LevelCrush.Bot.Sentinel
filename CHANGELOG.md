# Changelog

All notable changes to Sentinel Discord Bot will be documented in this file.

## [Unreleased] - 2025-06-10

### Added
- **Per-user cooldowns for `/snort` command** - Each user now has their own independent cooldown timer (default: 30 seconds)
  - Multiple users can snort simultaneously without blocking each other
  - Added `user_snort_cooldowns` table to track individual cooldowns

- **User autocomplete for moderation commands** - All user-targeting commands now provide smart autocomplete
  - Searches across usernames, global handles, and nicknames
  - Works with `/kick`, `/ban`, `/timeout`, and `/whitelist` commands
  - Returns up to 25 matching users from the database

### Improved
- **Human-readable snort counter** - The `/snort` command now displays counts in natural language
  - "once", "twice", "thrice" for 1-3 times
  - Written numbers like "four times", "five times" up to twenty
  - Numeric format for larger numbers

### Changed
- **Migrated all commands to Discord slash commands** - Modern Discord integration
  - `/help` - Shows available commands with permission-based visibility
  - `/kick` - Kick users from all guilds
  - `/ban` - Ban users from all guilds  
  - `/timeout` - Timeout users with configurable duration (1-40320 minutes)
  - `/cache` - Toggle media caching with on/off/status options
  - `/whitelist` - Manage command whitelist (super users only)
  - `/snort` - Fun command with global counter
  - Legacy DM commands still supported for backward compatibility

## [0.1.0] - 2025-06-09

### Initial Release
- **Core Discord bot functionality** using Serenity framework v0.12
- **Comprehensive logging system** for Discord activity
  - Message logging with edit tracking
  - Voice channel activity (join/leave/switch)
  - Thread and forum post creation
  - Direct message logging
  - User metadata tracking (username, discriminator, nickname)

### Added

#### Media & Attachment Features
- **Media file caching system** - Downloads and stores attachments locally
  - Organized by file type (images, videos, audio, documents)
  - Automatic cleanup of files older than 31 days
  - Toggle caching on/off via commands
  - UUID-based file naming to prevent collisions

#### Member & Channel Tracking  
- **Presence and status logging** - Tracks user online/offline status
  - Desktop, mobile, and web client status
  - Activity tracking (Playing, Streaming, Listening, etc.)
  - Per-guild presence monitoring

- **Advanced user tracking**
  - Nickname change monitoring with timestamps
  - Member join/leave events
  - Channel audit logs (create, delete, modify)
  - Permission and topic change tracking

#### Moderation System
- **Cross-guild moderation** - Commands work across ALL guilds where bot is present
  - User search by username, @handle, or server nickname
  - Detailed per-guild success/failure reporting
  - Reason tracking for all moderation actions

- **Permission hierarchy**
  - Super users: Full access including whitelist management
  - Whitelisted users: Access to moderation commands
  - Regular users: Access to help and fun commands only

- **Bot response logging** - All bot responses tracked in database
  - Command used, response type, content, and success status
  - Helps with debugging and audit trails

#### Fun Features
- **`/snort` command** - Global counter for "snorting brightdust"
  - Tracks total count across all users
  - Configurable cooldown system
  - Last user and guild tracking

#### Infrastructure
- **Raspberry Pi 5 support** - Cross-compilation for ARM64
  - Docker-based build system
  - Native build scripts
  - Deployment automation

- **Database schema** - MariaDB/MySQL backend
  - 10+ tables for comprehensive data storage
  - Proper indexing for performance
  - System settings table for configuration

- **Background jobs** - Automated maintenance tasks
  - User metadata synchronization
  - Media cache cleanup
  - Runs every 12-24 hours

- **Structured logging** 
  - Console output with pretty printing
  - JSON file logs with daily rotation
  - Separate log levels for different components

### Technical Details
- Written in Rust for performance and safety
- Async/await architecture using Tokio
- SQLx for database operations
- Environment-based configuration
- Comprehensive error handling and recovery

### Security
- Token-based authentication
- Whitelist system for sensitive commands
- No hardcoded credentials
- Input sanitization for database operations

---

*This changelog follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/) format*