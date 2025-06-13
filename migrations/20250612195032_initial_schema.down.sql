-- Reverse migration for initial schema

-- Drop tables in reverse order of dependencies
DROP TABLE IF EXISTS global_watchlist_votes;
DROP TABLE IF EXISTS global_watchlist;
DROP TABLE IF EXISTS meme_folders;
DROP TABLE IF EXISTS channel_scan_history;
DROP TABLE IF EXISTS channel_logs;
DROP TABLE IF EXISTS member_logs;
DROP TABLE IF EXISTS nickname_logs;
DROP TABLE IF EXISTS member_status_logs;
DROP TABLE IF EXISTS system_settings;
DROP TABLE IF EXISTS message_attachments;
DROP TABLE IF EXISTS user_watchlist;
DROP TABLE IF EXISTS media_scan_checkpoint;
DROP TABLE IF EXISTS media_recommendations;
DROP TABLE IF EXISTS event_update_logs;
DROP TABLE IF EXISTS event_interests;
DROP TABLE IF EXISTS event_logs;
DROP TABLE IF EXISTS poll_votes;
DROP TABLE IF EXISTS poll_answers;
DROP TABLE IF EXISTS poll_logs;
DROP TABLE IF EXISTS user_snort_cooldowns;
DROP TABLE IF EXISTS snort_counter;
DROP TABLE IF EXISTS bot_response_logs;
DROP TABLE IF EXISTS dm_logs;
DROP TABLE IF EXISTS forum_logs;
DROP TABLE IF EXISTS voice_logs;
DROP TABLE IF EXISTS message_logs;
DROP TABLE IF EXISTS super_user_whitelist;
DROP TABLE IF EXISTS command_whitelist;
DROP TABLE IF EXISTS users;