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
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM command_whitelist WHERE discord_user_id = ?",
        )
        .bind(user_id as i64)
        .fetch_one(&self.pool)
        .await?;

        Ok(result > 0)
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
}
