-- Initial schema migration for Sentinel Discord bot

-- Track all Discord users
CREATE TABLE IF NOT EXISTS users (
    id INT PRIMARY KEY AUTO_INCREMENT,
    discord_user_id BIGINT UNIQUE NOT NULL,
    username VARCHAR(255),
    discriminator VARCHAR(10),
    global_handle VARCHAR(255),
    nickname VARCHAR(255),
    last_seen DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_discord_user_id (discord_user_id),
    INDEX idx_username (username),
    INDEX idx_global_handle (global_handle)
);

-- Moderator whitelist
CREATE TABLE IF NOT EXISTS command_whitelist (
    id INT PRIMARY KEY AUTO_INCREMENT,
    discord_user_id BIGINT UNIQUE NOT NULL,
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_discord_user_id (discord_user_id)
);

-- Super user whitelist
CREATE TABLE IF NOT EXISTS super_user_whitelist (
    id INT PRIMARY KEY AUTO_INCREMENT,
    discord_user_id BIGINT UNIQUE NOT NULL,
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_discord_user_id (discord_user_id)
);

-- Message logs
CREATE TABLE IF NOT EXISTS message_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    message_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    guild_id BIGINT,
    content TEXT,
    timestamp DATETIME,
    edited BOOLEAN DEFAULT FALSE,
    edit_timestamp DATETIME,
    INDEX idx_message_id (message_id),
    INDEX idx_user_channel (user_id, channel_id),
    INDEX idx_timestamp (timestamp),
    INDEX idx_guild_id (guild_id)
);

-- Voice channel activity
CREATE TABLE IF NOT EXISTS voice_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    action ENUM('join', 'leave', 'switch') NOT NULL,
    timestamp DATETIME,
    INDEX idx_user_id (user_id),
    INDEX idx_channel_id (channel_id),
    INDEX idx_timestamp (timestamp),
    INDEX idx_guild_id (guild_id)
);

-- Forum and thread events
CREATE TABLE IF NOT EXISTS forum_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    thread_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    title TEXT,
    content TEXT,
    created_at DATETIME,
    INDEX idx_thread_id (thread_id),
    INDEX idx_user_id (user_id),
    INDEX idx_created_at (created_at),
    INDEX idx_guild_id (guild_id)
);

-- Direct message logs
CREATE TABLE IF NOT EXISTS dm_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    message_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    content TEXT,
    command VARCHAR(50),
    timestamp DATETIME,
    INDEX idx_user_id (user_id),
    INDEX idx_timestamp (timestamp)
);

-- Bot response logs
CREATE TABLE IF NOT EXISTS bot_response_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    channel_id BIGINT,
    command VARCHAR(50),
    response_type VARCHAR(50),
    response_content TEXT,
    success BOOLEAN DEFAULT TRUE,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_user_id (user_id),
    INDEX idx_timestamp (timestamp)
);

-- Snort counter
CREATE TABLE IF NOT EXISTS snort_counter (
    id INT PRIMARY KEY AUTO_INCREMENT,
    count BIGINT DEFAULT 0,
    last_snort_time DATETIME,
    last_snort_user_id BIGINT,
    last_snort_guild_id BIGINT
);

-- User snort cooldowns (per-user cooldown tracking)
CREATE TABLE IF NOT EXISTS user_snort_cooldowns (
    user_id BIGINT PRIMARY KEY,
    last_snort_time DATETIME NOT NULL
);

-- Discord polls tracking
CREATE TABLE IF NOT EXISTS poll_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    poll_id VARCHAR(255) NOT NULL,
    message_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    creator_id BIGINT NOT NULL,
    question TEXT,
    created_at DATETIME NOT NULL,
    expires_at DATETIME,
    closed_at DATETIME,
    is_multiselect BOOLEAN DEFAULT FALSE,
    INDEX idx_poll_id (poll_id),
    INDEX idx_message_id (message_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_creator_id (creator_id),
    INDEX idx_created_at (created_at)
);

