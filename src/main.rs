use std::env;
use std::sync::Arc;

use anyhow::Result;
use serenity::all::{
    ChannelType, Context, EventHandler, GatewayIntents, Guild, GuildChannel, Message, Ready,
    VoiceState,
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
            info!(
                "[DM COMMAND] {} ({}): {}",
                msg.author.name, msg.author.id, msg.content
            );

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

    async fn voice_state_update(&self, _ctx: Context, old: Option<VoiceState>, new: VoiceState) {
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
            info!("[VOICE] User {} {} channel {}", user_id, action, channel_id);

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

                info!(
                    "[THREAD] User {} created thread '{}' in channel {}",
                    owner_id, thread.name, thread.id
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
        | GatewayIntents::GUILD_MESSAGE_TYPING;

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
