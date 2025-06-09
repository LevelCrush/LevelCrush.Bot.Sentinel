use crate::db::Database;
use anyhow::Result;
use serenity::all::Context;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

pub async fn start_background_jobs(ctx: Arc<Context>, db: Database) -> Result<()> {
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
        info!(
            "Syncing {} members from guild {}",
            members_data.len(),
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
