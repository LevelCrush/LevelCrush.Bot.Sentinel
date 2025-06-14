# Changelog

All notable changes to Sentinel Discord Bot will be documented in this file.

## [Unreleased] - 2025-06-15

### Added

- **GIPHY Integration for `/snort` Command** - Enhanced meme support with GIPHY API
  - Automatic fetching of Destiny-themed memes from GIPHY
  - Smart caching system to reduce API calls and improve performance
  - Database-driven search terms with configurable priorities
  - Only uses top 10 most relevant results for quality control
  - Intelligent source selection: 60% GIPHY, 40% local files
  - Prevents back-to-back repeats by tracking last used meme
  - Falls back to local files if GIPHY fails or has no results
  - Cached GIFs displayed as embeds with "Powered by GIPHY" footer
  - New database tables: `giphy_search_terms` and `giphy_cache`
  - Background job cleans up unused cache entries after 7 days
  - Default search terms include "Destiny memes", "Destiny 2 memes", etc.
  - Search terms can be managed directly in the database

### Added (continued from 2025-06-12)
- **Global Community Watchlist** - Collaborative watchlist with voting system
  - `/global view [type]` - View the global watchlist, optionally filtered by media type
  - `/global add` - Add media to the global watchlist with title, type, URL, and description
  - `/global vote` - Vote on items (upvote/downvote/remove vote) to influence priority
  - `/global search` - Search the global watchlist by title or description
  - Items are automatically sorted by net votes (upvotes - downvotes)
  - Shows who added each item and current vote counts
  - Users automatically upvote items they add
  - Vote changes update item priority in real-time

- **Media Recommendations Export** - Export your watchlist and recommendations in multiple formats
  - `/watchlist export` - Export your personal watchlist or community recommendations
  - Choose between CSV, JSON, or Markdown formats
  - Watchlist exports include: type, title, URL, priority, status, and notes
  - Recommendations export includes: type, title, URL, confidence score, mention count, and who recommended it
  - Customizable time period for recommendations (1-365 days)
  - Files are generated with timestamps and sent as Discord attachments
  - Ephemeral responses ensure privacy

- **Super User Meme Management** - DM media attachments to organize into folders
  - Super users can DM images, videos, or GIFs to the bot
  - Supports multiple attachments in a single message
  - Bot responds with interactive buttons for folder selection
  - Instant processing when a folder is selected
  - Real-time progress updates during file processing
  - Automatically lists all subfolders in the `memes/` directory
  - Files are saved with unique UUIDs
  - Zone.Identifier files are automatically filtered out
  - Progress feedback during download and save operations
  - Discord's button interface provides better UX than polls

- **Personal Media Watchlist** - Track and manage your entertainment backlog
  - `/watchlist view` - See your personal watchlist or top community recommendations
  - `/watchlist add` - Add media to your watchlist with type, title, URL, and priority
  - `/watchlist remove` - Remove items from your watchlist
  - `/watchlist priority` - Reprioritize items in your watchlist
  - Priority-based sorting (1-100 scale)
  - Status tracking (plan to watch, watching, completed, etc.)
  - Shows top community recommendations based on automated detection

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

### Added (continued)
- **Database Migration System** - Implemented sqlx migrations for schema management
  - Replaced manual table creation with versioned migration files
  - Added `migrations/` directory with initial schema migration
  - Includes both up and down migrations for rollback capability
  - Migration status tracking with `_sqlx_migrations` table
  - Updated documentation with migration commands and best practices
  - Build scripts now include migration deployment instructions

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

- **Global watchlist voting now uses autocomplete** - Improved user experience
  - Changed from numeric ID input to searchable item selection
  - Autocomplete shows emoji, title, media type, and net votes
  - Search by partial title match as you type
  - Vote confirmation shows item title instead of ID number

### Fixed
- **Database Schema Inconsistencies** - Aligned table and column names
  - Fixed `system_settings` vs `settings` table name mismatch
  - Corrected `nickname_logs` vs `member_nickname_logs` table name
  - Fixed `channel_scan_history` vs `channel_scan_status` table name
  - Aligned `cached_at` vs `downloaded_at` column in message_attachments
  - Fixed member_status_logs structure to match actual usage
  - Corrected vote type ENUM values in global_watchlist_votes
  - Added missing `actor_id` column to channel_logs table

- **Global watchlist SQL type mismatches** - Fixed database query errors
  - Changed ID column type from `u64` to `i32` to match MariaDB `INT`
  - Added `CAST(... AS SIGNED)` for SUM aggregates returning DECIMAL
  - Fixed "Reference 'upvotes' not supported" error in ORDER BY clause
  - Updated all related functions to handle proper type conversions

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