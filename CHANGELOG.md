# Changelog

All notable changes to Sentinel Discord Bot will be documented in this file.

## [Unreleased] - 2025-06-11

### Added
- **Super User Meme Management** - DM media attachments to organize into folders
  - Super users can DM images, videos, or GIFs to the bot
  - Supports multiple attachments in a single message
  - Bot responds with a multi-select poll to choose storage folders
  - Poll closes immediately upon voting for instant feedback
  - Automatically lists all subfolders in the `memes/` directory
  - Option to create new folders on the fly
  - Files are saved with unique UUIDs to prevent conflicts
  - Supports multiple folder selection for cross-categorization
  - Progress feedback during download and save operations
  - Files can be saved to multiple folders simultaneously

- **Personal Media Watchlist** - Track and manage your entertainment backlog
  - `/watchlist view` - See your personal watchlist or top community recommendations
  - `/watchlist add` - Add media to your watchlist with type, title, URL, and priority
  - `/watchlist remove` - Remove items from your watchlist
  - `/watchlist priority` - Reprioritize items in your watchlist
  - `/watchlist scan` - Scan the current channel's last 100 messages for media mentions
  - Priority-based sorting (1-100 scale)
  - Status tracking (plan to watch, watching, completed, etc.)
  - Real-time progress updates during channel scanning
  - Shows who mentioned each media item and how many times
  - Integrates with media recommendations from message scanning

- **Media Recommendations Scanner** - Intelligent content analysis for media mentions
  - Real-time detection in new messages, edited messages, polls, and events
  - Background scan runs every 30 minutes for historical data
  - Detects anime, TV shows, movies, games, and YouTube links
  - Pattern matching for recommendation context ("watching", "recommend", "check out")
  - Tracks confidence scores based on context strength
  - Extracts URLs when mentioned alongside media titles
  - Incremental scanning with checkpoint tracking
  - Stores findings in `media_recommendations` table for analysis
- **Discord Logs Cleanup Job** - Automatic cleanup of old data
  - Runs daily at 4 AM to remove logs older than 31 days
  - Cleans member status logs, nickname logs, and voice logs
  - Removes old poll votes for closed polls and expired event data
  - Helps maintain database performance and manage storage
  - Configurable retention period (currently 31 days)

- **Discord Poll Tracking** - Comprehensive logging of Discord's native poll feature
  - Logs poll creation with question, answers, and expiry time
  - Tracks all votes and vote removals in real-time
  - Stores poll configuration (multiselect allowed, emojis)
  - Background job to close expired polls automatically
  - Poll data linked to messages and channels

- **Discord Scheduled Events Tracking** - Full monitoring of server events
  - Logs event creation with name, description, time, and location
  - Tracks event status (scheduled, active, completed, cancelled)
  - Records user interest/RSVP (interested, maybe, not interested, attending)
  - Monitors all event updates and changes with history
  - Logs event deletions

- **Historical message scanning** - Background job that scans all channels for historical messages
  - Runs hourly and scans up to 5 channels per run
  - Fetches up to 10,000 messages per channel going back as far as Discord allows
  - Skips bot messages and channels that have already been scanned
  - Does not cache media attachments for historical messages
  - Tracks scan progress in `channel_scan_history` table
  - Handles rate limiting and permission errors gracefully
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

- **Snort meme attachments** - The `/snort` command now attaches a random meme image
  - Only attaches memes when successfully incrementing the counter (not on cooldown)
  - Cooldown messages are ephemeral (only visible to the user)
  - Successful snorts with memes are visible to everyone
  - Reads images from `memes/snort` directory
  - Supports jpg, png, gif, and webp formats
  - Directory created automatically on startup
  - Memes directory excluded from version control (except README.md)

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