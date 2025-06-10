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
}
