use crate::db::Database;
use crate::media::MediaCache;
use crate::media_detector::MediaDetector;
use anyhow::Result;
use serenity::all::Context;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

pub async fn start_background_jobs(
    ctx: Arc<Context>,
    db: Database,
    media_cache: MediaCache,
) -> Result<()> {
    let scheduler = JobScheduler::new().await?;

    let ctx_clone = ctx.clone();
    let db_clone = db.clone();

    // Create a simpler job that spawns the actual task
    let user_sync_job = Job::new_async("0 0 */12 * * *", move |_uuid, _l| {
        let ctx = ctx_clone.clone();
        let db = db_clone.clone();
        Box::pin(async move {
            // Spawn the actual sync task
            tokio::spawn(async move {
                if let Err(e) = sync_all_users(ctx, db).await {
                    tracing::error!("Failed to sync users: {}", e);
                }
            });
        })
    })?;

    scheduler.add(user_sync_job).await?;

    // Media cleanup job - runs daily at 3 AM
    let db_cleanup = db.clone();
    let media_cache_cleanup = media_cache.clone();

    let media_cleanup_job = Job::new_async("0 0 3 * * *", move |_uuid, _l| {
        let db = db_cleanup.clone();
        let media_cache = media_cache_cleanup.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                if let Err(e) = cleanup_old_media(db, media_cache).await {
                    tracing::error!("Failed to cleanup old media: {}", e);
                }
            });
        })
    })?;

    scheduler.add(media_cleanup_job).await?;

    // Historical message scanning job - runs every hour
    let db_scan = db.clone();
    let ctx_scan = ctx.clone();

    let history_scan_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let db = db_scan.clone();
        let ctx = ctx_scan.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                if let Err(e) = scan_channel_history(ctx, db).await {
                    tracing::error!("Failed to scan channel history: {}", e);
                }
            });
        })
    })?;

    scheduler.add(history_scan_job).await?;

    // Poll expiry check job - runs every hour
    let db_poll_check = db.clone();

    let poll_expiry_job = Job::new_async("0 0 * * * *", move |_uuid, _l| {
        let db = db_poll_check.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                if let Err(e) = check_expired_polls(db).await {
                    tracing::error!("Failed to check expired polls: {}", e);
                }
            });
        })
    })?;

    scheduler.add(poll_expiry_job).await?;

    // Status cleanup job - runs daily at 4 AM
    let db_status_cleanup = db.clone();

    let status_cleanup_job = Job::new_async("0 0 4 * * *", move |_uuid, _l| {
        let db = db_status_cleanup.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                if let Err(e) = cleanup_old_status_logs(db).await {
                    tracing::error!("Failed to cleanup old status logs: {}", e);
                }
            });
        })
    })?;

    scheduler.add(status_cleanup_job).await?;

    // Media recommendations scanning job - runs every 30 minutes
    let db_media_scan = db.clone();

    let media_scan_job = Job::new_async("0 */30 * * * *", move |_uuid, _l| {
        let db = db_media_scan.clone();
        Box::pin(async move {
            tokio::spawn(async move {
                if let Err(e) = scan_for_media_recommendations(db).await {
                    tracing::error!("Failed to scan for media recommendations: {}", e);
                }
            });
        })
    })?;

    scheduler.add(media_scan_job).await?;
    scheduler.start().await?;

    info!("Background jobs started");

    // Keep the scheduler alive
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        }
    });

    Ok(())
}