CREATE TABLE IF NOT EXISTS poll_answers (
    id INT PRIMARY KEY AUTO_INCREMENT,
    poll_id VARCHAR(255) NOT NULL,
    answer_id INT NOT NULL,
    answer_text TEXT,
    emoji VARCHAR(255),
    INDEX idx_poll_answer (poll_id, answer_id)
);

CREATE TABLE IF NOT EXISTS poll_votes (
    id INT PRIMARY KEY AUTO_INCREMENT,
    poll_id VARCHAR(255) NOT NULL,
    user_id BIGINT NOT NULL,
    answer_id INT NOT NULL,
    voted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_poll_user (poll_id, user_id),
    INDEX idx_voted_at (voted_at),
    UNIQUE KEY unique_poll_user_answer (poll_id, user_id, answer_id)
);

-- Discord scheduled events tracking
CREATE TABLE IF NOT EXISTS event_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    event_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    channel_id BIGINT,
    creator_id BIGINT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    start_time DATETIME NOT NULL,
    end_time DATETIME,
    location VARCHAR(500),
    status VARCHAR(50),
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    INDEX idx_event_id (event_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_start_time (start_time),
    INDEX idx_status (status)
);

CREATE TABLE IF NOT EXISTS event_interests (
    id INT PRIMARY KEY AUTO_INCREMENT,
    event_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    interest_type ENUM('interested', 'maybe', 'not_interested', 'attending'),
    expressed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_event_user (event_id, user_id),
    UNIQUE KEY unique_event_user (event_id, user_id)
);

CREATE TABLE IF NOT EXISTS event_update_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    event_id BIGINT NOT NULL,
    field_name VARCHAR(100) NOT NULL,
    old_value TEXT,
    new_value TEXT,
    updated_by BIGINT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_event_id (event_id),
    INDEX idx_updated_at (updated_at)
);

-- Media recommendations tracking
CREATE TABLE IF NOT EXISTS media_recommendations (
    id INT PRIMARY KEY AUTO_INCREMENT,
    message_id BIGINT NOT NULL,
    user_id BIGINT NOT NULL,
    channel_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    media_type ENUM('anime', 'tv_show', 'movie', 'game', 'youtube', 'music', 'other') NOT NULL,
    title VARCHAR(500),
    url TEXT,
    confidence_score FLOAT DEFAULT 0.0,
    extracted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    message_timestamp DATETIME NOT NULL,
    INDEX idx_message_id (message_id),
    INDEX idx_user_id (user_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_media_type (media_type),
    INDEX idx_extracted_at (extracted_at),
    UNIQUE KEY unique_message_media (message_id, media_type, title)
);

CREATE TABLE IF NOT EXISTS media_scan_checkpoint (
    id INT PRIMARY KEY DEFAULT 1,
    last_scanned_message_id BIGINT NOT NULL DEFAULT 0,
    last_scan_time DATETIME DEFAULT CURRENT_TIMESTAMP,
    messages_scanned INT DEFAULT 0,
    recommendations_found INT DEFAULT 0
);

-- User personal media watchlist
CREATE TABLE IF NOT EXISTS user_watchlist (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    media_type ENUM('anime', 'tv_show', 'movie', 'game', 'youtube', 'music', 'other') NOT NULL,
    title VARCHAR(500) NOT NULL,
    url TEXT,
    priority INT DEFAULT 50,
    status ENUM('plan_to_watch', 'watching', 'completed', 'dropped', 'on_hold') DEFAULT 'plan_to_watch',
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    notes TEXT,
    UNIQUE KEY unique_user_media (user_id, media_type, title),
    INDEX idx_user_priority (user_id, priority DESC),
    INDEX idx_user_status (user_id, status)
);

-- Message attachments tracking
CREATE TABLE IF NOT EXISTS message_attachments (
    id INT PRIMARY KEY AUTO_INCREMENT,
    message_id BIGINT NOT NULL,
    attachment_id BIGINT NOT NULL,
    filename VARCHAR(255),
    content_type VARCHAR(100),
    size BIGINT,
    url TEXT,
    proxy_url TEXT,
    local_path VARCHAR(500),
    cached_at DATETIME,
    INDEX idx_message_id (message_id),
    INDEX idx_attachment_id (attachment_id),
    INDEX idx_cached_at (cached_at)
);

-- Settings table
CREATE TABLE IF NOT EXISTS system_settings (
    setting_key VARCHAR(100) PRIMARY KEY,
    setting_value TEXT,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
);

-- Member status logs
CREATE TABLE IF NOT EXISTS member_status_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    status VARCHAR(20),
    client_status_desktop VARCHAR(20),
    client_status_mobile VARCHAR(20),
    client_status_web VARCHAR(20),
    activity_type VARCHAR(50),
    activity_name TEXT,
    activity_details TEXT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_user_id (user_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_timestamp (timestamp)
);

-- Member nickname logs
CREATE TABLE IF NOT EXISTS nickname_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    old_nickname VARCHAR(255),
    new_nickname VARCHAR(255),
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_user_id (user_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_timestamp (timestamp)
);

-- Member join/leave logs
CREATE TABLE IF NOT EXISTS member_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    user_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    action ENUM('join', 'leave') NOT NULL,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_user_guild (user_id, guild_id),
    INDEX idx_timestamp (timestamp)
);

