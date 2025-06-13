# Database Migrations

This directory contains sqlx database migrations for the Sentinel Discord bot.

## Overview

Migrations are SQL scripts that define changes to the database schema. They allow for:
- Version control of database schema
- Reproducible database setup across environments
- Safe rollback of schema changes

## File Structure

- `{timestamp}_{name}.sql` - Forward migration (applies changes)
- `{timestamp}_{name}.down.sql` - Reverse migration (reverts changes)

## Usage

### Running Migrations

```bash
# Apply all pending migrations
sqlx migrate run

# Check migration status
sqlx migrate info

# Revert the last migration
sqlx migrate revert
```

### Creating New Migrations

```bash
# Create a new migration
sqlx migrate add <migration_name>

# Example:
sqlx migrate add add_user_avatars
```

This will create two files:
- `migrations/{timestamp}_add_user_avatars.sql` - Add your forward migration SQL here
- `migrations/{timestamp}_add_user_avatars.down.sql` - Add your reverse migration SQL here

## Current Schema

The initial migration (`20250612195032_initial_schema.sql`) creates all the base tables for Sentinel:

- **users** - Discord user tracking
- **command_whitelist** - Authorized moderators
- **super_user_whitelist** - Super users with admin privileges
- **message_logs** - All message history
- **voice_logs** - Voice channel activity
- **forum_logs** - Forum and thread posts
- **message_attachments** - Media attachment tracking
- **system_settings** - Bot configuration
- **member_status_logs** - User presence tracking
- **nickname_logs** - Nickname change history
- **channel_logs** - Channel modifications
- **dm_logs** - Direct message logs
- **bot_response_logs** - Bot response tracking
- **snort_counter** - Global snort counter
- **user_snort_cooldowns** - Per-user snort cooldowns
- **poll_logs**, **poll_answers**, **poll_votes** - Discord poll tracking
- **event_logs**, **event_interests**, **event_update_logs** - Discord event tracking
- **media_recommendations** - Media recommendation extraction
- **media_scan_checkpoint** - Scan progress tracking
- **user_watchlist** - Personal media watchlists
- **global_watchlist**, **global_watchlist_votes** - Community watchlist
- **channel_scan_history** - Channel scanning status
- **meme_folders** - Meme organization folders

## Best Practices

1. **Test migrations locally** before applying to production
2. **Always create down migrations** for rollback capability
3. **Use transactions** where possible for atomic changes
4. **Never modify existing migrations** that have been applied
5. **Document complex migrations** with comments

## Troubleshooting

If migrations fail:
1. Check the database connection in `.env`
2. Ensure the database user has CREATE/ALTER permissions
3. Check for syntax errors in the SQL
4. Review `sqlx migrate info` for current state

For manual intervention:
- The `_sqlx_migrations` table tracks applied migrations
- You can manually mark migrations as applied if needed