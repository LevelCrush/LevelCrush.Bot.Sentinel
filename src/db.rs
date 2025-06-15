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
        // Run sqlx migrations from the migrations directory
        sqlx::migrate!("./migrations").run(&self.pool).await?;

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
        let settings: Vec<(String, String)> =
            sqlx::query_as("SELECT setting_key, setting_value FROM system_settings")
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
        let items: Vec<(String, String, Option<String>, i32, String, Option<String>)> =
            sqlx::query_as(
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
            let id: (i32,) = sqlx::query_as(
                "SELECT id FROM global_watchlist WHERE media_type = ? AND title = ?",
            )
            .bind(media_type)
            .bind(title)
            .fetch_one(&self.pool)
            .await?;
            Ok(id.0 as u64)
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
        .bind(watchlist_id as i32)
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
            "DELETE FROM global_watchlist_votes WHERE watchlist_id = ? AND user_id = ?",
        )
        .bind(watchlist_id as i32)
        .bind(user_id as i64)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_global_watchlist(
        &self,
        limit: u32,
        media_type: Option<&str>,
    ) -> Result<
        Vec<(
            i32,
            String,
            String,
            Option<String>,
            Option<String>,
            i64,
            i64,
            String,
        )>,
    > {
        let query = if let Some(media_type) = media_type {
            sqlx::query_as(
                r#"
                SELECT 
                    gw.id,
                    gw.media_type,
                    gw.title,
                    gw.url,
                    gw.description,
                    CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) as upvotes,
                    CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED) as downvotes,
                    u.username as added_by_username
                FROM global_watchlist gw
                LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
                JOIN users u ON gw.added_by = u.discord_user_id
                WHERE gw.media_type = ?
                GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
                ORDER BY (CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) - 
                     CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED)) DESC, 
                     gw.added_at DESC
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
                    CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) as upvotes,
                    CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED) as downvotes,
                    u.username as added_by_username
                FROM global_watchlist gw
                LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
                JOIN users u ON gw.added_by = u.discord_user_id
                GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
                ORDER BY (CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) - 
                     CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED)) DESC, 
                     gw.added_at DESC
                LIMIT ?
                "#,
            )
            .bind(limit)
        };

        let items: Vec<(
            i32,
            String,
            String,
            Option<String>,
            Option<String>,
            i64,
            i64,
            String,
        )> = query.fetch_all(&self.pool).await?;

        Ok(items)
    }

    pub async fn get_user_vote_on_global_item(
        &self,
        watchlist_id: u64,
        user_id: u64,
    ) -> Result<Option<String>> {
        let vote: Option<(String,)> = sqlx::query_as(
            "SELECT vote_type FROM global_watchlist_votes WHERE watchlist_id = ? AND user_id = ?",
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
    ) -> Result<
        Vec<(
            i32,
            String,
            String,
            Option<String>,
            Option<String>,
            i64,
            i64,
            String,
        )>,
    > {
        let search_pattern = format!("%{}%", query);

        let items: Vec<(i32, String, String, Option<String>, Option<String>, i64, i64, String)> = sqlx::query_as(
            r#"
            SELECT 
                gw.id,
                gw.media_type,
                gw.title,
                gw.url,
                gw.description,
                CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) as upvotes,
                CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED) as downvotes,
                u.username as added_by_username
            FROM global_watchlist gw
            LEFT JOIN global_watchlist_votes gwv ON gw.id = gwv.watchlist_id
            JOIN users u ON gw.added_by = u.discord_user_id
            WHERE gw.title LIKE ? OR gw.description LIKE ?
            GROUP BY gw.id, gw.media_type, gw.title, gw.url, gw.description, u.username
            ORDER BY (CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'up' THEN 1 ELSE 0 END), 0) AS SIGNED) - 
                     CAST(COALESCE(SUM(CASE WHEN gwv.vote_type = 'down' THEN 1 ELSE 0 END), 0) AS SIGNED)) DESC, 
                     gw.added_at DESC
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

    // GIPHY related functions
    pub async fn get_active_giphy_search_terms(&self) -> Result<Vec<String>> {
        let terms: Vec<(String,)> = sqlx::query_as(
            "SELECT search_term FROM giphy_search_terms WHERE is_active = TRUE ORDER BY priority DESC, id ASC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(terms.into_iter().map(|(term,)| term).collect())
    }

    pub async fn get_cached_giphy_gif(
        &self,
        search_term: &str,
        exclude_id: Option<&str>,
    ) -> Result<Option<crate::giphy::GiphyGif>> {
        // Get a random cached gif for the search term, excluding the last used one if provided
        let result: Option<(String, String, String, String, i32, i32)> =
            if let Some(exclude) = exclude_id {
                sqlx::query_as(
                    r#"
                SELECT gif_id, gif_url, gif_title, gif_rating, width, height
                FROM giphy_cache
                WHERE search_term = ? AND gif_id != ?
                ORDER BY RAND()
                LIMIT 1
                "#,
                )
                .bind(search_term)
                .bind(exclude)
                .fetch_optional(&self.pool)
                .await?
            } else {
                sqlx::query_as(
                    r#"
                SELECT gif_id, gif_url, gif_title, gif_rating, width, height
                FROM giphy_cache
                WHERE search_term = ?
                ORDER BY RAND()
                LIMIT 1
                "#,
                )
                .bind(search_term)
                .fetch_optional(&self.pool)
                .await?
            };

        if let Some((id, url, title, rating, width, height)) = result {
            // Update last used time and increment use count
            sqlx::query(
                "UPDATE giphy_cache SET last_used = NOW(), use_count = use_count + 1 WHERE gif_id = ? AND search_term = ?"
            )
            .bind(&id)
            .bind(search_term)
            .execute(&self.pool)
            .await?;

            // Construct a GiphyGif object
            let gif = crate::giphy::GiphyGif {
                id,
                title,
                rating,
                images: crate::giphy::GiphyImages {
                    original: crate::giphy::GiphyImage {
                        url,
                        width: width.to_string(),
                        height: height.to_string(),
                        size: None,
                    },
                    fixed_height: crate::giphy::GiphyImage {
                        url: String::new(),
                        width: String::new(),
                        height: String::new(),
                        size: None,
                    },
                    fixed_width: crate::giphy::GiphyImage {
                        url: String::new(),
                        width: String::new(),
                        height: String::new(),
                        size: None,
                    },
                },
            };

            Ok(Some(gif))
        } else {
            Ok(None)
        }
    }

    pub async fn cache_giphy_gif(
        &self,
        search_term: &str,
        gif: &crate::giphy::GiphyGif,
    ) -> Result<()> {
        let width: i32 = gif.images.original.width.parse().unwrap_or(0);
        let height: i32 = gif.images.original.height.parse().unwrap_or(0);
        let file_size: Option<i64> = gif
            .images
            .original
            .size
            .as_ref()
            .and_then(|s| s.parse().ok());

        sqlx::query(
            r#"
            INSERT INTO giphy_cache (search_term, gif_id, gif_url, gif_title, gif_rating, width, height, file_size_bytes)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE
                gif_url = VALUES(gif_url),
                gif_title = VALUES(gif_title),
                gif_rating = VALUES(gif_rating),
                width = VALUES(width),
                height = VALUES(height),
                file_size_bytes = VALUES(file_size_bytes),
                cached_at = NOW()
            "#
        )
        .bind(search_term)
        .bind(&gif.id)
        .bind(&gif.images.original.url)
        .bind(&gif.title)
        .bind(&gif.rating)
        .bind(width)
        .bind(height)
        .bind(file_size)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_cache_size(&self, search_term: &str) -> Result<u32> {
        let count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM giphy_cache WHERE search_term = ?")
                .bind(search_term)
                .fetch_one(&self.pool)
                .await?;

        Ok(count as u32)
    }

    pub async fn clean_old_giphy_cache(&self, days_old: i32) -> Result<u64> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(days_old as i64);

        let result = sqlx::query("DELETE FROM giphy_cache WHERE last_used < ?")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;

        Ok(result.rows_affected())
    }

    pub async fn get_last_snort_meme(&self) -> Result<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT setting_value FROM system_settings WHERE setting_key = 'last_snort_meme'",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(value,)| value))
    }

    pub async fn set_last_snort_meme(&self, meme_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO system_settings (setting_key, setting_value)
            VALUES ('last_snort_meme', ?)
            ON DUPLICATE KEY UPDATE setting_value = VALUES(setting_value)
            "#,
        )
        .bind(meme_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