-- Channel update logs
CREATE TABLE IF NOT EXISTS channel_logs (
    id INT PRIMARY KEY AUTO_INCREMENT,
    channel_id BIGINT NOT NULL,
    guild_id BIGINT NOT NULL,
    action VARCHAR(50) NOT NULL,
    field_name VARCHAR(100),
    old_value TEXT,
    new_value TEXT,
    actor_id BIGINT,
    timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_channel_id (channel_id),
    INDEX idx_guild_id (guild_id),
    INDEX idx_action (action),
    INDEX idx_timestamp (timestamp)
);

-- Channel scan status
CREATE TABLE IF NOT EXISTS channel_scan_history (
    channel_id BIGINT PRIMARY KEY,
    guild_id BIGINT NOT NULL,
    scan_completed_at DATETIME NOT NULL,
    oldest_message_id BIGINT,
    messages_scanned INT DEFAULT 0,
    INDEX idx_guild_id (guild_id),
    INDEX idx_scan_completed_at (scan_completed_at)
);

-- Global watchlist
CREATE TABLE IF NOT EXISTS global_watchlist (
    id INT PRIMARY KEY AUTO_INCREMENT,
    media_type ENUM('anime', 'tv_show', 'movie', 'game', 'youtube', 'music', 'other') NOT NULL,
    title VARCHAR(500) NOT NULL,
    url TEXT,
    description TEXT,
    added_by BIGINT NOT NULL,
    added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
    UNIQUE KEY unique_media (media_type, title),
    INDEX idx_media_type (media_type),
    INDEX idx_added_by (added_by),
    INDEX idx_added_at (added_at)
);

-- Global watchlist votes
CREATE TABLE IF NOT EXISTS global_watchlist_votes (
    id INT PRIMARY KEY AUTO_INCREMENT,
    watchlist_id INT NOT NULL,
    user_id BIGINT NOT NULL,
    vote_type ENUM('up', 'down') DEFAULT 'up',
    voted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (watchlist_id) REFERENCES global_watchlist(id) ON DELETE CASCADE,
    UNIQUE KEY unique_user_vote (watchlist_id, user_id),
    INDEX idx_watchlist_id (watchlist_id),
    INDEX idx_user_id (user_id)
);

-- Meme folders
CREATE TABLE IF NOT EXISTS meme_folders (
    id INT PRIMARY KEY AUTO_INCREMENT,
    name VARCHAR(255) UNIQUE NOT NULL,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    INDEX idx_name (name)
);

-- Insert default settings
INSERT IGNORE INTO system_settings (setting_key, setting_value) VALUES
    ('cache_media', 'true'),
    ('snort_cooldown_seconds', '30');

-- Insert default snort counter
INSERT IGNORE INTO snort_counter (id, count) VALUES (1, 0);

-- Insert default media scan checkpoint
INSERT IGNORE INTO media_scan_checkpoint (id, last_scanned_message_id, messages_scanned, recommendations_found) 
VALUES (1, 0, 0, 0);