async fn sync_all_users(ctx: Arc<Context>, db: Database) -> Result<()> {
    info!("Starting user sync job");

    let guilds = ctx.cache.guilds();

    for guild_id in guilds {
        // Collect data from cache
        let members_data: Vec<_> = {
            if let Some(guild) = ctx.cache.guild(guild_id) {
                guild
                    .members
                    .values()
                    .map(|member| {
                        let user = &member.user;
                        let nickname = member.nick.clone();
                        let global_handle = if user.discriminator.is_some() {
                            None
                        } else {
                            Some(user.name.clone())
                        };

                        let discriminator = user.discriminator.map(|d| d.get().to_string());

                        (
                            user.id.get(),
                            user.name.clone(),
                            discriminator,
                            global_handle,
                            nickname,
                        )
                    })
                    .collect()
            } else {
                vec![]
            }
        };

        // Update database outside of cache lock
        let guild_name = ctx
            .cache
            .guild(guild_id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        info!(
            "Syncing {} members from guild {} ({})",
            members_data.len(),
            guild_name,
            guild_id
        );

        for (user_id, username, discriminator, global_handle, nickname) in members_data {
            if let Err(e) = db
                .update_user(
                    user_id,
                    &username,
                    discriminator.as_deref(),
                    global_handle.as_deref(),
                    nickname.as_deref(),
                )
                .await
            {
                tracing::error!("Failed to update user {}: {}", user_id, e);
            }
        }
    }

    info!("User sync job completed");
    Ok(())
}

async fn cleanup_old_media(db: Database, media_cache: MediaCache) -> Result<()> {
    info!("Starting media cleanup job");

    // Check if media caching is enabled
    if let Ok(Some(cache_enabled)) = db.get_setting("cache_media").await {
        if cache_enabled != "true" {
            info!("Media caching is disabled, skipping cleanup");
            return Ok(());
        }
    }

    // Delete files older than 31 days
    match media_cache.cleanup_old_files(31).await {
        Ok(count) => info!("Deleted {} old cached files", count),
        Err(e) => tracing::error!("Failed to cleanup cached files: {}", e),
    }

    // Get list of old attachments from database
    match db.get_old_cached_media(31).await {
        Ok(old_paths) => {
            info!("Found {} old media entries in database", old_paths.len());
            // Note: We don't clear the database entries here since the files are already deleted
            // The local_path column serves as a record that the file was once cached
        }
        Err(e) => tracing::error!("Failed to query old cached media: {}", e),
    }

    info!("Media cleanup job completed");
    Ok(())
}

async fn scan_channel_history(ctx: Arc<Context>, db: Database) -> Result<()> {
    info!("Starting channel history scan job");

    // Get all accessible channels from cache
    let mut channels_to_scan = Vec::new();

    for guild_id in ctx.cache.guilds() {
        if let Some(guild) = ctx.cache.guild(guild_id) {
            for (channel_id, channel) in &guild.channels {
                // Only scan text channels
                if channel.is_text_based() {
                    channels_to_scan.push((*channel_id, guild_id));
                }
            }
        }
    }

    info!(
        "Found {} text channels to potentially scan",
        channels_to_scan.len()
    );

    // Scan up to 5 channels per run to avoid overwhelming the system
    let mut scanned_count = 0;
    const MAX_CHANNELS_PER_RUN: usize = 5;

    for (channel_id, guild_id) in channels_to_scan {
        if scanned_count >= MAX_CHANNELS_PER_RUN {
            info!(
                "Reached maximum channels per run ({}), stopping",
                MAX_CHANNELS_PER_RUN
            );
            break;
        }

        // Check if channel has already been scanned
        match db.is_channel_scanned(channel_id.get()).await {
            Ok(true) => {
                // Already scanned, skip
                continue;
            }
            Ok(false) => {
                // Not scanned yet, proceed
            }
            Err(e) => {
                tracing::error!(
                    "Failed to check scan status for channel {}: {}",
                    channel_id,
                    e
                );
                continue;
            }
        }

        info!(
            "Scanning historical messages for channel {} in guild {}",
            channel_id, guild_id
        );

        // Scan the channel
        match scan_single_channel(&ctx, &db, channel_id, guild_id).await {
            Ok(messages_scanned) => {
                info!(
                    "Successfully scanned {} messages from channel {}",
                    messages_scanned, channel_id
                );
                scanned_count += 1;
            }
            Err(e) => {
                tracing::error!("Failed to scan channel {}: {}", channel_id, e);
            }
        }

        // Add a small delay between channels to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    }

    info!(
        "Channel history scan job completed. Scanned {} channels",
        scanned_count
    );
    Ok(())
}

async fn scan_single_channel(
    ctx: &Context,
    db: &Database,
    channel_id: serenity::all::ChannelId,
    guild_id: serenity::all::GuildId,
) -> Result<u32> {
    use serenity::all::GetMessages;

    let mut total_messages = 0u32;
    let mut oldest_message_id: Option<u64> = None;
    let mut last_message_id: Option<serenity::all::MessageId> = None;

    // Scan in batches of 100 messages (Discord API limit)
    const BATCH_SIZE: u8 = 100;
    const MAX_MESSAGES: u32 = 10000; // Limit to avoid excessive scanning

    loop {
        if total_messages >= MAX_MESSAGES {
            info!(
                "Reached maximum message limit ({}) for channel {}",
                MAX_MESSAGES, channel_id
            );
            break;
        }

        // Build the request
        let mut request = GetMessages::new().limit(BATCH_SIZE);
        if let Some(before_id) = last_message_id {
            request = request.before(before_id);
        }

        // Fetch messages
        let messages = match channel_id.messages(&ctx.http, request).await {
            Ok(messages) => messages,
            Err(e) => {
                // If we get an error (e.g., no permission), mark the channel as scanned anyway
                tracing::warn!(
                    "Error fetching messages from channel {}: {}. Marking as scanned.",
                    channel_id,
                    e
                );
                db.mark_channel_scanned(
                    channel_id.get(),
                    guild_id.get(),
                    oldest_message_id,
                    total_messages,
                )
                .await?;
                return Ok(total_messages);
            }
        };

        if messages.is_empty() {
            // No more messages to fetch
            break;
        }

        // Process messages
        for message in &messages {
            // Skip bot messages
            if message.author.bot {
                continue;
            }

            // Log the message (without caching media as requested)
            if let Err(e) = db
                .log_message(
                    message.id.get(),
                    message.author.id.get(),
                    channel_id.get(),
                    &message.content,
                    message.timestamp.to_utc(),
                )
                .await
            {
                tracing::error!("Failed to log historical message {}: {}", message.id, e);
                continue;
            }

            // Update oldest message ID
            if oldest_message_id.is_none() || message.id.get() < oldest_message_id.unwrap() {
                oldest_message_id = Some(message.id.get());
            }

            total_messages += 1;
        }

        // Update last_message_id for next batch
        if let Some(last_msg) = messages.last() {
            last_message_id = Some(last_msg.id);
        } else {
            break;
        }

        // Log progress
        if total_messages % 1000 == 0 {
            info!(
                "Scanned {} messages so far from channel {}",
                total_messages, channel_id
            );
        }

        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    }

    // Mark channel as scanned
    db.mark_channel_scanned(
        channel_id.get(),
        guild_id.get(),
        oldest_message_id,
        total_messages,
    )
    .await?;

    Ok(total_messages)
}

async fn check_expired_polls(db: Database) -> Result<()> {
    info!("Checking for expired polls");

    // Query for polls that have expired but haven't been closed
    let expired_polls: Vec<(String,)> = sqlx::query_as(
        r#"
        SELECT poll_id 
        FROM poll_logs 
        WHERE expires_at IS NOT NULL 
        AND expires_at < NOW() 
        AND closed_at IS NULL
        "#,
    )
    .fetch_all(&db.pool)
    .await?;

    info!("Found {} expired polls to close", expired_polls.len());

    for (poll_id,) in expired_polls {
        match db.close_poll(&poll_id).await {
            Ok(_) => info!("Closed expired poll: {}", poll_id),
            Err(e) => tracing::error!("Failed to close expired poll {}: {}", poll_id, e),
        }
    }

    Ok(())
}

async fn cleanup_old_status_logs(db: Database) -> Result<()> {
    info!("Starting Discord logs cleanup job");

    // Delete status logs older than 31 days
    match db.cleanup_old_status_logs(31).await {
        Ok(deleted_count) => {
            info!("Deleted {} old status log entries", deleted_count);
        }
        Err(e) => {
            tracing::error!("Failed to cleanup old status logs: {}", e);
        }
    }

    // Delete other old logs (nickname, voice, poll votes, event data)
    match db.cleanup_old_logs(31).await {
        Ok((nicknames, voice, poll_votes, event_interests, event_updates)) => {
            info!(
                "Cleanup complete - Deleted: {} nickname logs, {} voice logs, {} poll votes, {} event interests, {} event updates",
                nicknames, voice, poll_votes, event_interests, event_updates
            );
        }
        Err(e) => {
            tracing::error!("Failed to cleanup old logs: {}", e);
        }
    }

    info!("Discord logs cleanup job completed");
    Ok(())
}

async fn scan_for_media_recommendations(db: Database) -> Result<()> {
    info!("Starting media recommendations scan");

    // Get last scanned message ID
    let (last_scanned_id, last_scan_time) = match db.get_media_scan_checkpoint().await {
        Ok(checkpoint) => checkpoint,
        Err(_) => (0, chrono::Utc::now()),
    };

    info!(
        "Last scanned message ID: {}, last scan time: {}",
        last_scanned_id, last_scan_time
    );

    // Create media detector
    let detector = MediaDetector::new();

    // Process messages in batches
    const BATCH_SIZE: u32 = 1000;
    let mut messages_scanned = 0;
    let mut recommendations_found = 0;
    let mut current_last_id = last_scanned_id;

    loop {
        // Get next batch of unscanned messages
        let messages = match db.get_unscanned_messages(current_last_id, BATCH_SIZE).await {
            Ok(msgs) => msgs,
            Err(e) => {
                tracing::error!("Failed to fetch unscanned messages: {}", e);
                break;
            }
        };

        if messages.is_empty() {
            info!("No more messages to scan");
            break;
        }

        // Process each message
        for (msg_id, user_id, channel_id, guild_id, content, timestamp) in &messages {
            messages_scanned += 1;
            current_last_id = *msg_id;

            // Detect media recommendations
            let recommendations = detector.detect_media(&content);

            for rec in recommendations {
                if let Err(e) = db
                    .log_media_recommendation(
                        *msg_id,
                        *user_id,
                        *channel_id,
                        *guild_id,
                        rec.media_type,
                        &rec.title,
                        rec.url.as_deref(),
                        rec.confidence,
                        *timestamp,
                    )
                    .await
                {
                    tracing::error!("Failed to log media recommendation: {}", e);
                } else {
                    recommendations_found += 1;
                }
            }
        }

        // Update checkpoint after each batch
        if let Err(e) = db
            .update_media_scan_checkpoint(current_last_id, messages_scanned, recommendations_found)
            .await
        {
            tracing::error!("Failed to update scan checkpoint: {}", e);
        }

        // Log progress
        if messages_scanned % 10000 == 0 {
            info!(
                "Media scan progress: {} messages scanned, {} recommendations found",
                messages_scanned, recommendations_found
            );
        }

        // If we got less than a full batch, we're done
        if messages.len() < BATCH_SIZE as usize {
            break;
        }
    }

    info!(
        "Media recommendations scan completed. Scanned {} messages, found {} recommendations",
        messages_scanned, recommendations_found
    );

    Ok(())
}
