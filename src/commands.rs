use crate::db::Database;
use anyhow::Result;
use serenity::all::{
    Colour, Context, CreateEmbed, CreateMessage, EditMember, Message, Timestamp, UserId,
};
use tracing::{error, info};

pub struct CommandHandler {
    db: Database,
}

impl CommandHandler {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    async fn send_response(
        &self,
        ctx: &Context,
        msg: &Message,
        content: String,
        command: &str,
        success: bool,
    ) -> Result<()> {
        // Send the message
        let result = msg
            .author
            .direct_message(ctx, CreateMessage::new().content(&content))
            .await;

        // Log the response
        if let Err(e) = self
            .db
            .log_bot_response(msg.author.id.get(), Some(command), "dm", &content, success)
            .await
        {
            error!("Failed to log bot response: {}", e);
        }

        // Log to tracing
        info!(
            "[BOT RESPONSE] To {} ({}): {}",
            msg.author.name, msg.author.id, content
        );

        result?;
        Ok(())
    }

    async fn send_embed_response(
        &self,
        ctx: &Context,
        msg: &Message,
        embed: CreateEmbed,
        command: &str,
        success: bool,
    ) -> Result<()> {
        // Create a simple description for logging
        let embed_description = format!("Embed response for {} command", command);

        // Send the message
        let result = msg
            .author
            .direct_message(ctx, CreateMessage::new().embed(embed))
            .await;

        // Log the response
        if let Err(e) = self
            .db
            .log_bot_response(
                msg.author.id.get(),
                Some(command),
                "dm_embed",
                &embed_description,
                success,
            )
            .await
        {
            error!("Failed to log bot response: {}", e);
        }

        // Log to tracing
        info!(
            "[BOT RESPONSE] To {} ({}): {}",
            msg.author.name, msg.author.id, embed_description
        );

        result?;
        Ok(())
    }

    async fn find_user_by_handle(&self, ctx: &Context, handle: &str) -> Option<(UserId, String)> {
        // Remove @ prefix if present
        let handle = handle.strip_prefix('@').unwrap_or(handle);

        // Search through all guilds for a user with this handle
        for guild_id in ctx.cache.guilds() {
            if let Some(guild) = ctx.cache.guild(guild_id) {
                // Check members
                for (user_id, member) in &guild.members {
                    let user = &member.user;
                    // Check username (with or without discriminator)
                    if user.name == handle || user.tag() == handle {
                        return Some((*user_id, user.tag()));
                    }
                    // Check global handle (for users without discriminator)
                    if user.discriminator.is_none() && user.name == handle {
                        return Some((*user_id, user.tag()));
                    }
                    // Check server nickname
                    if let Some(nick) = &member.nick {
                        if nick == handle {
                            return Some((*user_id, user.tag()));
                        }
                    }
                }
            }
        }

        None
    }

    pub async fn handle_dm_command(&self, ctx: &Context, msg: &Message) -> Result<()> {
        let content = msg.content.trim();
        let parts: Vec<&str> = content.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        let command = parts[0].to_lowercase();

        match command.as_str() {
            "/help" => self.handle_help(ctx, msg, &parts[1..]).await?,
            "/kick" => self.handle_kick(ctx, msg, &parts[1..]).await?,
            "/ban" => self.handle_ban(ctx, msg, &parts[1..]).await?,
            "/timeout" => self.handle_timeout(ctx, msg, &parts[1..]).await?,
            "/cache" => self.handle_cache_toggle(ctx, msg, &parts[1..]).await?,
            _ => {
                // Suggest the most appropriate command
                let suggestion = self.suggest_command(&command);
                let mut response = format!("Unknown command: '{}'\n\n", parts[0]);

                if let Some(suggested) = suggestion {
                    response.push_str(&format!("Did you mean: {}\n\n", suggested));
                }

                response.push_str("Use /help to see all available commands.");

                self.send_response(ctx, msg, response, "unknown", false)
                    .await?;
            }
        }

        Ok(())
    }

    fn suggest_command(&self, input: &str) -> Option<&'static str> {
        let commands = vec![
            ("/help", vec!["help", "halp", "hlp", "h", "?"]),
            ("/kick", vec!["kick", "kik", "remove"]),
            ("/ban", vec!["ban", "bann", "block"]),
            ("/timeout", vec!["timeout", "mute", "silence", "quiet"]),
            ("/cache", vec!["cache", "cash", "media"]),
        ];

        // Check if the input (without /) matches any known aliases
        let input_lower = input.trim_start_matches('/').to_lowercase();

        for (command, aliases) in &commands {
            if aliases.iter().any(|&alias| alias == input_lower) {
                return Some(command);
            }
        }

