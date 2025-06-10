use std::env;
use std::sync::Arc;

use anyhow::Result;
use serenity::all::{
    ChannelType, Context, EventHandler, GatewayIntents, Guild, GuildChannel, GuildId,
    GuildMemberUpdateEvent, Member, Message, Presence, Ready, User, VoiceState,
};
use serenity::async_trait;
use serenity::client::Client;
use tracing::{error, info};

mod commands;
mod db;
mod jobs;
mod media;

use commands::CommandHandler;
use db::Database;
use media::MediaCache;

struct Handler {
    db: Database,
    command_handler: CommandHandler,
    media_cache: MediaCache,
}

impl Handler {
    fn new(db: Database, media_cache: MediaCache) -> Self {
        let command_handler = CommandHandler::new(db.clone());
        Self {
            db,
            command_handler,
            media_cache,
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        if msg.guild_id.is_none() {
            let timestamp = msg.timestamp;
            info!(
                "[DM MESSAGE] {} ({}): {}",
                msg.author.name, msg.author.id, msg.content
            );

            // Extract command if present
            let command = msg
                .content
                .trim()
                .split_whitespace()
                .next()
                .filter(|s| s.starts_with('/'))
                .map(|s| s.to_string());

            // Log DM to database
            if let Err(e) = self
                .db
                .log_dm_message(
                    msg.id.get(),
                    msg.author.id.get(),
                    &msg.content,
                    command.as_deref(),
                    timestamp.to_utc(),
                )
                .await
            {
                error!("Failed to log DM message: {}", e);
            }

            if let Err(e) = self.command_handler.handle_dm_command(&ctx, &msg).await {
                error!("Failed to handle DM command: {}", e);
            }
        } else {
            let timestamp = msg.timestamp;
            info!(
                "[MESSAGE] {} ({}): {}",
                msg.author.name, msg.author.id, msg.content
            );

            if let Err(e) = self
                .db
                .log_message(
                    msg.id.get(),
                    msg.author.id.get(),
                    msg.channel_id.get(),
                    &msg.content,
                    timestamp.to_utc(),
                )
                .await
            {
                error!("Failed to log message: {}", e);
            }

            // Handle attachments if media caching is enabled
            if !msg.attachments.is_empty() {
                if let Ok(Some(cache_enabled)) = self.db.get_setting("cache_media").await {
                    if cache_enabled == "true" {
                        for attachment in &msg.attachments {
                            info!(
                                "[ATTACHMENT] Message {} has attachment: {} ({})",
                                msg.id, attachment.filename, attachment.size
                            );

                            // Try to download and cache the attachment
                            let local_path = if let Ok(path) = self
                                .media_cache
                                .download_attachment(
                                    &attachment.url,
                                    &attachment.filename,
                                    attachment.content_type.as_deref(),
                                )
                                .await
                            {
                                self.media_cache.get_relative_path(&path)
                            } else {
                                error!("Failed to download attachment: {}", attachment.filename);
                                None
                            };

                            // Log attachment to database
                            if let Err(e) = self
                                .db
                                .log_attachment(
                                    msg.id.get(),
                                    attachment.id.get(),
                                    &attachment.filename,
                                    attachment.content_type.as_deref(),
                                    attachment.size as u64,
                                    &attachment.url,
                                    &attachment.proxy_url,
                                    local_path.as_deref(),
                                )
                                .await
                            {
                                error!("Failed to log attachment: {}", e);
                            }
                        }
                    }
                }

                let nickname = msg.member.as_ref().and_then(|m| m.nick.as_deref());
                info!(
                    "[USER UPDATE] {} ({}) - nickname: {}",
                    msg.author.name,
                    msg.author.id,
                    nickname.unwrap_or("none")
                );

                if let Err(e) = self
                    .db
                    .update_user(
                        msg.author.id.get(),
                        &msg.author.name,
                        msg.author
                            .discriminator
                            .map(|d| d.get().to_string())
                            .as_deref(),
                        if msg.author.discriminator.is_some() {
                            None
                        } else {
                            Some(&msg.author.name)
                        },
                        nickname,
                    )
                    .await
                {
                    error!("Failed to update user: {}", e);
                }
            }
        }
    }

    async fn message_update(
        &self,
        _ctx: Context,
        _old: Option<Message>,
        _new: Option<Message>,
        event: serenity::all::MessageUpdateEvent,
    ) {
        if let Some(content) = event.content {
            info!("[MESSAGE EDIT] Message {} edited to: {}", event.id, content);

            if let Err(e) = self.db.log_message_edit(event.id.get(), &content).await {
                error!("Failed to log message edit: {}", e);
            }
        }
    }

    async fn voice_state_update(&self, ctx: Context, old: Option<VoiceState>, new: VoiceState) {
        let user_id = new.user_id.get();

        let action = match (&old, &new.channel_id) {
            (None, Some(channel_id))
            | (
                Some(VoiceState {
                    channel_id: None, ..
                }),
                Some(channel_id),
            ) => Some(("join", channel_id.get())),
            (Some(old_state), None) if old_state.channel_id.is_some() => {
                if let Some(channel_id) = old_state.channel_id {
                    Some(("leave", channel_id.get()))
                } else {
                    None
                }
            }
            (Some(old_state), Some(new_channel_id))
                if old_state.channel_id != Some(*new_channel_id) =>
            {
                Some(("switch", new_channel_id.get()))
            }
            _ => None,
        };

        if let Some((action, channel_id)) = action {
            // Get channel name from cache
            let channel_name = {
                let channel_id = serenity::all::ChannelId::new(channel_id);
                let mut name = "Unknown".to_string();

                for guild_id in ctx.cache.guilds() {
                    if let Some(guild) = ctx.cache.guild(guild_id) {
                        if let Some(channel) = guild.channels.get(&channel_id) {
                            name = channel.name.clone();
                            break;
                        }
                    }
                }

                name
            };

            info!(
                "[VOICE] User {} {} channel {} ({})",
                user_id, action, channel_name, channel_id
            );

            if let Err(e) = self.db.log_voice_event(user_id, channel_id, action).await {
                error!("Failed to log voice event: {}", e);
            }
        }
    }

    async fn thread_create(&self, ctx: Context, thread: GuildChannel) {
        if thread.kind == ChannelType::PublicThread || thread.kind == ChannelType::PrivateThread {
            if let Some(owner_id) = thread.owner_id {
                let first_message = thread
                    .id
                    .messages(&ctx.http, serenity::all::GetMessages::new().limit(1))
                    .await;

                let content = if let Ok(messages) = &first_message {
                    messages
                        .first()
                        .map(|m| m.content.clone())
                        .unwrap_or_default()
                } else {
                    String::new()
                };

                // Get parent channel name
                let parent_channel_name = if let Some(parent_id) = thread.parent_id {
                    let mut name = "Unknown".to_string();

                    for guild_id in ctx.cache.guilds() {
                        if let Some(guild) = ctx.cache.guild(guild_id) {
                            if let Some(channel) = guild.channels.get(&parent_id) {
                                name = channel.name.clone();
                                break;
                            }
                        }
                    }

                    name
                } else {
                    "Unknown".to_string()
                };

                info!(
                    "[THREAD] User {} created thread '{}' in channel {} ({})",
                    owner_id, thread.name, parent_channel_name, thread.id
                );

                if let Err(e) = self
                    .db
                    .log_forum_thread(thread.id.get(), owner_id.get(), &thread.name, &content)
                    .await
                {
                    error!("Failed to log thread creation: {}", e);
                }
            }
        }
    }

    async fn guild_create(&self, _ctx: Context, guild: Guild, _is_new: Option<bool>) {
        info!("Connected to guild: {} ({})", guild.name, guild.id);

        for member in guild.members.values() {
            let user = &member.user;
            let nickname = member.nick.as_deref();
            let global_handle = if user.discriminator.is_some() {
                None
            } else {
                Some(user.name.as_str())
            };

            let discriminator = user.discriminator.map(|d| d.get().to_string());
            let discriminator_ref = discriminator.as_deref();

            if let Err(e) = self
                .db
                .update_user(
                    user.id.get(),
                    &user.name,
                    discriminator_ref,
                    global_handle,
                    nickname,
                )
                .await
            {
                error!("Failed to update user {}: {}", user.id, e);
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);

        let ctx_arc = Arc::new(ctx);
        if let Err(e) =
            jobs::start_background_jobs(ctx_arc, self.db.clone(), self.media_cache.clone()).await
        {
            error!("Failed to start background jobs: {}", e);
        }
    }

    async fn presence_update(&self, ctx: Context, new_data: Presence) {
        if let Some(guild_id) = new_data.guild_id {
            let user_id = new_data.user.id.get();

            // Get status information
            let status = new_data.status.name();

            // Get client status (desktop, mobile, web)
            let client_status = if let Some(cs) = &new_data.client_status {
                (
                    cs.desktop.as_ref().map(|s| s.name()).unwrap_or("offline"),
                    cs.mobile.as_ref().map(|s| s.name()).unwrap_or("offline"),
                    cs.web.as_ref().map(|s| s.name()).unwrap_or("offline"),
                )
            } else {
                ("offline", "offline", "offline")
            };

            // Get activity information
            let activity = new_data.activities.first().map(|act| {
                let activity_type = match act.kind {
                    serenity::all::ActivityType::Playing => "Playing",
                    serenity::all::ActivityType::Streaming => "Streaming",
                    serenity::all::ActivityType::Listening => "Listening",
                    serenity::all::ActivityType::Watching => "Watching",
                    serenity::all::ActivityType::Custom => "Custom",
                    serenity::all::ActivityType::Competing => "Competing",
                    _ => "Unknown",
                };

                (activity_type, act.name.as_str(), act.details.as_deref())
            });

            // Get guild name from cache
            let guild_name = ctx
                .cache
                .guild(guild_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            info!(
                "[PRESENCE] User {} in guild {} ({}) - Status: {} - Activity: {:?}",
                user_id,
                guild_name,
                guild_id,
                status,
                activity
                    .map(|(t, n, _)| format!("{} {}", t, n))
                    .unwrap_or_else(|| "None".to_string())
            );

            if let Err(e) = self
                .db
                .log_member_status(
                    user_id,
                    guild_id.get(),
                    Some(status),
                    Some(client_status),
                    activity,
                )
                .await
            {
                error!("Failed to log member status: {}", e);
            }
        }
    }

    async fn guild_member_update(
        &self,
        ctx: Context,
        old_if_available: Option<Member>,
        new: Option<Member>,
        _event: GuildMemberUpdateEvent,
    ) {
        if let Some(new) = new {
            let user_id = new.user.id.get();
            let guild_id = new.guild_id.get();

            // Check for nickname changes
            if let Some(old) = old_if_available {
                if old.nick != new.nick {
                    // Get guild name from cache
                    let guild_name = ctx
                        .cache
                        .guild(guild_id)
                        .map(|g| g.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());

                    info!(
                        "[NICKNAME] User {} in guild {} ({}) changed nickname from {:?} to {:?}",
                        user_id, guild_name, guild_id, old.nick, new.nick
                    );

                    if let Err(e) = self
                        .db
                        .log_nickname_change(
                            user_id,
                            guild_id,
                            old.nick.as_deref(),
                            new.nick.as_deref(),
                        )
                        .await
                    {
                        error!("Failed to log nickname change: {}", e);
                    }
                }
            }

            // Also update the user record with new nickname
            let user = &new.user;
            let global_handle = if user.discriminator.is_some() {
                None
            } else {
                Some(user.name.as_str())
            };

            let discriminator = user.discriminator.map(|d| d.get().to_string());

            if let Err(e) = self
                .db
                .update_user(
                    user_id,
                    &user.name,
                    discriminator.as_deref(),
                    global_handle,
                    new.nick.as_deref(),
                )
                .await
            {
                error!("Failed to update user: {}", e);
            }
        }
    }

    async fn channel_create(&self, ctx: Context, channel: GuildChannel) {
        let guild_id = channel.guild_id;
        // Get guild name from cache
        let guild_name = ctx
            .cache
            .guild(guild_id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        info!(
            "[CHANNEL CREATE] Channel '{}' ({}) created in guild {} ({})",
            channel.name, channel.id, guild_name, guild_id
        );

        if let Err(e) = self
            .db
            .log_channel_change(
                channel.id.get(),
                guild_id.get(),
                "create",
                Some("type"),
                None,
                Some(&format!("{:?}", channel.kind)),
                None,
            )
            .await
        {
            error!("Failed to log channel creation: {}", e);
        }
    }

    async fn channel_delete(
        &self,
        ctx: Context,
        channel: GuildChannel,
        _messages: Option<Vec<Message>>,
    ) {
        let guild_id = channel.guild_id;
        // Get guild name from cache
        let guild_name = ctx
            .cache
            .guild(guild_id)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        info!(
            "[CHANNEL DELETE] Channel '{}' ({}) deleted from guild {} ({})",
            channel.name, channel.id, guild_name, guild_id
        );

        if let Err(e) = self
            .db
            .log_channel_change(
                channel.id.get(),
                guild_id.get(),
                "delete",
                Some("name"),
                Some(&channel.name),
                None,
                None,
            )
            .await
        {
            error!("Failed to log channel deletion: {}", e);
        }
    }

    async fn channel_update(&self, ctx: Context, old: Option<GuildChannel>, new: GuildChannel) {
        if let Some(old_channel) = old {
            let guild_id = new.guild_id;
            let new_channel = &new;
            let channel_id = new_channel.id.get();

            // Get guild name from cache
            let guild_name = ctx
                .cache
                .guild(guild_id)
                .map(|g| g.name.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Check for name change
            if old_channel.name != new_channel.name {
                info!(
                    "[CHANNEL UPDATE] Channel {} name changed from '{}' to '{}' in guild {} ({})",
                    channel_id, old_channel.name, new_channel.name, guild_name, guild_id
                );

                if let Err(e) = self
                    .db
                    .log_channel_change(
                        channel_id,
                        guild_id.get(),
                        "update",
                        Some("name"),
                        Some(&old_channel.name),
                        Some(&new_channel.name),
                        None,
                    )
                    .await
                {
                    error!("Failed to log channel name change: {}", e);
                }
            }

            // Check for topic change (text channels)
            if old_channel.topic != new_channel.topic {
                info!(
                    "[CHANNEL UPDATE] Channel {} topic changed in guild {} ({})",
                    channel_id, guild_name, guild_id
                );

                if let Err(e) = self
                    .db
                    .log_channel_change(
                        channel_id,
                        guild_id.get(),
                        "update",
                        Some("topic"),
                        old_channel.topic.as_deref(),
                        new_channel.topic.as_deref(),
                        None,
                    )
                    .await
                {
                    error!("Failed to log channel topic change: {}", e);
                }
            }

            // Check for NSFW status change
            if old_channel.nsfw != new_channel.nsfw {
                info!(
                    "[CHANNEL UPDATE] Channel {} NSFW status changed from {} to {} in guild {} ({})",
                    channel_id, old_channel.nsfw, new_channel.nsfw, guild_name, guild_id
                );

                if let Err(e) = self
                    .db
                    .log_channel_change(
                        channel_id,
                        guild_id.get(),
                        "update",
                        Some("nsfw"),
                        Some(&old_channel.nsfw.to_string()),
                        Some(&new_channel.nsfw.to_string()),
                        None,
                    )
                    .await
                {
                    error!("Failed to log channel NSFW change: {}", e);
                }
            }

            // Check for position change
            if old_channel.position != new_channel.position {
                info!(
                    "[CHANNEL UPDATE] Channel {} position changed from {} to {} in guild {} ({})",
                    channel_id, old_channel.position, new_channel.position, guild_name, guild_id
                );

                if let Err(e) = self
                    .db
                    .log_channel_change(
                        channel_id,
                        guild_id.get(),
                        "update",
                        Some("position"),
                        Some(&old_channel.position.to_string()),
                        Some(&new_channel.position.to_string()),
                        None,
                    )
                    .await
                {
                    error!("Failed to log channel position change: {}", e);
                }
            }

            // Check for permission overwrites changes
            if old_channel.permission_overwrites != new_channel.permission_overwrites {
                info!(
                    "[CHANNEL UPDATE] Channel {} permissions changed in guild {} ({})",
                    channel_id, guild_name, guild_id
                );

                if let Err(e) = self
                    .db
                    .log_channel_change(
                        channel_id,
                        guild_id.get(),
                        "update",
                        Some("permissions"),
                        Some(&format!("{:?}", old_channel.permission_overwrites)),
                        Some(&format!("{:?}", new_channel.permission_overwrites)),
                        None,
                    )
                    .await
                {
                    error!("Failed to log channel permission change: {}", e);
                }
            }
        }
    }

    async fn guild_member_addition(&self, _ctx: Context, new_member: Member) {
        let guild_name = new_member
            .guild_id
            .to_guild_cached(&_ctx.cache)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        info!(
            "[MEMBER JOIN] {} ({}) joined guild {} ({})",
            new_member.user.name, new_member.user.id, guild_name, new_member.guild_id
        );

        // Update user in database
        let user = &new_member.user;
        let nickname = new_member.nick.as_deref();
        let global_handle = if user.discriminator.is_some() {
            None
        } else {
            Some(user.name.as_str())
        };

        let discriminator = user.discriminator.map(|d| d.get().to_string());

        if let Err(e) = self
            .db
            .update_user(
                user.id.get(),
                &user.name,
                discriminator.as_deref(),
                global_handle,
                nickname,
            )
            .await
        {
            error!("Failed to update user on guild join: {}", e);
        }
    }

    async fn guild_member_removal(
        &self,
        ctx: Context,
        guild_id: GuildId,
        user: User,
        _member_data: Option<Member>,
    ) {
        let guild_name = guild_id
            .to_guild_cached(&ctx.cache)
            .map(|g| g.name.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        info!(
            "[MEMBER LEAVE] {} ({}) left guild {} ({})",
            user.name, user.id, guild_name, guild_id
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sentinel=info".parse()?)
                .add_directive("serenity=warn".parse()?),
        )
        .init();

    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in environment");

    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "mysql://root:password@localhost/sentinel".to_string());

    info!("Connecting to database...");
    let db = Database::new(&database_url).await?;

    info!("Running database migrations...");
    db.run_migrations().await?;

    info!("Setting up media cache...");
    let media_cache = MediaCache::new("./media_cache");
    media_cache.ensure_directories().await?;

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_TYPING
        | GatewayIntents::GUILD_PRESENCES;

    let handler = Handler::new(db.clone(), media_cache.clone());

    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");

    info!("Starting Discord bot...");
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }

    Ok(())
}
