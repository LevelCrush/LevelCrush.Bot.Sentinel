use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{MySql, Pool};

#[derive(Clone)]
pub struct Database {
    pub pool: Pool<MySql>,
}

impl Database {
    pub async fn new(database_url: &str) -> Result<Self> {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn run_migrations(&self) -> Result<()> {
        sqlx::query(
            r#"
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
                INDEX idx_last_seen (last_seen)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS command_whitelist (
                id INT PRIMARY KEY AUTO_INCREMENT,
                discord_user_id BIGINT UNIQUE NOT NULL,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_discord_user_id (discord_user_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS message_logs (
                id INT PRIMARY KEY AUTO_INCREMENT,
                message_id BIGINT NOT NULL,
                user_id BIGINT NOT NULL,
                channel_id BIGINT NOT NULL,
                content TEXT,
                timestamp DATETIME,
                edited BOOLEAN DEFAULT FALSE,
                INDEX idx_message_id (message_id),
                INDEX idx_user_id (user_id),
                INDEX idx_channel_id (channel_id),
                INDEX idx_timestamp (timestamp)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS voice_logs (
                id INT PRIMARY KEY AUTO_INCREMENT,
                user_id BIGINT NOT NULL,
                channel_id BIGINT NOT NULL,
                action ENUM('join', 'leave', 'switch'),
                timestamp DATETIME,
                INDEX idx_user_id (user_id),
                INDEX idx_channel_id (channel_id),
                INDEX idx_timestamp (timestamp)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS forum_logs (
                id INT PRIMARY KEY AUTO_INCREMENT,
                thread_id BIGINT NOT NULL,
                user_id BIGINT NOT NULL,
                title TEXT,
                content TEXT,
                created_at DATETIME,
                INDEX idx_thread_id (thread_id),
                INDEX idx_user_id (user_id),
                INDEX idx_created_at (created_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS system_settings (
                setting_key VARCHAR(100) PRIMARY KEY,
                setting_value TEXT,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Insert default setting for media caching
        sqlx::query(
            "INSERT IGNORE INTO system_settings (setting_key, setting_value) VALUES ('cache_media', 'true')"
        )
        .execute(&self.pool)
        .await?;

        // Member status/presence logs
        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Nickname change logs
        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Channel modification logs
        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // DM logs
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS dm_logs (
                id INT PRIMARY KEY AUTO_INCREMENT,
                message_id BIGINT NOT NULL,
                user_id BIGINT NOT NULL,
                content TEXT,
                command VARCHAR(50),
                timestamp DATETIME,
                INDEX idx_message_id (message_id),
                INDEX idx_user_id (user_id),
                INDEX idx_command (command),
                INDEX idx_timestamp (timestamp)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Bot response logs
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS bot_response_logs (
                id INT PRIMARY KEY AUTO_INCREMENT,
                user_id BIGINT NOT NULL,
                command VARCHAR(50),
                response_type VARCHAR(50),
                response_content TEXT,
                success BOOLEAN DEFAULT TRUE,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_user_id (user_id),
                INDEX idx_command (command),
                INDEX idx_timestamp (timestamp)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Super user whitelist
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS super_user_whitelist (
                id INT PRIMARY KEY AUTO_INCREMENT,
                discord_user_id BIGINT UNIQUE NOT NULL,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_discord_user_id (discord_user_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Snort counter table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS snort_counter (
                id INT PRIMARY KEY AUTO_INCREMENT,
                count BIGINT DEFAULT 0,
                last_snort_time DATETIME,
                last_snort_user_id BIGINT,
                last_snort_guild_id BIGINT
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Initialize snort counter if it doesn't exist
        sqlx::query("INSERT IGNORE INTO snort_counter (id, count) VALUES (1, 0)")
            .execute(&self.pool)
            .await?;

        // Add default snort cooldown setting (in seconds)
        sqlx::query(
            "INSERT IGNORE INTO system_settings (setting_key, setting_value) VALUES ('snort_cooldown_seconds', '30')"
        )
        .execute(&self.pool)
        .await?;

        // Create user snort cooldowns table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_snort_cooldowns (
                user_id BIGINT PRIMARY KEY,
                last_snort_time DATETIME NOT NULL,
                INDEX idx_last_snort_time (last_snort_time)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Create channel scan history table to track which channels have been scanned for historical messages
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS channel_scan_history (
                channel_id BIGINT PRIMARY KEY,
                guild_id BIGINT NOT NULL,
                scan_completed_at DATETIME NOT NULL,
                oldest_message_id BIGINT,
                messages_scanned INT DEFAULT 0,
                INDEX idx_guild_id (guild_id),
                INDEX idx_scan_completed_at (scan_completed_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Poll tracking table
        sqlx::query(
            r#"
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
                INDEX idx_channel_id (channel_id),
                INDEX idx_guild_id (guild_id),
                INDEX idx_creator_id (creator_id),
                INDEX idx_expires_at (expires_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Poll answer options
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS poll_answers (
                id INT PRIMARY KEY AUTO_INCREMENT,
                poll_id VARCHAR(255) NOT NULL,
                answer_id INT NOT NULL,
                answer_text TEXT,
                emoji VARCHAR(255),
                UNIQUE KEY unique_poll_answer (poll_id, answer_id),
                INDEX idx_poll_id (poll_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Poll votes/interactions
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS poll_votes (
                id INT PRIMARY KEY AUTO_INCREMENT,
                poll_id VARCHAR(255) NOT NULL,
                user_id BIGINT NOT NULL,
                answer_id INT NOT NULL,
                voted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                INDEX idx_poll_id (poll_id),
                INDEX idx_user_id (user_id),
                INDEX idx_voted_at (voted_at),
                UNIQUE KEY unique_user_vote (poll_id, user_id, answer_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Discord server events table
        sqlx::query(
            r#"
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
                INDEX idx_creator_id (creator_id),
                INDEX idx_start_time (start_time),
                INDEX idx_status (status)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Event interest/RSVP tracking
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS event_interests (
                id INT PRIMARY KEY AUTO_INCREMENT,
                event_id BIGINT NOT NULL,
                user_id BIGINT NOT NULL,
                interest_type ENUM('interested', 'maybe', 'not_interested', 'attending'),
                expressed_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE KEY unique_user_event (event_id, user_id),
                INDEX idx_event_id (event_id),
                INDEX idx_user_id (user_id),
                INDEX idx_expressed_at (expressed_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Event update history
        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Media recommendations tracking
        sqlx::query(
            r#"
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
                UNIQUE KEY unique_message_media (message_id, media_type, title),
                INDEX idx_media_type (media_type),
                INDEX idx_user_id (user_id),
                INDEX idx_extracted_at (extracted_at),
                INDEX idx_confidence (confidence_score)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Track last scanned message for incremental scanning
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS media_scan_checkpoint (
                id INT PRIMARY KEY DEFAULT 1,
                last_scanned_message_id BIGINT NOT NULL DEFAULT 0,
                last_scan_time DATETIME DEFAULT CURRENT_TIMESTAMP,
                messages_scanned INT DEFAULT 0,
                recommendations_found INT DEFAULT 0
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Initialize checkpoint if not exists
        sqlx::query("INSERT IGNORE INTO media_scan_checkpoint (id) VALUES (1)")
            .execute(&self.pool)
            .await?;

        // User watchlist table
        sqlx::query(
            r#"
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
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Global watchlist table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS global_watchlist (
                id INT PRIMARY KEY AUTO_INCREMENT,
                media_type ENUM('anime', 'tv_show', 'movie', 'game', 'youtube', 'music', 'other') NOT NULL,
                title VARCHAR(500) NOT NULL,
                url TEXT,
                added_by BIGINT NOT NULL,
                added_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                updated_at DATETIME DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP,
                description TEXT,
                UNIQUE KEY unique_global_media (media_type, title),
                INDEX idx_media_type (media_type),
                INDEX idx_added_at (added_at)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        // Global watchlist votes table
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS global_watchlist_votes (
                id INT PRIMARY KEY AUTO_INCREMENT,
                watchlist_id INT NOT NULL,
                user_id BIGINT NOT NULL,
                vote_type ENUM('up', 'down') DEFAULT 'up',
                voted_at DATETIME DEFAULT CURRENT_TIMESTAMP,
                UNIQUE KEY unique_user_vote (watchlist_id, user_id),
                FOREIGN KEY (watchlist_id) REFERENCES global_watchlist(id) ON DELETE CASCADE,
                INDEX idx_watchlist_id (watchlist_id),
                INDEX idx_user_id (user_id)
            )
            "#,
        )
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_message(
        &self,
        message_id: u64,
        user_id: u64,
        channel_id: u64,
        content: &str,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO message_logs (message_id, user_id, channel_id, content, timestamp) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(message_id as i64)
        .bind(user_id as i64)
        .bind(channel_id as i64)
        .bind(content)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_message_edit(&self, message_id: u64, new_content: &str) -> Result<()> {
        sqlx::query("UPDATE message_logs SET content = ?, edited = TRUE WHERE message_id = ?")
            .bind(new_content)
            .bind(message_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn log_voice_event(&self, user_id: u64, channel_id: u64, action: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO voice_logs (user_id, channel_id, action, timestamp) VALUES (?, ?, ?, NOW())"
        )
        .bind(user_id as i64)
        .bind(channel_id as i64)
        .bind(action)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_forum_thread(
        &self,
        thread_id: u64,
        user_id: u64,
        title: &str,
        content: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO forum_logs (thread_id, user_id, title, content, created_at) VALUES (?, ?, ?, ?, NOW())"
        )
        .bind(thread_id as i64)
        .bind(user_id as i64)
        .bind(title)
        .bind(content)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn update_user(
        &self,
        user_id: u64,
        username: &str,
        discriminator: Option<&str>,
        global_handle: Option<&str>,
        nickname: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO users (discord_user_id, username, discriminator, global_handle, nickname, last_seen)
            VALUES (?, ?, ?, ?, ?, NOW())
            ON DUPLICATE KEY UPDATE
                username = VALUES(username),
                discriminator = VALUES(discriminator),
                global_handle = VALUES(global_handle),
                nickname = VALUES(nickname),
                last_seen = NOW()
            "#
        )
        .bind(user_id as i64)
        .bind(username)
        .bind(discriminator)
        .bind(global_handle)
        .bind(nickname)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn is_whitelisted(&self, user_id: u64) -> Result<bool> {
        // Check if user is a super user first
        if self.is_super_user(user_id).await? {
            return Ok(true);
        }

        // Check regular whitelist
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM command_whitelist WHERE discord_user_id = ?",
        )
        .bind(user_id as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(result > 0)
    }

    pub async fn is_super_user(&self, user_id: u64) -> Result<bool> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM super_user_whitelist WHERE discord_user_id = ?",
        )
        .bind(user_id as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(result > 0)
    }

    pub async fn search_users(
        &self,
        query: &str,
        limit: u64,
    ) -> Result<Vec<(u64, String, Option<String>, Option<String>)>> {
        let search_pattern = format!("%{}%", query);

        let results = sqlx::query!(
            r#"
            SELECT DISTINCT discord_user_id, username, global_handle, nickname
            FROM users
            WHERE username LIKE ? 
               OR global_handle LIKE ?
               OR nickname LIKE ?
            ORDER BY 
                CASE 
                    WHEN username LIKE ? THEN 1
                    WHEN global_handle LIKE ? THEN 2
                    WHEN nickname LIKE ? THEN 3
                END,
                last_seen DESC
            LIMIT ?
            "#,
            search_pattern,
            search_pattern,
            search_pattern,
            query,
            query,
            query,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|r| {
                (
                    r.discord_user_id as u64,
                    r.username.unwrap_or_else(|| "Unknown".to_string()),
                    r.global_handle,
                    r.nickname,
                )
            })
            .collect())
    }

    pub async fn add_to_whitelist(&self, user_id: u64) -> Result<()> {
        sqlx::query("INSERT IGNORE INTO command_whitelist (discord_user_id) VALUES (?)")
            .bind(user_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn remove_from_whitelist(&self, user_id: u64) -> Result<()> {
        sqlx::query("DELETE FROM command_whitelist WHERE discord_user_id = ?")
            .bind(user_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn add_to_super_whitelist(&self, user_id: u64) -> Result<()> {
        sqlx::query("INSERT IGNORE INTO super_user_whitelist (discord_user_id) VALUES (?)")
            .bind(user_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn remove_from_super_whitelist(&self, user_id: u64) -> Result<()> {
        sqlx::query("DELETE FROM super_user_whitelist WHERE discord_user_id = ?")
            .bind(user_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn log_attachment(
        &self,
        message_id: u64,
        attachment_id: u64,
        filename: &str,
        content_type: Option<&str>,
        size: u64,
        url: &str,
        proxy_url: &str,
        local_path: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO message_attachments 
            (message_id, attachment_id, filename, content_type, size, url, proxy_url, local_path, cached_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, NOW())
            "#
        )
        .bind(message_id as i64)
        .bind(attachment_id as i64)
        .bind(filename)
        .bind(content_type)
        .bind(size as i64)
        .bind(url)
        .bind(proxy_url)
        .bind(local_path)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_setting(&self, key: &str) -> Result<Option<String>> {
        let result = sqlx::query_scalar::<_, String>(
            "SELECT setting_value FROM system_settings WHERE setting_key = ?",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_settings (setting_key, setting_value)
            VALUES (?, ?)
            ON DUPLICATE KEY UPDATE setting_value = VALUES(setting_value)
            "#,
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_setting(&self, key: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM system_settings
            WHERE setting_key = ?
            "#,
        )
        .bind(key)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_all_settings(&self) -> Result<Vec<(String, String)>> {
        let settings: Vec<(String, String)> = sqlx::query_as(
            "SELECT setting_key, setting_value FROM system_settings"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(settings)
    }

    pub async fn get_old_cached_media(&self, days: i64) -> Result<Vec<String>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);

        let paths = sqlx::query_scalar::<_, String>(
            "SELECT local_path FROM message_attachments WHERE cached_at < ? AND local_path IS NOT NULL"
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        Ok(paths)
    }

    pub async fn clear_local_path(&self, attachment_id: u64) -> Result<()> {
        sqlx::query("UPDATE message_attachments SET local_path = NULL WHERE attachment_id = ?")
            .bind(attachment_id as i64)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn log_member_status(
        &self,
        user_id: u64,
        guild_id: u64,
        status: Option<&str>,
        client_status: Option<(&str, &str, &str)>,
        activity: Option<(&str, &str, Option<&str>)>,
    ) -> Result<()> {
        let (desktop, mobile, web) = client_status.unwrap_or(("offline", "offline", "offline"));
        let (activity_type, activity_name, activity_details) =
            activity.unwrap_or(("None", "", None));

        sqlx::query(
            r#"
            INSERT INTO member_status_logs 
            (user_id, guild_id, status, client_status_desktop, client_status_mobile, client_status_web, 
             activity_type, activity_name, activity_details)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .bind(status)
        .bind(desktop)
        .bind(mobile)
        .bind(web)
        .bind(activity_type)
        .bind(activity_name)
        .bind(activity_details)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_nickname_change(
        &self,
        user_id: u64,
        guild_id: u64,
        old_nickname: Option<&str>,
        new_nickname: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO nickname_logs (user_id, guild_id, old_nickname, new_nickname)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(user_id as i64)
        .bind(guild_id as i64)
        .bind(old_nickname)
        .bind(new_nickname)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_channel_change(
        &self,
        channel_id: u64,
        guild_id: u64,
        action: &str,
        field_name: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        actor_id: Option<u64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO channel_logs (channel_id, guild_id, action, field_name, old_value, new_value, actor_id)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(channel_id as i64)
        .bind(guild_id as i64)
        .bind(action)
        .bind(field_name)
        .bind(old_value)
        .bind(new_value)
        .bind(actor_id.map(|id| id as i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_dm_message(
        &self,
        message_id: u64,
        user_id: u64,
        content: &str,
        command: Option<&str>,
        timestamp: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO dm_logs (message_id, user_id, content, command, timestamp)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(message_id as i64)
        .bind(user_id as i64)
        .bind(content)
        .bind(command)
        .bind(timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_bot_response(
        &self,
        user_id: u64,
        command: Option<&str>,
        response_type: &str,
        response_content: &str,
        success: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO bot_response_logs (user_id, command, response_type, response_content, success)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(user_id as i64)
        .bind(command)
        .bind(response_type)
        .bind(response_content)
        .bind(success)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn increment_snort_counter(&self, user_id: u64, guild_id: u64) -> Result<i64> {
        // Update the counter and return the new count
        sqlx::query("UPDATE snort_counter SET count = count + 1, last_snort_time = NOW(), last_snort_user_id = ?, last_snort_guild_id = ? WHERE id = 1")
            .bind(user_id as i64)
            .bind(guild_id as i64)
            .execute(&self.pool)
            .await?;

        // Update user's last snort time
        sqlx::query(
            "INSERT INTO user_snort_cooldowns (user_id, last_snort_time) VALUES (?, NOW()) 
             ON DUPLICATE KEY UPDATE last_snort_time = NOW()",
        )
        .bind(user_id as i64)
        .execute(&self.pool)
        .await?;

        // Get the new count
        let count = sqlx::query_scalar::<_, i64>("SELECT count FROM snort_counter WHERE id = 1")
            .fetch_one(&self.pool)
            .await?;

        Ok(count)
    }

    pub async fn get_user_last_snort_time(&self, user_id: u64) -> Result<Option<DateTime<Utc>>> {
        let result = sqlx::query_scalar::<_, DateTime<Utc>>(
            "SELECT last_snort_time FROM user_snort_cooldowns WHERE user_id = ?",
        )
        .bind(user_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result)
    }

    pub async fn get_snort_cooldown_seconds(&self) -> Result<u64> {
        let result = self
            .get_setting("snort_cooldown_seconds")
            .await?
            .unwrap_or_else(|| "30".to_string())
            .parse::<u64>()
            .unwrap_or(30);

        Ok(result)
    }

    pub async fn is_channel_scanned(&self, channel_id: u64) -> Result<bool> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM channel_scan_history WHERE channel_id = ?",
        )
        .bind(channel_id as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(result > 0)
    }

    pub async fn mark_channel_scanned(
        &self,
        channel_id: u64,
        guild_id: u64,
        oldest_message_id: Option<u64>,
        messages_scanned: u32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO channel_scan_history (channel_id, guild_id, scan_completed_at, oldest_message_id, messages_scanned)
            VALUES (?, ?, NOW(), ?, ?)
            "#,
        )
        .bind(channel_id as i64)
        .bind(guild_id as i64)
        .bind(oldest_message_id.map(|id| id as i64))
        .bind(messages_scanned as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_unscanned_channels(&self) -> Result<Vec<(u64, u64)>> {
        // This method will be used by the background job to find channels that haven't been scanned
        // Using runtime query to avoid compile-time verification issues
        let results: Vec<(i64, i64)> = sqlx::query_as(
            r#"
            SELECT DISTINCT mc.channel_id, mc.guild_id
            FROM (
                SELECT DISTINCT channel_id, 
                       (SELECT guild_id FROM channel_logs WHERE channel_id = ml.channel_id LIMIT 1) as guild_id
                FROM message_logs ml
                UNION
                SELECT DISTINCT channel_id, guild_id
                FROM channel_logs
            ) mc
            LEFT JOIN channel_scan_history csh ON mc.channel_id = csh.channel_id
            WHERE csh.channel_id IS NULL AND mc.guild_id IS NOT NULL
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(results
            .into_iter()
            .map(|(channel_id, guild_id)| (channel_id as u64, guild_id as u64))
            .collect())
    }

    // Poll tracking methods
    pub async fn log_poll_created(
        &self,
        poll_id: &str,
        message_id: u64,
        channel_id: u64,
        guild_id: u64,
        creator_id: u64,
        question: &str,
        expires_at: Option<DateTime<Utc>>,
        is_multiselect: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO poll_logs (poll_id, message_id, channel_id, guild_id, creator_id, question, created_at, expires_at, is_multiselect)
            VALUES (?, ?, ?, ?, ?, ?, NOW(), ?, ?)
            "#,
        )
        .bind(poll_id)
        .bind(message_id as i64)
        .bind(channel_id as i64)
        .bind(guild_id as i64)
        .bind(creator_id as i64)
        .bind(question)
        .bind(expires_at)
        .bind(is_multiselect)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_poll_answer(
        &self,
        poll_id: &str,
        answer_id: u32,
        answer_text: &str,
        emoji: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO poll_answers (poll_id, answer_id, answer_text, emoji)
            VALUES (?, ?, ?, ?)
            "#,
        )
        .bind(poll_id)
        .bind(answer_id as i32)
        .bind(answer_text)
        .bind(emoji)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_poll_vote(&self, poll_id: &str, user_id: u64, answer_id: u32) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO poll_votes (poll_id, user_id, answer_id)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE voted_at = NOW()
            "#,
        )
        .bind(poll_id)
        .bind(user_id as i64)
        .bind(answer_id as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_poll_vote(
        &self,
        poll_id: &str,
        user_id: u64,
        answer_id: u32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM poll_votes 
            WHERE poll_id = ? AND user_id = ? AND answer_id = ?
            "#,
        )
        .bind(poll_id)
        .bind(user_id as i64)
        .bind(answer_id as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn close_poll(&self, poll_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE poll_logs 
            SET closed_at = NOW() 
            WHERE poll_id = ? AND closed_at IS NULL
            "#,
        )
        .bind(poll_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_poll_votes(&self, poll_id: &str, user_id: u64) -> Result<Vec<u32>> {
        let votes: Vec<(u32,)> = sqlx::query_as(
            r#"
            SELECT answer_id 
            FROM poll_votes 
            WHERE poll_id = ? AND user_id = ?
            "#,
        )
        .bind(poll_id)
        .bind(user_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(votes.into_iter().map(|v| v.0).collect())
    }

    // Event tracking methods
    pub async fn log_event_created(
        &self,
        event_id: u64,
        guild_id: u64,
        channel_id: Option<u64>,
        creator_id: u64,
        name: &str,
        description: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: Option<DateTime<Utc>>,
        location: Option<&str>,
        status: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO event_logs (event_id, guild_id, channel_id, creator_id, name, description, start_time, end_time, location, status)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                name = VALUES(name),
                description = VALUES(description),
                start_time = VALUES(start_time),
                end_time = VALUES(end_time),
                location = VALUES(location),
                status = VALUES(status)
            "#,
        )
        .bind(event_id as i64)
        .bind(guild_id as i64)
        .bind(channel_id.map(|id| id as i64))
        .bind(creator_id as i64)
        .bind(name)
        .bind(description)
        .bind(start_time)
        .bind(end_time)
        .bind(location)
        .bind(status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_event_interest(
        &self,
        event_id: u64,
        user_id: u64,
        interest_type: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO event_interests (event_id, user_id, interest_type)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                interest_type = VALUES(interest_type),
                expressed_at = NOW()
            "#,
        )
        .bind(event_id as i64)
        .bind(user_id as i64)
        .bind(interest_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_event_interest(&self, event_id: u64, user_id: u64) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM event_interests 
            WHERE event_id = ? AND user_id = ?
            "#,
        )
        .bind(event_id as i64)
        .bind(user_id as i64)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn log_event_update(
        &self,
        event_id: u64,
        field_name: &str,
        old_value: Option<&str>,
        new_value: Option<&str>,
        updated_by: Option<u64>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO event_update_logs (event_id, field_name, old_value, new_value, updated_by)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(event_id as i64)
        .bind(field_name)
        .bind(old_value)
        .bind(new_value)
        .bind(updated_by.map(|id| id as i64))
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn cleanup_old_status_logs(&self, days: i64) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);

        let result = sqlx::query("DELETE FROM member_status_logs WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn log_media_recommendation(
        &self,
        message_id: u64,
        user_id: u64,
        channel_id: u64,
        guild_id: u64,
        media_type: &str,
        title: &str,
        url: Option<&str>,
        confidence: f32,
        message_timestamp: DateTime<Utc>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT IGNORE INTO media_recommendations 
            (message_id, user_id, channel_id, guild_id, media_type, title, url, confidence_score, message_timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(message_id as i64)
        .bind(user_id as i64)
        .bind(channel_id as i64)
        .bind(guild_id as i64)
        .bind(media_type)
        .bind(title)
        .bind(url)
        .bind(confidence)
        .bind(message_timestamp)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_media_scan_checkpoint(&self) -> Result<(u64, DateTime<Utc>)> {
        let row: (i64, DateTime<Utc>) = sqlx::query_as(
            "SELECT last_scanned_message_id, last_scan_time FROM media_scan_checkpoint WHERE id = 1"
        )
        .fetch_one(&self.pool)
        .await?;

        Ok((row.0 as u64, row.1))
    }

    pub async fn update_media_scan_checkpoint(
        &self,
        last_message_id: u64,
        messages_scanned: u32,
        recommendations_found: u32,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE media_scan_checkpoint 
            SET last_scanned_message_id = ?, 
                last_scan_time = NOW(),
                messages_scanned = messages_scanned + ?,
                recommendations_found = recommendations_found + ?
            WHERE id = 1
            "#,
        )
        .bind(last_message_id as i64)
        .bind(messages_scanned as i32)
        .bind(recommendations_found as i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_unscanned_messages(
        &self,
        last_id: u64,
        limit: u32,
    ) -> Result<Vec<(u64, u64, u64, u64, String, DateTime<Utc>)>> {
        let messages: Vec<(i64, i64, i64, i64, String, DateTime<Utc>)> = sqlx::query_as(
            r#"
            SELECT ml.message_id, ml.user_id, ml.channel_id, 
                   COALESCE(cl.guild_id, 0) as guild_id,
                   ml.content, ml.timestamp
            FROM message_logs ml
            LEFT JOIN channel_logs cl ON ml.channel_id = cl.channel_id 
                AND cl.action = 'create'
            WHERE ml.message_id > ? 
                AND ml.content IS NOT NULL 
                AND ml.content != ''
            ORDER BY ml.message_id ASC
            LIMIT ?
            "#,
        )
        .bind(last_id as i64)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(messages
            .into_iter()
            .map(
                |(msg_id, user_id, channel_id, guild_id, content, timestamp)| {
                    (
                        msg_id as u64,
                        user_id as u64,
                        channel_id as u64,
                        guild_id as u64,
                        content,
                        timestamp,
                    )
                },
            )
            .collect())
    }

    // Watchlist methods
    pub async fn add_to_watchlist(
        &self,
        user_id: u64,
        media_type: &str,
        title: &str,
        url: Option<&str>,
        priority: Option<i32>,
        notes: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO user_watchlist (user_id, media_type, title, url, priority, notes)
            VALUES (?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                url = COALESCE(VALUES(url), url),
                priority = COALESCE(VALUES(priority), priority),
                notes = COALESCE(VALUES(notes), notes),
                updated_at = NOW()
            "#,
        )
        .bind(user_id as i64)
        .bind(media_type)
        .bind(title)
        .bind(url)
        .bind(priority.unwrap_or(50))
        .bind(notes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_from_watchlist(
        &self,
        user_id: u64,
        media_type: &str,
        title: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM user_watchlist WHERE user_id = ? AND media_type = ? AND title = ?",
        )
        .bind(user_id as i64)
        .bind(media_type)
        .bind(title)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn update_watchlist_priority(
        &self,
        user_id: u64,
        media_type: &str,
        title: &str,
        priority: i32,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE user_watchlist 
            SET priority = ?, updated_at = NOW()
            WHERE user_id = ? AND media_type = ? AND title = ?
            "#,
        )
        .bind(priority)
        .bind(user_id as i64)
        .bind(media_type)
        .bind(title)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_user_watchlist(
        &self,
        user_id: u64,
        limit: u32,
    ) -> Result<Vec<(String, String, Option<String>, i32, String)>> {
        let items: Vec<(String, String, Option<String>, i32, String)> = sqlx::query_as(
            r#"
            SELECT media_type, title, url, priority, status
            FROM user_watchlist
            WHERE user_id = ? AND status IN ('plan_to_watch', 'watching')
            ORDER BY priority DESC, updated_at DESC
            LIMIT ?
            "#,
        )
        .bind(user_id as i64)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    pub async fn get_top_recommendations(
        &self,
        limit: u32,
        days: i32,
    ) -> Result<Vec<(String, String, f32, i64, Option<String>)>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);

        let items: Vec<(String, String, f32, i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT 
                media_type,
                title,
                AVG(confidence_score) as avg_confidence,
                COUNT(*) as mention_count,
                MAX(url) as sample_url
            FROM media_recommendations
            WHERE message_timestamp > ?
            GROUP BY media_type, title
            HAVING COUNT(*) >= 2
            ORDER BY COUNT(*) DESC, AVG(confidence_score) DESC
            LIMIT ?
            "#,
        )
        .bind(cutoff)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    pub async fn search_recommendations(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<(String, String, f32, i64)>> {
        let search_pattern = format!("%{}%", query);

        let items: Vec<(String, String, f32, i64)> = sqlx::query_as(
            r#"
            SELECT 
                media_type,
                title,
                AVG(confidence_score) as avg_confidence,
                COUNT(*) as mention_count
            FROM media_recommendations
            WHERE title LIKE ?
            GROUP BY media_type, title
            ORDER BY COUNT(*) DESC, AVG(confidence_score) DESC
            LIMIT ?
            "#,
        )
        .bind(search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    pub async fn get_user_watchlist_full(
        &self,
        user_id: u64,
    ) -> Result<Vec<(String, String, Option<String>, i32, String, Option<String>)>> {
        let items: Vec<(String, String, Option<String>, i32, String, Option<String>)> = sqlx::query_as(
            r#"
            SELECT media_type, title, url, priority, status, notes
            FROM user_watchlist
            WHERE user_id = ?
            ORDER BY priority DESC, updated_at DESC
            "#,
        )
        .bind(user_id as i64)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    pub async fn get_user_recommendations(
        &self,
        days: i32,
    ) -> Result<Vec<(String, String, Option<String>, f32, i64, Vec<String>)>> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days as i64);

        let items: Vec<(String, String, f32, i64, Option<String>)> = sqlx::query_as(
            r#"
            SELECT 
                mr.media_type,
                mr.title,
                AVG(mr.confidence_score) as avg_confidence,
                COUNT(*) as mention_count,
                MAX(mr.url) as sample_url
            FROM media_recommendations mr
            WHERE mr.message_timestamp > ?
            GROUP BY mr.media_type, mr.title
            ORDER BY COUNT(*) DESC, AVG(mr.confidence_score) DESC
            "#,
        )
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        // Get usernames for each recommendation
        let mut results = Vec::new();
        for (media_type, title, confidence, count, url) in items {
            let users: Vec<(String,)> = sqlx::query_as(
                r#"
                SELECT DISTINCT u.username
                FROM media_recommendations mr
                JOIN users u ON mr.user_id = u.discord_user_id
                WHERE mr.media_type = ? AND mr.title = ? AND mr.message_timestamp > ?
                LIMIT 10
                "#,
            )
            .bind(&media_type)
            .bind(&title)
            .bind(cutoff)
            .fetch_all(&self.pool)
            .await?;

            let usernames: Vec<String> = users.into_iter().map(|u| u.0).collect();
            results.push((media_type, title, url, confidence, count, usernames));
        }

        Ok(results)
    }

    // Global watchlist methods
    pub async fn add_to_global_watchlist(
        &self,
        media_type: &str,
        title: &str,
        url: Option<&str>,
        description: Option<&str>,
        added_by: u64,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"
            INSERT INTO global_watchlist (media_type, title, url, description, added_by)
            VALUES (?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                url = COALESCE(VALUES(url), url),
                description = COALESCE(VALUES(description), description),
                updated_at = NOW()
            "#,
        )
        .bind(media_type)
        .bind(title)
        .bind(url)
        .bind(description)
        .bind(added_by as i64)
        .execute(&self.pool)
        .await?;

        // Get the ID of the inserted/updated item
        if result.rows_affected() > 0 {
            let id: (u64,) = sqlx::query_as(
                "SELECT id FROM global_watchlist WHERE media_type = ? AND title = ?"
            )
            .bind(media_type)
            .bind(title)
            .fetch_one(&self.pool)
            .await?;
            Ok(id.0)
        } else {
            Err(anyhow::anyhow!("Failed to add to global watchlist"))
        }
    }

    pub async fn vote_global_watchlist(
        &self,
        watchlist_id: u64,
        user_id: u64,
        vote_type: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO global_watchlist_votes (watchlist_id, user_id, vote_type)
            VALUES (?, ?, ?)
            ON DUPLICATE KEY UPDATE 
                vote_type = VALUES(vote_type),
                voted_at = NOW()
            "#,
        )
        .bind(watchlist_id as i64)
        .bind(user_id as i64)
        .bind(vote_type)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn remove_vote_global_watchlist(
        &self,
        watchlist_id: u64,
        user_id: u64,
    ) -> Result<bool> {
        let result = sqlx::query(
            "DELETE FROM global_watchlist_votes WHERE watchlist_id = ? AND user_id = ?"
        )
        .bind(watchlist_id as i64)
        .bind(user_id as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_global_watchlist(
        &self,
        limit: u32,
        media_type: Option<&str>,
    ) -> Result<Vec<(u64, String, String, Option<String>, Option<String>, i64, i64, String)>> {
        let query = if let Some(media_type) = media_type {
            sqlx::query_as(
                r#"
                SELECT 
                    gw.id,
                    gw.media_type,
                    gw.title,
                    gw.url,
                    gw.description,
                    COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) as upvotes,
                    COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) as downvotes,
                    u.username as added_by_username
                FROM global_watchlist gw
                LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
                JOIN users u ON gw.added_by = u.discord_user_id
                WHERE gw.media_type = ?
                GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
                ORDER BY (upvotes - downvotes) DESC, gw.added_at DESC
                LIMIT ?
                "#,
            )
            .bind(media_type)
            .bind(limit)
        } else {
            sqlx::query_as(
                r#"
                SELECT 
                    gw.id,
                    gw.media_type,
                    gw.title,
                    gw.url,
                    gw.description,
                    COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) as upvotes,
                    COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) as downvotes,
                    u.username as added_by_username
                FROM global_watchlist gw
                LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
                JOIN users u ON gw.added_by = u.discord_user_id
                GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
                ORDER BY (upvotes - downvotes) DESC, gw.added_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
        };

        let items: Vec<(u64, String, String, Option<String>, Option<String>, i64, i64, String)> = 
            query.fetch_all(&self.pool).await?;

        Ok(items)
    }

    pub async fn get_user_vote_on_global_item(
        &self,
        watchlist_id: u64,
        user_id: u64,
    ) -> Result<Option<String>> {
        let vote: Option<(String,)> = sqlx::query_as(
            "SELECT vote_type FROM global_watchlist_votes WHERE watchlist_id = ? AND user_id = ?"
        )
        .bind(watchlist_id as i64)
        .bind(user_id as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(vote.map(|v| v.0))
    }

    pub async fn search_global_watchlist(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<(u64, String, String, Option<String>, Option<String>, i64, i64, String)>> {
        let search_pattern = format!("%{}%", query);

        let items: Vec<(u64, String, String, Option<String>, Option<String>, i64, i64, String)> = sqlx::query_as(
            r#"
            SELECT 
                gw.id,
                gw.media_type,
                gw.title,
                gw.url,
                gw.description,
                COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) as upvotes,
                COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) as downvotes,
                u.username as added_by_username
            FROM global_watchlist gw
            LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
            JOIN users u ON gw.added_by = u.discord_user_id
            WHERE gw.title LIKE ? OR gw.description LIKE ?
            GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
            ORDER BY (upvotes - downvotes) DESC, gw.added_at DESC
            LIMIT ?
            "#,
        )
        .bind(&search_pattern)
        .bind(&search_pattern)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        Ok(items)
    }

    pub async fn cleanup_old_logs(&self, days: i64) -> Result<(u64, u64, u64, u64, u64)> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days);

        // Clean up old nickname logs
        let nickname_result = sqlx::query("DELETE FROM nickname_logs WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        // Clean up old voice logs
        let voice_result = sqlx::query("DELETE FROM voice_logs WHERE timestamp < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        // Clean up old poll votes (for closed polls)
        let poll_votes_result = sqlx::query(
            r#"
            DELETE pv FROM poll_votes pv
            INNER JOIN poll_logs pl ON pv.poll_id = pl.poll_id
            WHERE pl.closed_at IS NOT NULL AND pl.closed_at < ?
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        // Clean up old event interests (for past events)
        let event_interests_result = sqlx::query(
            r#"
            DELETE ei FROM event_interests ei
            INNER JOIN event_logs el ON ei.event_id = el.event_id
            WHERE el.end_time IS NOT NULL AND el.end_time < ?
            "#,
        )
        .bind(cutoff)
        .execute(&self.pool)
        .await?;

        // Clean up old event update logs
        let event_updates_result =
            sqlx::query("DELETE FROM event_update_logs WHERE updated_at < ?")
                .bind(cutoff)
                .execute(&self.pool)
                .await?;

        Ok((
            nickname_result.rows_affected(),
            voice_result.rows_affected(),
            poll_votes_result.rows_affected(),
            event_interests_result.rows_affected(),
            event_updates_result.rows_affected(),
        ))
    }
}