        // Check for partial matches at the beginning
        for (command, aliases) in &commands {
            if aliases
                .iter()
                .any(|&alias| input_lower.starts_with(alias) || alias.starts_with(&input_lower))
            {
                return Some(command);
            }
        }

        // Default to help if no match found
        Some("/help")
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
                    "/kick <@user> [reason]",
                    "Kick a user from all guilds (whitelisted only)",
                    false,
                )
                .field(
                    "/ban <@user> [reason]",
                    "Ban a user from all guilds (whitelisted only)",
                    false,
                )
                .field(
                    "/timeout <@user> <duration_minutes> [reason]",
                    "Timeout a user in all guilds (whitelisted only)",
                    false,
                )
                .field(
                    "/cache [on|off]",
                    "Toggle media caching (whitelisted only)",
                    false,
                )
                .colour(Colour::BLUE);

            self.send_embed_response(ctx, msg, help_embed, "/help", true)
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

            self.send_response(
                ctx,
                msg,
                "Your help alert has been sent to moderators.".to_string(),
                "/help",
                true,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_kick(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            self.send_response(
                ctx,
                msg,
                "You are not authorized to use this command.".to_string(),
                "/kick",
                false,
            )
            .await?;
            return Ok(());
        }

        if args.is_empty() {
            self.send_response(
                ctx,
                msg,
                "Usage: /kick <@user> [reason]".to_string(),
                "/kick",
                false,
            )
            .await?;
            return Ok(());
        }

        let user_handle = args[0];
        let reason = if args.len() > 1 {
            Some(args[1..].join(" "))
        } else {
            None
        };

        if let Some((user_id, user_tag)) = self.find_user_by_handle(ctx, user_handle).await {
            let guilds = ctx.cache.guilds();
            let mut kicked_from = Vec::new();
            let mut failed_guilds = Vec::new();

            for guild_id in guilds {
                // Check if the user is in this guild
                let is_member = ctx
                    .cache
                    .guild(guild_id)
                    .map(|guild| guild.members.contains_key(&user_id))
                    .unwrap_or(false);

                if is_member {
                    let result = if let Some(reason) = reason.as_deref() {
                        guild_id.kick_with_reason(&ctx.http, user_id, reason).await
                    } else {
                        guild_id.kick(&ctx.http, user_id).await
                    };

                    match result {
                        Ok(_) => {
                            // Get guild name from cache
                            let guild_name = ctx
                                .cache
                                .guild(guild_id)
                                .map(|g| g.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());

                            info!(
                                "[MOD ACTION] {} kicked user {} ({}) from guild {} ({}) - reason: {}",
                                msg.author.id,
                                user_tag,
                                user_id,
                                guild_name,
                                guild_id,
                                reason.as_deref().unwrap_or("none")
                            );
                            kicked_from.push(guild_id);
                        }
                        Err(e) => {
                            failed_guilds.push((guild_id, e.to_string()));
                        }
                    }
                }
            }

            let mut response = String::new();
            if !kicked_from.is_empty() {
                let guild_names: Vec<String> = kicked_from
                    .iter()
                    .map(|g| {
                        ctx.cache
                            .guild(*g)
                            .map(|guild| format!("{} ({})", guild.name, g))
                            .unwrap_or_else(|| g.to_string())
                    })
                    .collect();

                response.push_str(&format!(
                    "Successfully kicked user {} from {} guild(s): {}\n",
                    user_tag,
                    kicked_from.len(),
                    guild_names.join(", ")
                ));
            }
            if !failed_guilds.is_empty() {
                response.push_str(&format!(
                    "Failed to kick from {} guild(s):\n",
                    failed_guilds.len()
                ));
                for (guild_id, error) in &failed_guilds {
                    let guild_name = ctx
                        .cache
                        .guild(*guild_id)
                        .map(|g| format!("{} ({})", g.name, guild_id))
                        .unwrap_or_else(|| guild_id.to_string());
                    response.push_str(&format!("- Guild {}: {}\n", guild_name, error));
                }
            }
            if kicked_from.is_empty() && failed_guilds.is_empty() {
                response = format!("User {} was not found in any guilds.", user_tag);
            }

            self.send_response(ctx, msg, response, "/kick", !kicked_from.is_empty())
                .await?;
        } else {
            self.send_response(
                ctx,
                msg,
                format!(
                    "User '{}' not found. Please use their username, @handle, or server nickname.",
                    user_handle
                ),
                "/kick",
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_ban(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            self.send_response(
                ctx,
                msg,
                "You are not authorized to use this command.".to_string(),
                "/ban",
                false,
            )
            .await?;
            return Ok(());
        }

        if args.is_empty() {
            self.send_response(
                ctx,
                msg,
                "Usage: /ban <@user> [reason]".to_string(),
                "/ban",
                false,
            )
            .await?;
            return Ok(());
        }

        let user_handle = args[0];
        let reason = if args.len() > 1 {
            Some(args[1..].join(" "))
        } else {
            None
        };

        if let Some((user_id, user_tag)) = self.find_user_by_handle(ctx, user_handle).await {
            let guilds = ctx.cache.guilds();
            let mut banned_from = Vec::new();
            let mut failed_guilds = Vec::new();

            for guild_id in guilds {
                let result = if let Some(reason) = reason.as_deref() {
                    guild_id
                        .ban_with_reason(&ctx.http, user_id, 0, reason)
                        .await
                } else {
                    guild_id.ban(&ctx.http, user_id, 0).await
                };

                match result {
                    Ok(_) => {
                        // Get guild name from cache
                        let guild_name = ctx
                            .cache
                            .guild(guild_id)
                            .map(|g| g.name.clone())
                            .unwrap_or_else(|| "Unknown".to_string());

                        info!(
                            "[MOD ACTION] {} banned user {} ({}) from guild {} ({}) - reason: {}",
                            msg.author.id,
                            user_tag,
                            user_id,
                            guild_name,
                            guild_id,
                            reason.as_deref().unwrap_or("none")
                        );
                        banned_from.push(guild_id);
                    }
                    Err(e) => {
                        failed_guilds.push((guild_id, e.to_string()));
                    }
                }
            }

            let mut response = String::new();
            if !banned_from.is_empty() {
                let guild_names: Vec<String> = banned_from
                    .iter()
                    .map(|g| {
                        ctx.cache
                            .guild(*g)
                            .map(|guild| format!("{} ({})", guild.name, g))
                            .unwrap_or_else(|| g.to_string())
                    })
                    .collect();

                response.push_str(&format!(
                    "Successfully banned user {} from {} guild(s): {}\n",
                    user_tag,
                    banned_from.len(),
                    guild_names.join(", ")
                ));
            }
            if !failed_guilds.is_empty() {
                response.push_str(&format!(
                    "Failed to ban from {} guild(s):\n",
                    failed_guilds.len()
                ));
                for (guild_id, error) in &failed_guilds {
                    let guild_name = ctx
                        .cache
                        .guild(*guild_id)
                        .map(|g| format!("{} ({})", g.name, guild_id))
                        .unwrap_or_else(|| guild_id.to_string());
                    response.push_str(&format!("- Guild {}: {}\n", guild_name, error));
                }
            }
            if banned_from.is_empty() && failed_guilds.is_empty() {
                response = "No guilds found to ban the user from.".to_string();
            }

            self.send_response(ctx, msg, response, "/ban", !banned_from.is_empty())
                .await?;
        } else {
            self.send_response(
                ctx,
                msg,
                format!(
                    "User '{}' not found. Please use their username, @handle, or server nickname.",
                    user_handle
                ),
                "/ban",
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_timeout(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            self.send_response(
                ctx,
                msg,
                "You are not authorized to use this command.".to_string(),
                "/timeout",
                false,
            )
            .await?;
            return Ok(());
        }

        if args.len() < 2 {
            self.send_response(
                ctx,
                msg,
                "Usage: /timeout <@user> <duration_minutes> [reason]".to_string(),
                "/timeout",
                false,
            )
            .await?;
            return Ok(());
        }

        let user_handle = args[0];
        let duration_minutes = args[1].parse::<u64>().ok();
        let reason = if args.len() > 2 {
            Some(args[2..].join(" "))
        } else {
            None
        };

        if let Some((user_id, user_tag)) = self.find_user_by_handle(ctx, user_handle).await {
            if let Some(duration_minutes) = duration_minutes {
                // Discord's maximum timeout duration is 28 days
                const MAX_TIMEOUT_MINUTES: u64 = 28 * 24 * 60;

                if duration_minutes > MAX_TIMEOUT_MINUTES {
                    self.send_response(
                        ctx,
                        msg,
                        format!(
                            "Timeout duration cannot exceed 28 days ({} minutes). You specified {} minutes.",
                            MAX_TIMEOUT_MINUTES, duration_minutes
                        ),
                        "/timeout",
                        false,
                    )
                    .await?;
                    return Ok(());
                }

                if duration_minutes == 0 {
                    self.send_response(
                        ctx,
                        msg,
                        "Timeout duration must be at least 1 minute".to_string(),
                        "/timeout",
                        false,
                    )
                    .await?;
                    return Ok(());
                }

                let timeout_until =
                    chrono::Utc::now() + chrono::Duration::minutes(duration_minutes as i64);
                let timeout_str = timeout_until.to_rfc3339();

                let guilds = ctx.cache.guilds();
                let mut timed_out_from = Vec::new();
                let mut failed_guilds = Vec::new();

                for guild_id in guilds {
                    // Check if the user is in this guild
                    let is_member = ctx
                        .cache
                        .guild(guild_id)
                        .map(|guild| guild.members.contains_key(&user_id))
                        .unwrap_or(false);

                    if is_member {
                        let edit_member =
                            EditMember::new().disable_communication_until(timeout_str.clone());
                        match guild_id.edit_member(&ctx.http, user_id, edit_member).await {
                            Ok(_) => {
                                // Get guild name from cache
                                let guild_name = ctx
                                    .cache
                                    .guild(guild_id)
                                    .map(|g| g.name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                info!(
                                "[MOD ACTION] {} timed out user {} ({}) in guild {} ({}) for {} minutes - reason: {}",
                                msg.author.id,
                                user_tag,
                                user_id,
                                guild_name,
                                guild_id,
                                duration_minutes,
                                reason.as_deref().unwrap_or("none")
                            );
                                timed_out_from.push(guild_id);
                            }
                            Err(e) => {
                                failed_guilds.push((guild_id, e.to_string()));
                            }
                        }
                    }
                }

                let mut response = String::new();
                if !timed_out_from.is_empty() {
                    let guild_names: Vec<String> = timed_out_from
                        .iter()
                        .map(|g| {
                            ctx.cache
                                .guild(*g)
                                .map(|guild| format!("{} ({})", guild.name, g))
                                .unwrap_or_else(|| g.to_string())
                        })
                        .collect();

                    response.push_str(&format!(
                        "Successfully timed out user {} for {} minutes in {} guild(s): {}\n",
                        user_tag,
                        duration_minutes,
                        timed_out_from.len(),
                        guild_names.join(", ")
                    ));
                }
                if !failed_guilds.is_empty() {
                    response.push_str(&format!(
                        "Failed to timeout in {} guild(s):\n",
                        failed_guilds.len()
                    ));
                    for (guild_id, error) in &failed_guilds {
                        let guild_name = ctx
                            .cache
                            .guild(*guild_id)
                            .map(|g| format!("{} ({})", g.name, guild_id))
                            .unwrap_or_else(|| guild_id.to_string());
                        response.push_str(&format!("- Guild {}: {}\n", guild_name, error));
                    }
                }
                if timed_out_from.is_empty() && failed_guilds.is_empty() {
                    response = format!("User {} was not found in any guilds.", user_tag);
                }

                self.send_response(ctx, msg, response, "/timeout", !timed_out_from.is_empty())
                    .await?;
            } else {
                self.send_response(
                    ctx,
                    msg,
                    "Invalid duration. Please specify duration in minutes.".to_string(),
                    "/timeout",
                    false,
                )
                .await?;
            }
        } else {
            self.send_response(
                ctx,
                msg,
                format!(
                    "User '{}' not found. Please use their username, @handle, or server nickname.",
                    user_handle
                ),
                "/timeout",
                false,
            )
            .await?;
        }

        Ok(())
    }

    async fn handle_cache_toggle(&self, ctx: &Context, msg: &Message, args: &[&str]) -> Result<()> {
        if !self.db.is_whitelisted(msg.author.id.get()).await? {
            self.send_response(
                ctx,
                msg,
                "You are not authorized to use this command.".to_string(),
                "/cache",
                false,
            )
            .await?;
            return Ok(());
        }

        if args.is_empty() {
            // Show current status
            let current_status = self
                .db
                .get_setting("cache_media")
                .await?
                .unwrap_or_else(|| "false".to_string());

            self.send_response(
                ctx,
                msg,
                format!(
                    "Media caching is currently: {}",
                    if current_status == "true" {
                        "ENABLED"
                    } else {
                        "DISABLED"
                    }
                ),
                "/cache",
                true,
            )
            .await?;
        } else {
            match args[0].to_lowercase().as_str() {
                "on" | "enable" | "true" => {
                    self.db.set_setting("cache_media", "true").await?;
                    info!("[SETTING] {} enabled media caching", msg.author.id);

                    self.send_response(
                        ctx,
                        msg,
                        "Media caching has been ENABLED".to_string(),
                        "/cache",
                        true,
                    )
                    .await?;
                }
                "off" | "disable" | "false" => {
                    self.db.set_setting("cache_media", "false").await?;
                    info!("[SETTING] {} disabled media caching", msg.author.id);

                    self.send_response(
                        ctx,
                        msg,
                        "Media caching has been DISABLED".to_string(),
                        "/cache",
                        true,
                    )
                    .await?;
                }
                _ => {
                    self.send_response(
                        ctx,
                        msg,
                        "Usage: /cache [on|off]".to_string(),
                        "/cache",
                        false,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }
}
