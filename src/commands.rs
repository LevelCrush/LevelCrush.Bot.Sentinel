use crate::db::Database;
use anyhow::Result;
use serenity::all::{
    Colour, Context, CreateEmbed, CreateMessage, EditMember, GuildId, Message, Timestamp, UserId,
};
use tracing::info;

pub struct CommandHandler {
    db: Database,
}

impl CommandHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub async fn handle_dm_command(&self, ctx: &Context, msg: &Message) -> Result<()> {
        let content = msg.content.trim();
        let parts: Vec<&str> = content.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "/help" => self.handle_help(ctx, msg, &parts[1..]).await?,
            "/kick" => self.handle_kick(ctx, msg, &parts[1..]).await?,
            "/ban" => self.handle_ban(ctx, msg, &parts[1..]).await?,
            "/timeout" => self.handle_timeout(ctx, msg, &parts[1..]).await?,
            _ => {}
        }

        Ok(())
    }

    async fn handle_help(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            let help_embed = CreateEmbed::new()
                .title("Sentinel Help")
                .description("Available commands (DM only):")
                .field(
                    "/help <message>",
                    "Send a mod alert with your message",
                    false,
                )
                .field(
                    "/kick <user_id> <guild_id> [reason]",
                    "Kick a user (whitelisted only)",
                    false,
                )
                .field(
                    "/ban <user_id> <guild_id> [reason]",
                    "Ban a user (whitelisted only)",
                    false,
                )
                .field(
                    "/timeout <user_id> <guild_id> <duration_minutes> [reason]",
                    "Timeout a user (whitelisted only)",
                    false,
                )
                .colour(Colour::BLUE);

            msg.author
                .direct_message(&ctx.http, CreateMessage::new().embed(help_embed))
                .await?;
        } else {
            let alert_message = args.join(" ");
            let alert_embed = CreateEmbed::new()
                .title("🚨 Help Alert Received")
                .field(
                    "From",
                    &format!("{} ({})", msg.author.name, msg.author.id),
                    false,
                )
                .field("Message", &alert_message, false)
                .timestamp(Timestamp::now())
                .colour(Colour::RED);

            let guilds = ctx.cache.guilds();
            let mut system_channels = Vec::new();

            for guild_id in guilds {
                if let Some(guild) = ctx.cache.guild(guild_id) {
                    if let Some(system_channel_id) = guild.system_channel_id {
                        system_channels.push(system_channel_id);
                    }
                }
            }

            info!(
                "[HELP ALERT] {} sent help alert: {}",
                msg.author.id, alert_message
            );

            for channel_id in system_channels {
                let _ = channel_id
                    .send_message(&ctx.http, CreateMessage::new().embed(alert_embed.clone()))
                    .await;
            }

            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Your help alert has been sent to moderators."),
                )
                .await?;
        }

        Ok(())
    }

    async fn handle_kick(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("You are not authorized to use this command."),
                )
                .await?;
            return Ok(());
        }

        if args.len() < 2 {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Usage: /kick <user_id> <guild_id> [reason]"),
                )
                .await?;
            return Ok(());
        }

        let user_id = args[0].parse::<u64>().ok().map(UserId::new);
        let guild_id = args[1].parse::<u64>().ok().map(GuildId::new);
        let reason = if args.len() > 2 {
            Some(args[2..].join(" "))
        } else {
            None
        };

        if let (Some(user_id), Some(guild_id)) = (user_id, guild_id) {
            let result = if let Some(reason) = reason.as_deref() {
                guild_id.kick_with_reason(&ctx.http, user_id, reason).await
            } else {
                guild_id.kick(&ctx.http, user_id).await
            };

            match result {
                Ok(_) => {
                    info!(
                        "[MOD ACTION] {} kicked user {} from guild {} - reason: {}",
                        msg.author.id,
                        user_id,
                        guild_id,
                        reason.as_deref().unwrap_or("none")
                    );

                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!(
                                "Successfully kicked user {} from guild {}",
                                user_id, guild_id
                            )),
                        )
                        .await?;
                }
                Err(e) => {
                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!("Failed to kick user: {}", e)),
                        )
                        .await?;
                }
            }
        } else {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Invalid user ID or guild ID"),
                )
                .await?;
        }

        Ok(())
    }

    async fn handle_ban(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("You are not authorized to use this command."),
                )
                .await?;
            return Ok(());
        }

        if args.len() < 2 {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Usage: /ban <user_id> <guild_id> [reason]"),
                )
                .await?;
            return Ok(());
        }

        let user_id = args[0].parse::<u64>().ok().map(UserId::new);
        let guild_id = args[1].parse::<u64>().ok().map(GuildId::new);
        let reason = if args.len() > 2 {
            Some(args[2..].join(" "))
        } else {
            None
        };

        if let (Some(user_id), Some(guild_id)) = (user_id, guild_id) {
            let result = if let Some(reason) = reason.as_deref() {
                guild_id
                    .ban_with_reason(&ctx.http, user_id, 0, reason)
                    .await
            } else {
                guild_id.ban(&ctx.http, user_id, 0).await
            };

            match result {
                Ok(_) => {
                    info!(
                        "[MOD ACTION] {} banned user {} from guild {} - reason: {}",
                        msg.author.id,
                        user_id,
                        guild_id,
                        reason.as_deref().unwrap_or("none")
                    );

                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!(
                                "Successfully banned user {} from guild {}",
                                user_id, guild_id
                            )),
                        )
                        .await?;
                }
                Err(e) => {
                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!("Failed to ban user: {}", e)),
                        )
                        .await?;
                }
            }
        } else {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Invalid user ID or guild ID"),
                )
                .await?;
        }

        Ok(())
    }

    async fn handle_timeout(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("You are not authorized to use this command."),
                )
                .await?;
            return Ok(());
        }

        if args.len() < 3 {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content(
                        "Usage: /timeout <user_id> <guild_id> <duration_minutes> [reason]",
                    ),
                )
                .await?;
            return Ok(());
        }

        let user_id = args[0].parse::<u64>().ok().map(UserId::new);
        let guild_id = args[1].parse::<u64>().ok().map(GuildId::new);
        let duration_minutes = args[2].parse::<u64>().ok();
        let _reason = if args.len() > 3 {
            Some(args[3..].join(" "))
        } else {
            None
        };

        if let (Some(user_id), Some(guild_id), Some(duration_minutes)) =
            (user_id, guild_id, duration_minutes)
        {
            // Discord's maximum timeout duration is 28 days
            const MAX_TIMEOUT_MINUTES: u64 = 28 * 24 * 60;

            if duration_minutes > MAX_TIMEOUT_MINUTES {
                msg.author
                    .direct_message(
                        &ctx.http,
                        CreateMessage::new().content(format!(
                            "Timeout duration cannot exceed 28 days ({} minutes). You specified {} minutes.",
                            MAX_TIMEOUT_MINUTES, duration_minutes
                        )),
                    )
                    .await?;
                return Ok(());
            }

            if duration_minutes == 0 {
                msg.author
                    .direct_message(
                        &ctx.http,
                        CreateMessage::new().content("Timeout duration must be at least 1 minute"),
                    )
                    .await?;
                return Ok(());
            }

            let timeout_until =
                chrono::Utc::now() + chrono::Duration::minutes(duration_minutes as i64);
            let timeout_str = timeout_until.to_rfc3339();

            let edit_member = EditMember::new().disable_communication_until(timeout_str);
            match guild_id.edit_member(&ctx.http, user_id, edit_member).await {
                Ok(_) => {
                    info!(
                        "[MOD ACTION] {} timed out user {} in guild {} for {} minutes - reason: {}",
                        msg.author.id,
                        user_id,
                        guild_id,
                        duration_minutes,
                        _reason.as_deref().unwrap_or("none")
                    );

                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!(
                                "Successfully timed out user {} for {} minutes",
                                user_id, duration_minutes
                            )),
                        )
                        .await?;
                }
                Err(e) => {
                    msg.author
                        .direct_message(
                            &ctx.http,
                            CreateMessage::new().content(format!("Failed to timeout user: {}", e)),
                        )
                        .await?;
                }
            }
        } else {
            msg.author
                .direct_message(
                    &ctx.http,
                    CreateMessage::new().content("Invalid user ID, guild ID, or duration"),
                )
                .await?;
        }

        Ok(())
    }
}
