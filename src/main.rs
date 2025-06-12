use std::env;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serde_json;
use serenity::all::{
    ChannelType, Colour, Command, Context, CreateAttachment, CreateEmbed,
    CreateInteractionResponse, CreateInteractionResponseMessage, EditMember, EventHandler,
    GatewayIntents, Guild, GuildChannel, GuildId, GuildMemberUpdateEvent,
    GuildScheduledEventUserAddEvent, GuildScheduledEventUserRemoveEvent, Interaction, Member,
    Message, Presence, Ready, ScheduledEvent, ScheduledEventStatus, User, VoiceState,
};
use serenity::async_trait;
use serenity::client::Client;
use tracing::{error, info};

mod commands;
mod db;
mod jobs;
mod media;
mod media_detector;

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

    fn format_snort_count(count: i64) -> String {
        match count {
            1 => "once".to_string(),
            2 => "twice".to_string(),
            3 => "thrice".to_string(),
            4..=20 => format!(
                "{} times",
                match count {
                    4 => "four",
                    5 => "five",
                    6 => "six",
                    7 => "seven",
                    8 => "eight",
                    9 => "nine",
                    10 => "ten",
                    11 => "eleven",
                    12 => "twelve",
                    13 => "thirteen",
                    14 => "fourteen",
                    15 => "fifteen",
                    16 => "sixteen",
                    17 => "seventeen",
                    18 => "eighteen",
                    19 => "nineteen",
                    20 => "twenty",
                    _ => unreachable!(),
                }
            ),
            _ => format!("{} times", count), // For larger numbers, use numeric format
        }
    }

    async fn get_random_snort_meme() -> Option<std::path::PathBuf> {
        let memes_dir = Path::new("memes/snort");

        // Create directory if it doesn't exist
        if !memes_dir.exists() {
            if let Err(e) = tokio::fs::create_dir_all(memes_dir).await {
                error!("Failed to create memes/snort directory: {}", e);
                return None;
            }
        }

        // Get list of image files
        let valid_extensions = ["jpg", "jpeg", "png", "gif", "webp"];
        let mut entries = match tokio::fs::read_dir(memes_dir).await {
            Ok(entries) => entries,
            Err(e) => {
                error!("Failed to read memes/snort directory: {}", e);
                return None;
            }
        };

        let mut image_files = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if valid_extensions
                        .contains(&extension.to_str().unwrap_or("").to_lowercase().as_str())
                    {
                        image_files.push(path);
                    }
                }
            }
        }

        if image_files.is_empty() {
            info!("No meme images found in memes/snort directory");
            return None;
        }

        // Select random image
        use rand::seq::SliceRandom;
        image_files.choose(&mut rand::thread_rng()).cloned()
    }

    async fn handle_help_slash(&self, ctx: &Context, command: &serenity::all::CommandInteraction) {
        let user_id = command.user.id.get();
        let is_super_user = self.db.is_super_user(user_id).await.unwrap_or(false);

        let mut embed = CreateEmbed::new()
            .title("Sentinel Help")
            .description("Available slash commands:")
            .field("/help", "Show this command list", false)
            .field(
                "/kick <user> [reason]",
                "Kick a user from all guilds (whitelisted only)",
                false,
            )
            .field(
                "/ban <user> [reason]",
                "Ban a user from all guilds (whitelisted only)",
                false,
            )
            .field(
                "/timeout <user> <duration> [reason]",
                "Timeout a user in all guilds (whitelisted only)",
                false,
            )
            .field(
                "/cache [on|off|status]",
                "Toggle or check media caching (whitelisted only)",
                false,
            )
            .field("/snort", "Snort some brightdust!", false)
            .field(
                "/watchlist",
                "Manage your media watchlist and view recommendations",
                false,
            );

        if is_super_user {
            embed = embed.field(
                "/whitelist <add|remove> <user>",
                "Manage command whitelist (super users only)",
                false,
            );
        }

        let embed = embed.colour(Colour::BLUE);

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true),
        );

        if let Err(e) = command.create_response(&ctx.http, response).await {
            error!("Failed to respond to /help command: {}", e);
        }

        self.db
            .log_bot_response(
                user_id,
                Some("/help"),
                "slash_command",
                "Help embed shown",
                true,
            )
            .await
            .ok();
    }

    async fn handle_kick_slash(&self, ctx: &Context, command: &serenity::all::CommandInteraction) {
        let user_id = command.user.id.get();

        if !self.db.is_whitelisted(user_id).await.unwrap_or(false) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You are not authorized to use this command.")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await.ok();
            self.db
                .log_bot_response(
                    user_id,
                    Some("/kick"),
                    "slash_command",
                    "Unauthorized",
                    false,
                )
                .await
                .ok();
            return;
        }

        let user_handle = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "user")
            .and_then(|opt| opt.value.as_str());

        let reason = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "reason")
            .and_then(|opt| opt.value.as_str());

        if let Some(user_handle) = user_handle {
            if let Some((target_id, user_tag)) = self
                .command_handler
                .find_user_by_handle(ctx, user_handle)
                .await
            {
                let guilds = ctx.cache.guilds();
                let mut kicked_from = Vec::new();
                let mut failed_guilds = Vec::new();

                for guild_id in guilds {
                    let is_member = ctx
                        .cache
                        .guild(guild_id)
                        .map(|guild| guild.members.contains_key(&target_id))
                        .unwrap_or(false);

                    if is_member {
                        let result = if let Some(reason) = reason {
                            guild_id
                                .kick_with_reason(&ctx.http, target_id, reason)
                                .await
                        } else {
                            guild_id.kick(&ctx.http, target_id).await
                        };

                        match result {
                            Ok(_) => {
                                let guild_name = ctx
                                    .cache
                                    .guild(guild_id)
                                    .map(|g| g.name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                info!("[MOD ACTION] {} kicked user {} ({}) from guild {} ({}) - reason: {}",
                                    user_id, user_tag, target_id, guild_name, guild_id,
                                    reason.unwrap_or("none"));
                                kicked_from.push(guild_id);
                            }
                            Err(e) => {
                                failed_guilds.push((guild_id, e.to_string()));
                            }
                        }
                    }
                }

                let mut response_content = String::new();
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

                    response_content.push_str(&format!(
                        "Successfully kicked user {} from {} guild(s): {}\\n",
                        user_tag,
                        kicked_from.len(),
                        guild_names.join(", ")
                    ));
                }
                if !failed_guilds.is_empty() {
                    response_content.push_str(&format!(
                        "Failed to kick from {} guild(s):\\n",
                        failed_guilds.len()
                    ));
                    for (guild_id, error) in &failed_guilds {
                        let guild_name = ctx
                            .cache
                            .guild(*guild_id)
                            .map(|g| format!("{} ({})", g.name, guild_id))
                            .unwrap_or_else(|| guild_id.to_string());
                        response_content.push_str(&format!("- Guild {}: {}\\n", guild_name, error));
                    }
                }
                if kicked_from.is_empty() && failed_guilds.is_empty() {
                    response_content = format!("User {} was not found in any guilds.", user_tag);
                }

                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_content.clone())
                        .ephemeral(true),
                );

                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/kick"),
                        "slash_command",
                        &response_content,
                        !kicked_from.is_empty(),
                    )
                    .await
                    .ok();
            } else {
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("User '{}' not found. Please use their username, @handle, or server nickname.", user_handle))
                        .ephemeral(true)
                );
                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/kick"),
                        "slash_command",
                        "User not found",
                        false,
                    )
                    .await
                    .ok();
            }
        }
    }

    async fn handle_ban_slash(&self, ctx: &Context, command: &serenity::all::CommandInteraction) {
        let user_id = command.user.id.get();

        if !self.db.is_whitelisted(user_id).await.unwrap_or(false) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You are not authorized to use this command.")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await.ok();
            self.db
                .log_bot_response(
                    user_id,
                    Some("/ban"),
                    "slash_command",
                    "Unauthorized",
                    false,
                )
                .await
                .ok();
            return;
        }

        let user_handle = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "user")
            .and_then(|opt| opt.value.as_str());

        let reason = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "reason")
            .and_then(|opt| opt.value.as_str());

        if let Some(user_handle) = user_handle {
            if let Some((target_id, user_tag)) = self
                .command_handler
                .find_user_by_handle(ctx, user_handle)
                .await
            {
                let guilds = ctx.cache.guilds();
                let mut banned_from = Vec::new();
                let mut failed_guilds = Vec::new();

                for guild_id in guilds {
                    let result = if let Some(reason) = reason {
                        guild_id
                            .ban_with_reason(&ctx.http, target_id, 0, reason)
                            .await
                    } else {
                        guild_id.ban(&ctx.http, target_id, 0).await
                    };

                    match result {
                        Ok(_) => {
                            let guild_name = ctx
                                .cache
                                .guild(guild_id)
                                .map(|g| g.name.clone())
                                .unwrap_or_else(|| "Unknown".to_string());

                            info!("[MOD ACTION] {} banned user {} ({}) from guild {} ({}) - reason: {}",
                                user_id, user_tag, target_id, guild_name, guild_id,
                                reason.unwrap_or("none"));
                            banned_from.push(guild_id);
                        }
                        Err(e) => {
                            failed_guilds.push((guild_id, e.to_string()));
                        }
                    }
                }

                let mut response_content = String::new();
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

                    response_content.push_str(&format!(
                        "Successfully banned user {} from {} guild(s): {}\\n",
                        user_tag,
                        banned_from.len(),
                        guild_names.join(", ")
                    ));
                }
                if !failed_guilds.is_empty() {
                    response_content.push_str(&format!(
                        "Failed to ban from {} guild(s):\\n",
                        failed_guilds.len()
                    ));
                    for (guild_id, error) in &failed_guilds {
                        let guild_name = ctx
                            .cache
                            .guild(*guild_id)
                            .map(|g| format!("{} ({})", g.name, guild_id))
                            .unwrap_or_else(|| guild_id.to_string());
                        response_content.push_str(&format!("- Guild {}: {}\\n", guild_name, error));
                    }
                }
                if banned_from.is_empty() && failed_guilds.is_empty() {
                    response_content = "No guilds found to ban the user from.".to_string();
                }

                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_content.clone())
                        .ephemeral(true),
                );

                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/ban"),
                        "slash_command",
                        &response_content,
                        !banned_from.is_empty(),
                    )
                    .await
                    .ok();
            } else {
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("User '{}' not found. Please use their username, @handle, or server nickname.", user_handle))
                        .ephemeral(true)
                );
                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/ban"),
                        "slash_command",
                        "User not found",
                        false,
                    )
                    .await
                    .ok();
            }
        }
    }

    async fn handle_timeout_slash(
        &self,
        ctx: &Context,
        command: &serenity::all::CommandInteraction,
    ) {
        let user_id = command.user.id.get();

        if !self.db.is_whitelisted(user_id).await.unwrap_or(false) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You are not authorized to use this command.")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await.ok();
            self.db
                .log_bot_response(
                    user_id,
                    Some("/timeout"),
                    "slash_command",
                    "Unauthorized",
                    false,
                )
                .await
                .ok();
            return;
        }

        let user_handle = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "user")
            .and_then(|opt| opt.value.as_str());

        let duration_minutes = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "duration")
            .and_then(|opt| opt.value.as_i64())
            .map(|v| v as u64);

        let reason = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "reason")
            .and_then(|opt| opt.value.as_str());

        if let (Some(user_handle), Some(duration_minutes)) = (user_handle, duration_minutes) {
            if let Some((target_id, user_tag)) = self
                .command_handler
                .find_user_by_handle(ctx, user_handle)
                .await
            {
                let timeout_until =
                    chrono::Utc::now() + chrono::Duration::minutes(duration_minutes as i64);
                let timeout_str = timeout_until.to_rfc3339();

                let guilds = ctx.cache.guilds();
                let mut timed_out_from = Vec::new();
                let mut failed_guilds = Vec::new();

                for guild_id in guilds {
                    let is_member = ctx
                        .cache
                        .guild(guild_id)
                        .map(|guild| guild.members.contains_key(&target_id))
                        .unwrap_or(false);

                    if is_member {
                        let edit_member =
                            EditMember::new().disable_communication_until(timeout_str.clone());
                        match guild_id
                            .edit_member(&ctx.http, target_id, edit_member)
                            .await
                        {
                            Ok(_) => {
                                let guild_name = ctx
                                    .cache
                                    .guild(guild_id)
                                    .map(|g| g.name.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                info!("[MOD ACTION] {} timed out user {} ({}) in guild {} ({}) for {} minutes - reason: {}",
                                    user_id, user_tag, target_id, guild_name, guild_id, duration_minutes,
                                    reason.unwrap_or("none"));
                                timed_out_from.push(guild_id);
                            }
                            Err(e) => {
                                failed_guilds.push((guild_id, e.to_string()));
                            }
                        }
                    }
                }

                let mut response_content = String::new();
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

                    response_content.push_str(&format!(
                        "Successfully timed out user {} for {} minutes in {} guild(s): {}\\n",
                        user_tag,
                        duration_minutes,
                        timed_out_from.len(),
                        guild_names.join(", ")
                    ));
                }
                if !failed_guilds.is_empty() {
                    response_content.push_str(&format!(
                        "Failed to timeout in {} guild(s):\\n",
                        failed_guilds.len()
                    ));
                    for (guild_id, error) in &failed_guilds {
                        let guild_name = ctx
                            .cache
                            .guild(*guild_id)
                            .map(|g| format!("{} ({})", g.name, guild_id))
                            .unwrap_or_else(|| guild_id.to_string());
                        response_content.push_str(&format!("- Guild {}: {}\\n", guild_name, error));
                    }
                }
                if timed_out_from.is_empty() && failed_guilds.is_empty() {
                    response_content = format!("User {} was not found in any guilds.", user_tag);
                }

                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_content.clone())
                        .ephemeral(true),
                );

                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/timeout"),
                        "slash_command",
                        &response_content,
                        !timed_out_from.is_empty(),
                    )
                    .await
                    .ok();
            } else {
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("User '{}' not found. Please use their username, @handle, or server nickname.", user_handle))
                        .ephemeral(true)
                );
                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/timeout"),
                        "slash_command",
                        "User not found",
                        false,
                    )
                    .await
                    .ok();
            }
        }
    }

    async fn handle_cache_slash(&self, ctx: &Context, command: &serenity::all::CommandInteraction) {
        let user_id = command.user.id.get();

        if !self.db.is_whitelisted(user_id).await.unwrap_or(false) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You are not authorized to use this command.")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await.ok();
            self.db
                .log_bot_response(
                    user_id,
                    Some("/cache"),
                    "slash_command",
                    "Unauthorized",
                    false,
                )
                .await
                .ok();
            return;
        }

        let action = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "action")
            .and_then(|opt| opt.value.as_str());

        let response_content = if let Some(action) = action {
            match action {
                "on" => {
                    self.db.set_setting("cache_media", "true").await.ok();
                    info!("[SETTING] {} enabled media caching", user_id);
                    "Media caching has been ENABLED".to_string()
                }
                "off" => {
                    self.db.set_setting("cache_media", "false").await.ok();
                    info!("[SETTING] {} disabled media caching", user_id);
                    "Media caching has been DISABLED".to_string()
                }
                "status" | _ => {
                    let current_status = self
                        .db
                        .get_setting("cache_media")
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "false".to_string());
                    format!(
                        "Media caching is currently: {}",
                        if current_status == "true" {
                            "ENABLED"
                        } else {
                            "DISABLED"
                        }
                    )
                }
            }
        } else {
            // Default to status if no action specified
            let current_status = self
                .db
                .get_setting("cache_media")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "false".to_string());
            format!(
                "Media caching is currently: {}",
                if current_status == "true" {
                    "ENABLED"
                } else {
                    "DISABLED"
                }
            )
        };

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content(response_content.clone())
                .ephemeral(true),
        );

        command.create_response(&ctx.http, response).await.ok();
        self.db
            .log_bot_response(
                user_id,
                Some("/cache"),
                "slash_command",
                &response_content,
                true,
            )
            .await
            .ok();
    }

    async fn handle_whitelist_slash(
        &self,
        ctx: &Context,
        command: &serenity::all::CommandInteraction,
    ) {
        let user_id = command.user.id.get();

        if !self.db.is_super_user(user_id).await.unwrap_or(false) {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("You are not authorized to use this command. Only super users can manage the whitelist.")
                    .ephemeral(true)
            );
            command.create_response(&ctx.http, response).await.ok();
            self.db
                .log_bot_response(
                    user_id,
                    Some("/whitelist"),
                    "slash_command",
                    "Unauthorized",
                    false,
                )
                .await
                .ok();
            return;
        }

        let action = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "action")
            .and_then(|opt| opt.value.as_str());

        let user_handle = command
            .data
            .options
            .iter()
            .find(|opt| opt.name == "user")
            .and_then(|opt| opt.value.as_str());

        if let (Some(action), Some(user_handle)) = (action, user_handle) {
            if let Some((target_id, user_tag)) = self
                .command_handler
                .find_user_by_handle(ctx, user_handle)
                .await
            {
                let response_content = match action {
                    "add" => {
                        if self
                            .db
                            .is_whitelisted(target_id.get())
                            .await
                            .unwrap_or(false)
                        {
                            format!("User {} is already whitelisted.", user_tag)
                        } else {
                            self.db.add_to_whitelist(target_id.get()).await.ok();
                            info!(
                                "[WHITELIST] {} added {} ({}) to whitelist",
                                user_id, user_tag, target_id
                            );
                            format!("Successfully added {} to the whitelist.", user_tag)
                        }
                    }
                    "remove" => {
                        if self
                            .db
                            .is_super_user(target_id.get())
                            .await
                            .unwrap_or(false)
                        {
                            format!(
                                "Cannot remove {} from whitelist as they are a super user.",
                                user_tag
                            )
                        } else {
                            self.db.remove_from_whitelist(target_id.get()).await.ok();
                            info!(
                                "[WHITELIST] {} removed {} ({}) from whitelist",
                                user_id, user_tag, target_id
                            );
                            format!("Successfully removed {} from the whitelist.", user_tag)
                        }
                    }
                    _ => "Invalid action".to_string(),
                };

                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(response_content.clone())
                        .ephemeral(true),
                );

                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/whitelist"),
                        "slash_command",
                        &response_content,
                        true,
                    )
                    .await
                    .ok();
            } else {
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content(format!("User '{}' not found. Please use their username, @handle, or server nickname.", user_handle))
                        .ephemeral(true)
                );
                command.create_response(&ctx.http, response).await.ok();
                self.db
                    .log_bot_response(
                        user_id,
                        Some("/whitelist"),
                        "slash_command",
                        "User not found",
                        false,
                    )
                    .await
                    .ok();
            }
        }
    }

    async fn handle_watchlist_slash(
        &self,
        ctx: &Context,
        command: &serenity::all::CommandInteraction,
    ) {
        let user_id = command.user.id.get();

        // Get the subcommand
        let subcommand_opt = command.data.options.first();
        if subcommand_opt.is_none() {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("No subcommand provided")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await.ok();
            return;
        }

        let subcommand = &subcommand_opt.unwrap().name;
        let subcommand_value = &subcommand_opt.unwrap().value;

        match subcommand.as_str() {
            "view" => {
                let view_type = if let serenity::all::CommandDataOptionValue::SubCommand(opts) =
                    subcommand_value
                {
                    opts.iter()
                        .find(|o| o.name == "type")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("mine")
                } else {
                    "mine"
                };

                if view_type == "mine" {
                    // Show user's watchlist
                    match self.db.get_user_watchlist(user_id, 10).await {
                        Ok(items) if !items.is_empty() => {
                            let mut embed = CreateEmbed::new()
                                .title("Your Watchlist")
                                .colour(Colour::BLUE);

                            for (media_type, title, url, priority, status) in items {
                                let field_value = format!(
                                    "Type: {} | Priority: {} | Status: {}{}",
                                    media_type,
                                    priority,
                                    status,
                                    url.as_ref()
                                        .map(|u| format!("\n[Link]({})", u))
                                        .unwrap_or_default()
                                );
                                embed = embed.field(title, field_value, false);
                            }

                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .embed(embed)
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Ok(_) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Your watchlist is empty! Use `/watchlist add` to add items.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Err(e) => {
                            error!("Failed to get watchlist: {}", e);
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Failed to retrieve your watchlist.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                    }
                } else {
                    // Show top recommendations
                    match self.db.get_top_recommendations(10, 7).await {
                        Ok(items) if !items.is_empty() => {
                            let mut embed = CreateEmbed::new()
                                .title("🔥 Top Media Recommendations (Past Week)")
                                .description("Based on what everyone's talking about!")
                                .colour(Colour::GOLD);

                            for (media_type, title, _avg_confidence, mentions, url) in items {
                                let emoji = match media_type.as_str() {
                                    "anime" => "🎌",
                                    "tv_show" => "📺",
                                    "movie" => "🎬",
                                    "game" => "🎮",
                                    "youtube" => "📹",
                                    "music" => "🎵",
                                    _ => "📋",
                                };

                                let field_value = format!(
                                    "{} {} | Mentioned {} times{}",
                                    emoji,
                                    media_type,
                                    mentions,
                                    url.as_ref()
                                        .map(|u| format!("\n[Link]({})", u))
                                        .unwrap_or_default()
                                );
                                embed = embed.field(title, field_value, false);
                            }

                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new().embed(embed),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Ok(_) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("No recommendations found yet. The bot needs to scan more messages!")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Err(e) => {
                            error!("Failed to get recommendations: {}", e);
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Failed to retrieve recommendations.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                    }
                }
            }
            "add" => {
                if let Some(opt) = command.data.options.first() {
                    if let serenity::all::CommandDataOptionValue::SubCommand(opts) = &opt.value {
                        let media_type = opts
                            .iter()
                            .find(|o| o.name == "type")
                            .and_then(|o| o.value.as_str())
                            .unwrap_or("other");
                        let title = opts
                            .iter()
                            .find(|o| o.name == "title")
                            .and_then(|o| o.value.as_str())
                            .unwrap_or("");
                        let url = opts
                            .iter()
                            .find(|o| o.name == "url")
                            .and_then(|o| o.value.as_str());
                        let priority = opts
                            .iter()
                            .find(|o| o.name == "priority")
                            .and_then(|o| o.value.as_i64())
                            .map(|p| p as i32);

                        match self
                            .db
                            .add_to_watchlist(user_id, media_type, title, url, priority, None)
                            .await
                        {
                            Ok(_) => {
                                let response = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(format!(
                                            "✅ Added **{}** to your {} watchlist!",
                                            title, media_type
                                        ))
                                        .ephemeral(true),
                                );
                                command.create_response(&ctx.http, response).await.ok();
                            }
                            Err(e) => {
                                error!("Failed to add to watchlist: {}", e);
                                let response = CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("Failed to add item to watchlist.")
                                        .ephemeral(true),
                                );
                                command.create_response(&ctx.http, response).await.ok();
                            }
                        }
                    }
                }
            }
            "remove" => {
                if let serenity::all::CommandDataOptionValue::SubCommand(opts) = subcommand_value {
                    let media_type = opts
                        .iter()
                        .find(|o| o.name == "type")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("other");
                    let title = opts
                        .iter()
                        .find(|o| o.name == "title")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("");

                    match self
                        .db
                        .remove_from_watchlist(user_id, media_type, title)
                        .await
                    {
                        Ok(true) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(format!(
                                        "✅ Removed **{}** from your watchlist!",
                                        title
                                    ))
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Ok(false) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Item not found in your watchlist.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Err(e) => {
                            error!("Failed to remove from watchlist: {}", e);
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Failed to remove item from watchlist.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                    }
                }
            }
            "priority" => {
                if let serenity::all::CommandDataOptionValue::SubCommand(opts) = subcommand_value {
                    let media_type = opts
                        .iter()
                        .find(|o| o.name == "type")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("other");
                    let title = opts
                        .iter()
                        .find(|o| o.name == "title")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("");
                    let new_priority = opts
                        .iter()
                        .find(|o| o.name == "new_priority")
                        .and_then(|o| o.value.as_i64())
                        .map(|p| p as i32)
                        .unwrap_or(50);

                    match self
                        .db
                        .update_watchlist_priority(user_id, media_type, title, new_priority)
                        .await
                    {
                        Ok(true) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(format!(
                                        "✅ Updated priority for **{}** to {}!",
                                        title, new_priority
                                    ))
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Ok(false) => {
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Item not found in your watchlist.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                        Err(e) => {
                            error!("Failed to update priority: {}", e);
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("Failed to update priority.")
                                    .ephemeral(true),
                            );
                            command.create_response(&ctx.http, response).await.ok();
                        }
                    }
                }
            }
            "scan" => {
                // Get the channel ID where the command was used
                let channel_id = command.channel_id;

                // Send initial ephemeral message
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("🔍 Starting channel scan for media recommendations...")
                        .ephemeral(true),
                );

                if let Err(e) = command.create_response(&ctx.http, response).await {
                    error!("Failed to send initial scan response: {}", e);
                    return;
                }

                // Perform the scan
                self.scan_channel_for_media(ctx, command, channel_id).await;
            }
            "export" => {
                if let serenity::all::CommandDataOptionValue::SubCommand(opts) = subcommand_value {
                    let data_type = opts
                        .iter()
                        .find(|o| o.name == "data")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("watchlist");
                    let format = opts
                        .iter()
                        .find(|o| o.name == "format")
                        .and_then(|o| o.value.as_str())
                        .unwrap_or("csv");
                    let days = opts
                        .iter()
                        .find(|o| o.name == "days")
                        .and_then(|o| o.value.as_i64())
                        .map(|d| d as i32)
                        .unwrap_or(30);

                    self.handle_watchlist_export(ctx, command, data_type, format, days).await;
                }
            }
            _ => {
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("Unknown subcommand")
                        .ephemeral(true),
                );
                command.create_response(&ctx.http, response).await.ok();
            }
        }

        // Log the command usage
        self.db
            .log_bot_response(
                user_id,
                Some("/watchlist"),
                "slash_command",
                &format!("Used watchlist {}", subcommand),
                true,
            )
            .await
            .ok();
    }

    async fn detect_and_log_media(
        &self,
        message_id: u64,
        user_id: u64,
        channel_id: u64,
        guild_id: u64,
        content: &str,
        timestamp: chrono::DateTime<chrono::Utc>,
    ) {
        use crate::media_detector::MediaDetector;

        // Create media detector
        let detector = MediaDetector::new();

        // Detect media in the content
        let recommendations = detector.detect_media(content);

        // Log each recommendation to the database
        for rec in recommendations {
            if let Err(e) = self
                .db
                .log_media_recommendation(
                    message_id,
                    user_id,
                    channel_id,
                    guild_id,
                    rec.media_type,
                    &rec.title,
                    rec.url.as_deref(),
                    rec.confidence,
                    timestamp,
                )
                .await
            {
                error!("Failed to log media recommendation: {}", e);
            } else {
                info!(
                    "Detected {} recommendation '{}' with {:.0}% confidence",
                    rec.media_type,
                    rec.title,
                    rec.confidence * 100.0
                );
            }
        }
    }

    async fn scan_channel_for_media(
        &self,
        ctx: &Context,
        command: &serenity::all::CommandInteraction,
        channel_id: serenity::all::ChannelId,
    ) {
        use crate::media_detector::{MediaDetector, MediaRecommendation};
        use std::collections::HashMap;

        // Fetch the last 100 messages from the channel
        let messages = match channel_id
            .messages(&ctx.http, serenity::all::GetMessages::new().limit(100))
            .await
        {
            Ok(msgs) => msgs,
            Err(e) => {
                error!("Failed to fetch messages: {}", e);
                // Send error followup
                let followup = serenity::all::CreateInteractionResponseFollowup::new()
                    .content("❌ Failed to fetch messages from this channel.")
                    .ephemeral(true);
                command.create_followup(&ctx.http, followup).await.ok();
                return;
            }
        };

        // Send progress update
        let followup = serenity::all::CreateInteractionResponseFollowup::new()
            .content(format!(
                "📊 Scanning {} messages for media recommendations...",
                messages.len()
            ))
            .ephemeral(true);
        command.create_followup(&ctx.http, followup).await.ok();

        // Create media detector
        let detector = MediaDetector::new();
        let mut all_recommendations: HashMap<String, (MediaRecommendation, i32, Vec<String>)> =
            HashMap::new();
        let mut scanned_count = 0;

        // Scan messages
        for msg in messages.iter() {
            // Skip bot messages
            if msg.author.bot {
                continue;
            }

            scanned_count += 1;

            // Detect media in this message
            let recommendations = detector.detect_media(&msg.content);

            for rec in recommendations {
                let key = format!("{}:{}", rec.media_type, rec.title);

                all_recommendations
                    .entry(key)
                    .and_modify(|(existing, count, users)| {
                        *count += 1;
                        // Update confidence if higher
                        if rec.confidence > existing.confidence {
                            existing.confidence = rec.confidence;
                        }
                        // Update URL if we found one and didn't have one before
                        if existing.url.is_none() && rec.url.is_some() {
                            existing.url = rec.url.clone();
                        }
                        // Track who mentioned it
                        let user_tag = format!(
                            "{}#{}",
                            msg.author.name,
                            msg.author
                                .discriminator
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "0000".to_string())
                        );
                        if !users.contains(&user_tag) {
                            users.push(user_tag);
                        }
                    })
                    .or_insert((
                        rec,
                        1,
                        vec![format!(
                            "{}#{}",
                            msg.author.name,
                            msg.author
                                .discriminator
                                .map(|d| d.to_string())
                                .unwrap_or_else(|| "0000".to_string())
                        )],
                    ));
            }

            // Send progress update every 25 messages
            if scanned_count % 25 == 0 {
                let progress_followup = serenity::all::CreateInteractionResponseFollowup::new()
                    .content(format!("⏳ Scanned {} messages...", scanned_count))
                    .ephemeral(true);
                command
                    .create_followup(&ctx.http, progress_followup)
                    .await
                    .ok();
            }
        }

        // Sort recommendations by mention count
        let mut sorted_recommendations: Vec<_> = all_recommendations.into_iter().collect();
        sorted_recommendations.sort_by(|a, b| b.1 .1.cmp(&a.1 .1));

        // Create the final embed
        if sorted_recommendations.is_empty() {
            let followup = serenity::all::CreateInteractionResponseFollowup::new()
                .content("No media recommendations found in the last 100 messages.")
                .ephemeral(false);
            command.create_followup(&ctx.http, followup).await.ok();
        } else {
            let mut embed = CreateEmbed::new()
                .title("📺 Media Recommendations from Channel Scan")
                .description(format!(
                    "Found {} unique media items from {} messages",
                    sorted_recommendations.len(),
                    scanned_count
                ))
                .colour(Colour::PURPLE)
                .footer(serenity::all::CreateEmbedFooter::new(
                    "Use /watchlist add to save items to your personal list",
                ));

            // Add top 10 recommendations
            for (i, (_key, (rec, count, users))) in
                sorted_recommendations.iter().take(10).enumerate()
            {
                let emoji = match rec.media_type {
                    "anime" => "🎌",
                    "tv_show" => "📺",
                    "movie" => "🎬",
                    "game" => "🎮",
                    "youtube" => "📹",
                    "music" => "🎵",
                    _ => "📋",
                };

                let users_str = if users.len() > 3 {
                    format!("{} and {} others", users[..3].join(", "), users.len() - 3)
                } else {
                    users.join(", ")
                };

                let field_value = format!(
                    "{} **{}**\nMentioned {} times by: {}\nConfidence: {:.0}%{}",
                    emoji,
                    rec.media_type,
                    count,
                    users_str,
                    rec.confidence * 100.0,
                    rec.url
                        .as_ref()
                        .map(|u| format!("\n[Link]({})", u))
                        .unwrap_or_default()
                );

                embed = embed.field(format!("{}. {}", i + 1, rec.title), field_value, false);
            }

            // Send the final visible embed
            let followup = serenity::all::CreateInteractionResponseFollowup::new().embed(embed);
            command.create_followup(&ctx.http, followup).await.ok();
        }
    }

    async fn handle_watchlist_export(
        &self,
        ctx: &Context,
        command: &serenity::all::CommandInteraction,
        data_type: &str,
        format: &str,
        days: i32,
    ) {
        let user_id = command.user.id.get();

        // Send initial response
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("📥 Generating export...")
                .ephemeral(true),
        );

        if let Err(e) = command.create_response(&ctx.http, response).await {
            error!("Failed to send initial export response: {}", e);
            return;
        }

        // Generate the export content
        let export_content = match data_type {
            "watchlist" => {
                match self.db.get_user_watchlist_full(user_id).await {
                    Ok(items) => self.generate_watchlist_export(items, format),
                    Err(e) => {
                        error!("Failed to get watchlist for export: {}", e);
                        let followup = serenity::all::CreateInteractionResponseFollowup::new()
                            .content("❌ Failed to retrieve watchlist data.")
                            .ephemeral(true);
                        command.create_followup(&ctx.http, followup).await.ok();
                        return;
                    }
                }
            }
            "recommendations" => {
                match self.db.get_user_recommendations(days).await {
                    Ok(items) => self.generate_recommendations_export(items, format, days),
                    Err(e) => {
                        error!("Failed to get recommendations for export: {}", e);
                        let followup = serenity::all::CreateInteractionResponseFollowup::new()
                            .content("❌ Failed to retrieve recommendations data.")
                            .ephemeral(true);
                        command.create_followup(&ctx.http, followup).await.ok();
                        return;
                    }
                }
            }
            _ => {
                let followup = serenity::all::CreateInteractionResponseFollowup::new()
                    .content("❌ Invalid export type.")
                    .ephemeral(true);
                command.create_followup(&ctx.http, followup).await.ok();
                return;
            }
        };

        // Create a file attachment
        let filename = format!(
            "{}_{}.{}",
            data_type,
            chrono::Utc::now().format("%Y%m%d_%H%M%S"),
            format
        );

        let attachment = serenity::all::CreateAttachment::bytes(
            export_content.as_bytes(),
            filename.clone(),
        );

        // Send the export as a file attachment
        let description = if data_type == "watchlist" { 
            "watchlist".to_string() 
        } else { 
            format!("recommendations from the last {} days", days) 
        };
        
        let followup = serenity::all::CreateInteractionResponseFollowup::new()
            .content(format!(
                "✅ Export complete! Here's your {} in {} format:",
                description,
                format.to_uppercase()
            ))
            .add_file(attachment)
            .ephemeral(true);

        if let Err(e) = command.create_followup(&ctx.http, followup).await {
            error!("Failed to send export file: {}", e);
            let error_followup = serenity::all::CreateInteractionResponseFollowup::new()
                .content("❌ Failed to send export file. The data might be too large.")
                .ephemeral(true);
            command.create_followup(&ctx.http, error_followup).await.ok();
        }
    }

    fn generate_watchlist_export(
        &self,
        items: Vec<(String, String, Option<String>, i32, String, Option<String>)>,
        format: &str,
    ) -> String {
        match format {
            "csv" => {
                let mut csv = String::from("Type,Title,URL,Priority,Status,Notes\n");
                for (media_type, title, url, priority, status, notes) in items {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{}\n",
                        self.escape_csv(&media_type),
                        self.escape_csv(&title),
                        self.escape_csv(&url.unwrap_or_default()),
                        priority,
                        self.escape_csv(&status),
                        self.escape_csv(&notes.unwrap_or_default())
                    ));
                }
                csv
            }
            "json" => {
                let json_items: Vec<serde_json::Value> = items
                    .into_iter()
                    .map(|(media_type, title, url, priority, status, notes)| {
                        serde_json::json!({
                            "type": media_type,
                            "title": title,
                            "url": url,
                            "priority": priority,
                            "status": status,
                            "notes": notes
                        })
                    })
                    .collect();
                
                serde_json::to_string_pretty(&serde_json::json!({
                    "watchlist": json_items,
                    "exported_at": chrono::Utc::now().to_rfc3339()
                })).unwrap_or_else(|_| "[]".to_string())
            }
            "markdown" => {
                let mut md = String::from("# My Media Watchlist\n\n");
                md.push_str(&format!("*Exported on {}*\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")));
                
                // Group by media type
                let mut grouped: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
                for item in items {
                    grouped.entry(item.0.clone()).or_insert_with(Vec::new).push(item);
                }
                
                for (media_type, items) in grouped {
                    let emoji = match media_type.as_str() {
                        "anime" => "🎌",
                        "tv_show" => "📺",
                        "movie" => "🎬",
                        "game" => "🎮",
                        "youtube" => "📹",
                        "music" => "🎵",
                        _ => "📋",
                    };
                    
                    md.push_str(&format!("\n## {} {}\n\n", emoji, self.capitalize(&media_type.replace('_', " "))));
                    
                    for (_, title, url, priority, status, notes) in items {
                        md.push_str(&format!("### {}\n", title));
                        md.push_str(&format!("- **Priority**: {}/100\n", priority));
                        md.push_str(&format!("- **Status**: {}\n", self.capitalize(&status.replace('_', " "))));
                        if let Some(url) = url {
                            md.push_str(&format!("- **Link**: [{}]({})\n", url, url));
                        }
                        if let Some(notes) = notes {
                            if !notes.is_empty() {
                                md.push_str(&format!("- **Notes**: {}\n", notes));
                            }
                        }
                        md.push('\n');
                    }
                }
                
                md
            }
            _ => String::new()
        }
    }

    fn generate_recommendations_export(
        &self,
        items: Vec<(String, String, Option<String>, f32, i64, Vec<String>)>,
        format: &str,
        days: i32,
    ) -> String {
        match format {
            "csv" => {
                let mut csv = String::from("Type,Title,URL,Confidence,Mentions,Recommended By\n");
                for (media_type, title, url, confidence, mentions, users) in items {
                    csv.push_str(&format!(
                        "{},{},{},{:.2},{},{}\n",
                        self.escape_csv(&media_type),
                        self.escape_csv(&title),
                        self.escape_csv(&url.unwrap_or_default()),
                        confidence,
                        mentions,
                        self.escape_csv(&users.join("; "))
                    ));
                }
                csv
            }
            "json" => {
                let json_items: Vec<serde_json::Value> = items
                    .into_iter()
                    .map(|(media_type, title, url, confidence, mentions, users)| {
                        serde_json::json!({
                            "type": media_type,
                            "title": title,
                            "url": url,
                            "confidence": confidence,
                            "mentions": mentions,
                            "recommended_by": users
                        })
                    })
                    .collect();
                
                serde_json::to_string_pretty(&serde_json::json!({
                    "recommendations": json_items,
                    "period_days": days,
                    "exported_at": chrono::Utc::now().to_rfc3339()
                })).unwrap_or_else(|_| "[]".to_string())
            }
            "markdown" => {
                let mut md = String::from("# Media Recommendations\n\n");
                md.push_str(&format!("*Based on the last {} days of activity*\n", days));
                md.push_str(&format!("*Exported on {}*\n\n", chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")));
                
                // Group by media type
                let mut grouped: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
                for item in items {
                    grouped.entry(item.0.clone()).or_insert_with(Vec::new).push(item);
                }
                
                for (media_type, items) in grouped {
                    let emoji = match media_type.as_str() {
                        "anime" => "🎌",
                        "tv_show" => "📺",
                        "movie" => "🎬",
                        "game" => "🎮",
                        "youtube" => "📹",
                        "music" => "🎵",
                        _ => "📋",
                    };
                    
                    md.push_str(&format!("\n## {} {}\n\n", emoji, self.capitalize(&media_type.replace('_', " "))));
                    
                    for (_, title, url, confidence, mentions, users) in items {
                        md.push_str(&format!("### {}\n", title));
                        md.push_str(&format!("- **Mentioned**: {} time{}\n", mentions, if mentions == 1 { "" } else { "s" }));
                        md.push_str(&format!("- **Confidence**: {:.0}%\n", confidence * 100.0));
                        if let Some(url) = url {
                            md.push_str(&format!("- **Link**: [{}]({})\n", url, url));
                        }
                        if !users.is_empty() {
                            md.push_str(&format!("- **Recommended by**: {}\n", users.join(", ")));
                        }
                        md.push('\n');
                    }
                }
                
                md
            }
            _ => String::new()
        }
    }

    fn escape_csv(&self, field: &str) -> String {
        if field.contains(',') || field.contains('"') || field.contains('\n') {
            format!("\"{}\"", field.replace('"', "\"\""))
        } else {
            field.to_string()
        }
    }

    fn capitalize(&self, s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }

    async fn handle_super_user_media_attachments(&self, ctx: &Context, msg: &Message) {
        use serenity::all::{CreateMessage, CreateActionRow, CreateButton, ButtonStyle};

        info!(
            "[SUPER USER MEDIA] {} sent {} attachment(s)",
            msg.author.name,
            msg.attachments.len()
        );

        // Get list of meme folders
        let meme_folders = self.get_meme_folders().await;

        // Process each attachment
        for attachment in &msg.attachments {
            // Skip Zone.Identifier files
            if attachment.filename.ends_with(":Zone.Identifier") || attachment.filename == "Zone.Identifier" {
                continue;
            }

            // Check if it's an image/video/gif
            let is_media = attachment
                .content_type
                .as_ref()
                .map(|ct| ct.starts_with("image/") || ct.starts_with("video/") || ct == "image/gif")
                .unwrap_or(false);

            if !is_media {
                let _ = msg
                    .channel_id
                    .say(
                        &ctx.http,
                        format!(
                            "⚠️ {} is not a supported media file (images/videos/gifs only)",
                            attachment.filename
                        ),
                    )
                    .await;
                continue;
            }

            // Create buttons for each folder (Discord limit is 5 buttons per row, 5 rows max = 25 buttons)
            let mut rows = Vec::new();
            let mut current_row = Vec::new();

            for (i, folder) in meme_folders.iter().enumerate() {
                if i >= 25 { // Max 25 buttons total
                    break;
                }

                let button = CreateButton::new(format!("meme_folder_{}", folder))
                    .label(folder)
                    .style(ButtonStyle::Primary);
                
                current_row.push(button);

                // Create new row every 5 buttons
                if current_row.len() == 5 {
                    rows.push(CreateActionRow::Buttons(current_row.clone()));
                    current_row.clear();
                }
            }

            // Add any remaining buttons as the last row
            if !current_row.is_empty() {
                rows.push(CreateActionRow::Buttons(current_row));
            }

            // Send message with buttons
            let message_content = format!(
                "🎨 New meme from **{}**!\n**File:** {}\n\nSelect a folder to save to:",
                msg.author.name,
                attachment.filename
            );

            let builder = CreateMessage::new()
                .content(message_content)
                .components(rows);

            match msg.channel_id.send_message(&ctx.http, builder).await {
                Ok(button_message) => {
                    info!(
                        "Created button message for attachment {} (message {})",
                        attachment.filename, button_message.id
                    );

                    // Store the attachment info for later processing when button is clicked
                    let button_key = format!(
                        "meme_buttons_{}_{}",
                        msg.channel_id.get(),
                        button_message.id.get()
                    );
                    let attachment_data = format!(
                        "{}|{}|{}",
                        attachment.url,
                        attachment.filename,
                        msg.author.id.get()
                    );

                    // Store in system settings temporarily
                    if let Err(e) = self.db.set_setting(&button_key, &attachment_data).await {
                        error!("Failed to store button attachment data: {}", e);
                    }
                }
                Err(e) => {
                    error!("Failed to create button message for attachment: {}", e);
                    let _ = msg
                        .channel_id
                        .say(&ctx.http, "❌ Failed to create selection buttons for this attachment")
                        .await;
                }
            }
        }
    }

    async fn handle_meme_folder_button(
        &self,
        ctx: &Context,
        component: serenity::all::ComponentInteraction,
    ) {
        use serenity::all::{CreateInteractionResponse, CreateInteractionResponseFollowup, EditMessage};

        // Send immediate acknowledgment
        let response = CreateInteractionResponse::Acknowledge;
        if let Err(e) = component.create_response(&ctx.http, response).await {
            error!("Failed to acknowledge button interaction: {}", e);
            return;
        }

        // Get the attachment data for this message
        let button_key = format!(
            "meme_buttons_{}_{}",
            component.channel_id.get(),
            component.message.id.get()
        );

        if let Ok(Some(attachment_data)) = self.db.get_setting(&button_key).await {
            // Parse attachment data
            let parts: Vec<&str> = attachment_data.split('|').collect();
            if parts.len() != 3 {
                error!("Invalid attachment data format");
                return;
            }

            let url = parts[0];
            let original_filename = parts[1];
            let _uploader_id = parts[2];

            // Extract folder name from custom_id
            let folder_name = component.data.custom_id.strip_prefix("meme_folder_").unwrap_or("");

            if folder_name.is_empty() {
                error!("Invalid folder name in button custom_id");
                return;
            }

            // Update the message to show processing
            let edit_msg = EditMessage::new()
                .content(format!("🎨 Processing meme: **{}**...", original_filename))
                .components(vec![]); // Remove buttons

            if let Err(e) = component.message.channel_id.edit_message(&ctx.http, component.message.id, edit_msg).await {
                error!("Failed to update message: {}", e);
            }

            // Download and save the meme
            let processing_key = format!("meme_processing_{}_{}", component.channel_id.get(), component.message.id.get());
            self.download_and_save_meme(
                ctx,
                &component.message,
                url,
                original_filename,
                &[folder_name.to_string()],
                &processing_key,
            ).await;

            // Clean up the button data
            let _ = self.db.delete_setting(&button_key).await;
        } else {
            // No attachment data found
            let followup = CreateInteractionResponseFollowup::new()
                .content("❌ Error: Could not find attachment data for this message.")
                .ephemeral(true);

            let _ = component.create_followup(&ctx.http, followup).await;
        }
    }

    async fn download_and_save_meme(
        &self,
        ctx: &Context,
        message: &Message,
        url: &str,
        original_filename: &str,
        folders: &[String],
        processing_key: &str,
    ) {
        use reqwest;
        use serenity::all::EditMessage;
        use tokio::fs;
        use uuid::Uuid;

        // Download the file once
        match reqwest::get(url).await {
            Ok(response) => {
                if let Ok(bytes) = response.bytes().await {
                    // Get file extension
                    let extension = std::path::Path::new(original_filename)
                        .extension()
                        .and_then(|e| e.to_str())
                        .or_else(|| {
                            // Try to get extension from URL if not in filename
                            if url.contains(".jpg") || url.contains(".jpeg") { Some("jpg") }
                            else if url.contains(".png") { Some("png") }
                            else if url.contains(".gif") { Some("gif") }
                            else if url.contains(".webp") { Some("webp") }
                            else if url.contains(".mp4") { Some("mp4") }
                            else if url.contains(".webm") { Some("webm") }
                            else { Some("png") } // Default to png
                        })
                        .unwrap_or("png");

                    // Generate unique filename
                    let new_filename = format!("{}.{}", Uuid::new_v4(), extension);

                    let mut saved_folders = Vec::new();
                    let mut failed_folders = Vec::new();

                    // Save to each selected folder
                    for folder_name in folders {
                        let folder_path = format!("./memes/{}", folder_name);
                        let file_path = format!("{}/{}", folder_path, new_filename);

                        // Ensure folder exists
                        if let Err(e) = fs::create_dir_all(&folder_path).await {
                            error!("Failed to create folder {}: {}", folder_path, e);
                            failed_folders.push(folder_name.clone());
                            continue;
                        }

                        // Save the file
                        match fs::write(&file_path, &bytes).await {
                            Ok(_) => {
                                info!("Saved meme to {}", file_path);
                                saved_folders.push(folder_name.clone());
                            }
                            Err(e) => {
                                error!("Failed to save file to {}: {}", file_path, e);
                                failed_folders.push(folder_name.clone());
                            }
                        }
                    }

                    // Update the message with results
                    let result_msg = if !saved_folders.is_empty() {
                        if saved_folders.len() == 1 {
                            format!(
                                "✅ Successfully saved **{}** to folder **{}**!",
                                original_filename, saved_folders[0]
                            )
                        } else {
                            format!(
                                "✅ Successfully saved **{}** to {} folders: **{}**!",
                                original_filename,
                                saved_folders.len(),
                                saved_folders.join("**, **")
                            )
                        }
                    } else {
                        format!("❌ Failed to save **{}** to any folder", original_filename)
                    };

                    let edit_msg = EditMessage::new().content(result_msg);
                    let _ = message
                        .channel_id
                        .edit_message(&ctx.http, message.id, edit_msg)
                        .await;

                    // Clean up the poll data from settings
                    let poll_key = format!(
                        "meme_poll_{}_{}",
                        message.channel_id.get(),
                        message.id.get()
                    );
                    let _ = self.db.delete_setting(&poll_key).await;
                    let _ = self.db.delete_setting(&processing_key).await;
                } else {
                    // Failed to get bytes
                    let error_msg = EditMessage::new().content(format!(
                        "❌ Failed to download **{}** - Invalid response",
                        original_filename
                    ));

                    let _ = message
                        .channel_id
                        .edit_message(&ctx.http, message.id, error_msg)
                        .await;
                    let _ = self.db.delete_setting(&processing_key).await;
                }
            }
            Err(e) => {
                error!("Failed to download attachment: {}", e);

                // Update the message with download error
                let error_msg = EditMessage::new().content(format!(
                    "❌ Failed to download **{}** - Network error",
                    original_filename
                ));

                let _ = message
                    .channel_id
                    .edit_message(&ctx.http, message.id, error_msg)
                    .await;
                let _ = self.db.delete_setting(&processing_key).await;
            }
        }
    }


    async fn get_meme_folders(&self) -> Vec<String> {
        use tokio::fs;

        let memes_dir = "./memes";
        let mut folders = Vec::new();

        // Ensure memes directory exists
        if let Err(e) = fs::create_dir_all(memes_dir).await {
            error!("Failed to create memes directory: {}", e);
            return folders;
        }

        // Read subdirectories
        match fs::read_dir(memes_dir).await {
            Ok(mut entries) => {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_dir() {
                            if let Some(folder_name) = entry.file_name().to_str() {
                                folders.push(folder_name.to_string());
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Failed to read memes directory: {}", e);
            }
        }

        // Sort folders alphabetically
        folders.sort();

        // If no folders exist, create a default one
        if folders.is_empty() {
            let default_folder = "general";
            if let Err(e) = fs::create_dir_all(format!("{}/{}", memes_dir, default_folder)).await {
                error!("Failed to create default meme folder: {}", e);
            } else {
                folders.push(default_folder.to_string());
            }
        }

        folders
    }

    async fn handle_autocomplete(
        &self,
        ctx: &Context,
        autocomplete: serenity::all::CommandInteraction,
    ) {
        // Find which option is being autocompleted by checking the resolved data
        let input = autocomplete
            .data
            .options
            .iter()
            .find(|opt| opt.name == "user")
            .and_then(|opt| opt.value.as_str())
            .unwrap_or("");

        // Search users in database
        let users = match self.db.search_users(input, 25).await {
            Ok(users) => users,
            Err(e) => {
                error!("Failed to search users for autocomplete: {}", e);
                vec![]
            }
        };

        // Create autocomplete choices
        let choices: Vec<serenity::all::AutocompleteChoice> = users
            .iter()
            .map(|(_user_id, username, global_handle, nickname)| {
                // Build display name
                let mut display = username.clone();
                if let Some(handle) = global_handle {
                    display = format!("@{}", handle);
                }
                if let Some(nick) = nickname {
                    display = format!("{} ({})", display, nick);
                }

                serenity::all::AutocompleteChoice::new(display.clone(), display)
            })
            .collect();

        // Send autocomplete response
        let response = CreateInteractionResponse::Autocomplete(
            serenity::all::CreateAutocompleteResponse::new().set_choices(choices),
        );

        if let Err(e) = autocomplete.create_response(&ctx.http, response).await {
            error!("Failed to send autocomplete response: {}", e);
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

            // Check if super user sent media attachments
            if !msg.attachments.is_empty()
                && self
                    .db
                    .is_super_user(msg.author.id.get())
                    .await
                    .unwrap_or(false)
            {
                self.handle_super_user_media_attachments(&ctx, &msg).await;
            } else if let Err(e) = self.command_handler.handle_dm_command(&ctx, &msg).await {
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

            // Detect and log media recommendations in the message
            if let Some(guild_id) = msg.guild_id {
                self.detect_and_log_media(
                    msg.id.get(),
                    msg.author.id.get(),
                    msg.channel_id.get(),
                    guild_id.get(),
                    &msg.content,
                    timestamp.to_utc(),
                )
                .await;
            }

            // Check if message contains a poll
            if let Some(poll) = &msg.poll {
                let poll_id = format!("{}_{}", msg.channel_id.get(), msg.id.get());
                let guild_id = msg.guild_id.unwrap_or_default().get();

                let question_text = poll.question.text.as_deref().unwrap_or("<no question>");
                info!(
                    "[POLL CREATE] User {} created poll '{}' in channel {} (message {})",
                    msg.author.id, question_text, msg.channel_id, msg.id
                );

                // Log poll creation
                if let Some(question_text) = &poll.question.text {
                    if let Err(e) = self
                        .db
                        .log_poll_created(
                            &poll_id,
                            msg.id.get(),
                            msg.channel_id.get(),
                            guild_id,
                            msg.author.id.get(),
                            question_text,
                            poll.expiry.map(|t| t.to_utc()),
                            poll.allow_multiselect,
                        )
                        .await
                    {
                        error!("Failed to log poll creation: {}", e);
                    }

                    // Check poll question for media recommendations
                    self.detect_and_log_media(
                        msg.id.get(),
                        msg.author.id.get(),
                        msg.channel_id.get(),
                        guild_id,
                        question_text,
                        timestamp.to_utc(),
                    )
                    .await;
                }

                // Log poll answers
                for (i, answer) in poll.answers.iter().enumerate() {
                    if let Some(answer_text) = &answer.poll_media.text {
                        if let Err(e) = self
                            .db
                            .log_poll_answer(
                                &poll_id,
                                i as u32,
                                answer_text,
                                answer
                                    .poll_media
                                    .emoji
                                    .as_ref()
                                    .map(|e| match e {
                                        serenity::all::PollMediaEmoji::Name(name) => name.clone(),
                                        serenity::all::PollMediaEmoji::Id(id) => id.to_string(),
                                    })
                                    .as_deref(),
                            )
                            .await
                        {
                            error!("Failed to log poll answer: {}", e);
                        }

                        // Check poll answer for media recommendations
                        self.detect_and_log_media(
                            msg.id.get(),
                            msg.author.id.get(),
                            msg.channel_id.get(),
                            guild_id,
                            answer_text,
                            timestamp.to_utc(),
                        )
                        .await;
                    }
                }
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

            // Detect and log media recommendations in edited message
            if let (Some(author), Some(guild_id)) = (event.author, event.guild_id) {
                if !author.bot {
                    self.detect_and_log_media(
                        event.id.get(),
                        author.id.get(),
                        event.channel_id.get(),
                        guild_id.get(),
                        &content,
                        event
                            .edited_timestamp
                            .map(|t| t.to_utc())
                            .unwrap_or_else(chrono::Utc::now),
                    )
                    .await;
                }
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

        // Register slash commands
        info!("Registering slash commands...");

        // Register /snort command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("snort").description("Snort some brightdust!"),
        )
        .await
        {
            Ok(command) => info!("Registered /snort command with ID: {}", command.id),
            Err(e) => error!("Failed to register /snort command: {}", e),
        }

        // Register /help command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("help").description("Show available commands"),
        )
        .await
        {
            Ok(command) => info!("Registered /help command with ID: {}", command.id),
            Err(e) => error!("Failed to register /help command: {}", e),
        }

        // Register /kick command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("kick")
                .description("Kick a user from all guilds")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "user",
                        "Username, @handle, or server nickname",
                    )
                    .required(true)
                    .set_autocomplete(true),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "reason",
                        "Reason for the kick",
                    )
                    .required(false),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /kick command with ID: {}", command.id),
            Err(e) => error!("Failed to register /kick command: {}", e),
        }

        // Register /ban command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("ban")
                .description("Ban a user from all guilds")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "user",
                        "Username, @handle, or server nickname",
                    )
                    .required(true)
                    .set_autocomplete(true),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "reason",
                        "Reason for the ban",
                    )
                    .required(false),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /ban command with ID: {}", command.id),
            Err(e) => error!("Failed to register /ban command: {}", e),
        }

        // Register /timeout command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("timeout")
                .description("Timeout a user in all guilds")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "user",
                        "Username, @handle, or server nickname",
                    )
                    .required(true)
                    .set_autocomplete(true),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::Integer,
                        "duration",
                        "Duration in minutes (max 40320 - 28 days)",
                    )
                    .required(true)
                    .min_int_value(1)
                    .max_int_value(40320),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "reason",
                        "Reason for the timeout",
                    )
                    .required(false),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /timeout command with ID: {}", command.id),
            Err(e) => error!("Failed to register /timeout command: {}", e),
        }

        // Register /cache command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("cache")
                .description("Toggle media caching")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "action",
                        "Enable or disable media caching",
                    )
                    .add_string_choice("on", "on")
                    .add_string_choice("off", "off")
                    .add_string_choice("status", "status")
                    .required(false),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /cache command with ID: {}", command.id),
            Err(e) => error!("Failed to register /cache command: {}", e),
        }

        // Register /whitelist command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("whitelist")
                .description("Manage command whitelist (super users only)")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "action",
                        "Add or remove from whitelist",
                    )
                    .add_string_choice("add", "add")
                    .add_string_choice("remove", "remove")
                    .required(true),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::String,
                        "user",
                        "Username, @handle, or server nickname",
                    )
                    .required(true)
                    .set_autocomplete(true),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /whitelist command with ID: {}", command.id),
            Err(e) => error!("Failed to register /whitelist command: {}", e),
        }

        // Register /watchlist command
        match Command::create_global_command(
            &ctx.http,
            serenity::all::CreateCommand::new("watchlist")
                .description("Manage your media watchlist or view top recommendations")
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::SubCommand,
                        "view",
                        "View your watchlist or top recommendations",
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "type",
                            "What to view",
                        )
                        .add_string_choice("my watchlist", "mine")
                        .add_string_choice("top recommendations", "top")
                        .required(false),
                    ),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::SubCommand,
                        "add",
                        "Add media to your watchlist",
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "type",
                            "Media type",
                        )
                        .add_string_choice("anime", "anime")
                        .add_string_choice("tv show", "tv_show")
                        .add_string_choice("movie", "movie")
                        .add_string_choice("game", "game")
                        .add_string_choice("youtube", "youtube")
                        .add_string_choice("music", "music")
                        .add_string_choice("other", "other")
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "title",
                            "Title of the media",
                        )
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "url",
                            "URL or link (optional)",
                        )
                        .required(false),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::Integer,
                            "priority",
                            "Priority (1-100, higher = more important)",
                        )
                        .min_int_value(1)
                        .max_int_value(100)
                        .required(false),
                    ),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::SubCommand,
                        "remove",
                        "Remove media from your watchlist",
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "type",
                            "Media type",
                        )
                        .add_string_choice("anime", "anime")
                        .add_string_choice("tv show", "tv_show")
                        .add_string_choice("movie", "movie")
                        .add_string_choice("game", "game")
                        .add_string_choice("youtube", "youtube")
                        .add_string_choice("music", "music")
                        .add_string_choice("other", "other")
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "title",
                            "Title of the media",
                        )
                        .required(true),
                    ),
                )
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::SubCommand,
                        "priority",
                        "Change priority of an item in your watchlist",
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "type",
                            "Media type",
                        )
                        .add_string_choice("anime", "anime")
                        .add_string_choice("tv show", "tv_show")
                        .add_string_choice("movie", "movie")
                        .add_string_choice("game", "game")
                        .add_string_choice("youtube", "youtube")
                        .add_string_choice("music", "music")
                        .add_string_choice("other", "other")
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "title",
                            "Title of the media",
                        )
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::Integer,
                            "new_priority",
                            "New priority (1-100)",
                        )
                        .min_int_value(1)
                        .max_int_value(100)
                        .required(true),
                    ),
                )
                .add_option(serenity::all::CreateCommandOption::new(
                    serenity::all::CommandOptionType::SubCommand,
                    "scan",
                    "Scan this channel for media recommendations",
                ))
                .add_option(
                    serenity::all::CreateCommandOption::new(
                        serenity::all::CommandOptionType::SubCommand,
                        "export",
                        "Export your watchlist or recommendations",
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "data",
                            "What to export",
                        )
                        .add_string_choice("my watchlist", "watchlist")
                        .add_string_choice("all recommendations", "recommendations")
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::String,
                            "format",
                            "Export format",
                        )
                        .add_string_choice("CSV", "csv")
                        .add_string_choice("JSON", "json")
                        .add_string_choice("Markdown", "markdown")
                        .required(true),
                    )
                    .add_sub_option(
                        serenity::all::CreateCommandOption::new(
                            serenity::all::CommandOptionType::Integer,
                            "days",
                            "Days of data to include (for recommendations)",
                        )
                        .min_int_value(1)
                        .max_int_value(365)
                        .required(false),
                    ),
                ),
        )
        .await
        {
            Ok(command) => info!("Registered /watchlist command with ID: {}", command.id),
            Err(e) => error!("Failed to register /watchlist command: {}", e),
        }

        let ctx_arc = Arc::new(ctx);
        if let Err(e) =
            jobs::start_background_jobs(ctx_arc, self.db.clone(), self.media_cache.clone()).await
        {
            error!("Failed to start background jobs: {}", e);
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
                match command.data.name.as_str() {
                    "help" => {
                        self.handle_help_slash(&ctx, &command).await;
                    }
                    "kick" => {
                        self.handle_kick_slash(&ctx, &command).await;
                    }
                    "ban" => {
                        self.handle_ban_slash(&ctx, &command).await;
                    }
                    "timeout" => {
                        self.handle_timeout_slash(&ctx, &command).await;
                    }
                    "cache" => {
                        self.handle_cache_slash(&ctx, &command).await;
                    }
                    "whitelist" => {
                        self.handle_whitelist_slash(&ctx, &command).await;
                    }
                    "watchlist" => {
                        self.handle_watchlist_slash(&ctx, &command).await;
                    }
                    "snort" => {
                        if let Some(guild_id) = command.guild_id {
                            let user_id = command.user.id.get();

                            // Check per-user cooldown
                            let cooldown_seconds =
                                self.db.get_snort_cooldown_seconds().await.unwrap_or(30);
                            let user_last_snort = self
                                .db
                                .get_user_last_snort_time(user_id)
                                .await
                                .unwrap_or(None);

                            let can_snort = if let Some(last_time) = user_last_snort {
                                let elapsed = chrono::Utc::now() - last_time;
                                elapsed.num_seconds() >= cooldown_seconds as i64
                            } else {
                                true
                            };

                            let (response_content, should_attach_meme) = if can_snort {
                                // Increment counter
                                match self
                                    .db
                                    .increment_snort_counter(user_id, guild_id.get())
                                    .await
                                {
                                    Ok(count) => {
                                        info!(
                                        "[SLASH COMMAND] {} used /snort in guild {} - count is now {}",
                                        command.user.name, guild_id, count
                                    );
                                        (
                                            format!(
                                                "We have snorted brightdust {}",
                                                Self::format_snort_count(count)
                                            ),
                                            true, // Successfully incremented, attach meme
                                        )
                                    }
                                    Err(e) => {
                                        error!("Failed to increment snort counter: {}", e);
                                        (
                                            "Failed to snort brightdust! Database error."
                                                .to_string(),
                                            false,
                                        )
                                    }
                                }
                            } else {
                                let remaining = cooldown_seconds as i64
                                    - (chrono::Utc::now() - user_last_snort.unwrap()).num_seconds();
                                (
                                    format!("Brightdust is still settling! Please wait {} more seconds before you can snort again.", remaining),
                                    false // On cooldown, don't attach meme
                                )
                            };

                            // Send response with meme only if we incremented the counter
                            let mut response_message = CreateInteractionResponseMessage::new()
                                .content(response_content.clone());

                            // Make cooldown messages ephemeral (only visible to the user)
                            if !should_attach_meme {
                                response_message = response_message.ephemeral(true);
                            }

                            // Add random meme only if we should (counter was incremented)
                            if should_attach_meme {
                                if let Some(meme_path) = Self::get_random_snort_meme().await {
                                    if let Ok(file_contents) = tokio::fs::read(&meme_path).await {
                                        let filename = meme_path
                                            .file_name()
                                            .and_then(|name| name.to_str())
                                            .unwrap_or("snort_meme.png");

                                        let attachment =
                                            CreateAttachment::bytes(file_contents, filename);
                                        response_message = response_message.add_file(attachment);

                                        info!("Attached snort meme: {}", meme_path.display());
                                    }
                                }
                            }

                            let response = CreateInteractionResponse::Message(response_message);

                            if let Err(e) = command.create_response(&ctx.http, response).await {
                                error!("Failed to respond to /snort command: {}", e);
                            }

                            // Log bot response
                            if let Err(e) = self
                                .db
                                .log_bot_response(
                                    user_id,
                                    Some("/snort"),
                                    "slash_command",
                                    &response_content,
                                    true,
                                )
                                .await
                            {
                                error!("Failed to log bot response: {}", e);
                            }
                        } else {
                            // Not in a guild
                            let response = CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("This command can only be used in a server!")
                                    .ephemeral(true),
                            );

                            if let Err(e) = command.create_response(&ctx.http, response).await {
                                error!("Failed to respond to /snort command: {}", e);
                            }
                        }
                    }
                    _ => {
                        error!("Unknown slash command: {}", command.data.name);
                    }
                }
            }
            Interaction::Autocomplete(autocomplete) => {
                self.handle_autocomplete(&ctx, autocomplete).await;
            }
            Interaction::Component(component) => {
                if component.data.custom_id.starts_with("meme_folder_") {
                    self.handle_meme_folder_button(&ctx, component).await;
                }
            }
            _ => {}
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

    // Poll tracking - Discord polls are sent as messages with poll data
    async fn poll_vote_add(&self, ctx: Context, add_event: serenity::all::MessagePollVoteAddEvent) {
        let user_id = add_event.user_id.get();
        let message_id = add_event.message_id.get();
        let answer_id = add_event.answer_id;

        // Get the message to extract poll details
        if let Ok(message) = ctx
            .http
            .get_message(add_event.channel_id, add_event.message_id)
            .await
        {
            if let Some(poll) = &message.poll {
                let poll_id = format!("{}_{}", message.channel_id.get(), message_id);
                let _guild_id = message.guild_id.unwrap_or_default().get();

                let question_text = poll.question.text.as_deref().unwrap_or("<no question>");
                info!(
                    "[POLL VOTE] User {} voted for answer {} in poll {} (message {})",
                    user_id,
                    answer_id.get(),
                    question_text,
                    message_id
                );

                // Log the vote
                if let Err(e) = self
                    .db
                    .log_poll_vote(&poll_id, user_id, answer_id.get() as u32)
                    .await
                {
                    error!("Failed to log poll vote: {}", e);
                }

                // We no longer use polls for meme management, only log the vote
            }
        }
    }

    async fn poll_vote_remove(
        &self,
        ctx: Context,
        remove_event: serenity::all::MessagePollVoteRemoveEvent,
    ) {
        let user_id = remove_event.user_id.get();
        let message_id = remove_event.message_id.get();
        let answer_id = remove_event.answer_id;

        if let Ok(message) = ctx
            .http
            .get_message(remove_event.channel_id, remove_event.message_id)
            .await
        {
            if let Some(poll) = &message.poll {
                let poll_id = format!("{}_{}", message.channel_id.get(), message_id);

                let question_text = poll.question.text.as_deref().unwrap_or("<no question>");
                info!(
                    "[POLL UNVOTE] User {} removed vote for answer {} in poll {} (message {})",
                    user_id,
                    answer_id.get(),
                    question_text,
                    message_id
                );

                // Remove the vote
                if let Err(e) = self
                    .db
                    .remove_poll_vote(&poll_id, user_id, answer_id.get() as u32)
                    .await
                {
                    error!("Failed to remove poll vote: {}", e);
                }
            }
        }
    }

    // Guild scheduled events tracking
    async fn guild_scheduled_event_create(&self, _ctx: Context, event: ScheduledEvent) {
        info!(
            "[EVENT CREATE] Event '{}' created by {} in guild {}",
            event.name,
            event.creator_id.unwrap_or_default(),
            event.guild_id
        );

        let status = match event.status {
            ScheduledEventStatus::Scheduled => "scheduled",
            ScheduledEventStatus::Active => "active",
            ScheduledEventStatus::Completed => "completed",
            ScheduledEventStatus::Canceled => "cancelled",
            _ => "unknown",
        };

        if let Err(e) = self
            .db
            .log_event_created(
                event.id.get(),
                event.guild_id.get(),
                event.channel_id.map(|c| c.get()),
                event.creator_id.unwrap_or_default().get(),
                &event.name,
                event.description.as_deref(),
                event.start_time.to_utc(),
                event.end_time.map(|t| t.to_utc()),
                event.metadata.as_ref().and_then(|m| m.location.as_deref()),
                status,
            )
            .await
        {
            error!("Failed to log event creation: {}", e);
        }

        // Check event name and description for media recommendations
        let event_text = format!(
            "{} {}",
            event.name,
            event.description.as_deref().unwrap_or("")
        );
        self.detect_and_log_media(
            event.id.get(), // Using event ID as message ID
            event.creator_id.unwrap_or_default().get(),
            event.channel_id.map(|c| c.get()).unwrap_or(0),
            event.guild_id.get(),
            &event_text,
            chrono::Utc::now(),
        )
        .await;
    }

    async fn guild_scheduled_event_update(&self, _ctx: Context, event: ScheduledEvent) {
        info!(
            "[EVENT UPDATE] Event '{}' updated in guild {}",
            event.name, event.guild_id
        );

        let status = match event.status {
            ScheduledEventStatus::Scheduled => "scheduled",
            ScheduledEventStatus::Active => "active",
            ScheduledEventStatus::Completed => "completed",
            ScheduledEventStatus::Canceled => "cancelled",
            _ => "unknown",
        };

        // Log as update - the database will handle updating existing record
        if let Err(e) = self
            .db
            .log_event_created(
                event.id.get(),
                event.guild_id.get(),
                event.channel_id.map(|c| c.get()),
                event.creator_id.unwrap_or_default().get(),
                &event.name,
                event.description.as_deref(),
                event.start_time.to_utc(),
                event.end_time.map(|t| t.to_utc()),
                event.metadata.as_ref().and_then(|m| m.location.as_deref()),
                status,
            )
            .await
        {
            error!("Failed to log event update: {}", e);
        }
    }

    async fn guild_scheduled_event_delete(&self, _ctx: Context, event: ScheduledEvent) {
        info!(
            "[EVENT DELETE] Event '{}' deleted from guild {}",
            event.name, event.guild_id
        );

        // Log the deletion as a status update
        if let Err(e) = self
            .db
            .log_event_update(
                event.id.get(),
                "status",
                Some("active/scheduled"),
                Some("deleted"),
                None,
            )
            .await
        {
            error!("Failed to log event deletion: {}", e);
        }
    }

    async fn guild_scheduled_event_user_add(
        &self,
        _ctx: Context,
        subscribed: GuildScheduledEventUserAddEvent,
    ) {
        info!(
            "[EVENT INTEREST] User {} expressed interest in event {} in guild {}",
            subscribed.user_id, subscribed.scheduled_event_id, subscribed.guild_id
        );

        if let Err(e) = self
            .db
            .log_event_interest(
                subscribed.scheduled_event_id.get(),
                subscribed.user_id.get(),
                "interested",
            )
            .await
        {
            error!("Failed to log event interest: {}", e);
        }
    }

    async fn guild_scheduled_event_user_remove(
        &self,
        _ctx: Context,
        unsubscribed: GuildScheduledEventUserRemoveEvent,
    ) {
        info!(
            "[EVENT UNINTEREST] User {} removed interest in event {} in guild {}",
            unsubscribed.user_id, unsubscribed.scheduled_event_id, unsubscribed.guild_id
        );

        if let Err(e) = self
            .db
            .remove_event_interest(
                unsubscribed.scheduled_event_id.get(),
                unsubscribed.user_id.get(),
            )
            .await
        {
            error!("Failed to remove event interest: {}", e);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    // Set up file logging with daily rotation
    let file_appender = tracing_appender::rolling::daily("logs", "sentinel.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    // Create a layer for file output (JSON format)
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .json()
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true);

    // Create a layer for console output
    let console_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .pretty();

    // Combine layers
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sentinel=info".parse()?)
                .add_directive("serenity=warn".parse()?),
        )
        .with(file_layer)
        .with(console_layer)
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

    info!("Setting up memes directory...");
    tokio::fs::create_dir_all("memes/snort").await?;

    let intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_VOICE_STATES
        | GatewayIntents::GUILD_MEMBERS
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_TYPING
        | GatewayIntents::GUILD_PRESENCES
        | GatewayIntents::GUILD_SCHEDULED_EVENTS
        | GatewayIntents::GUILD_MESSAGE_POLLS;

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
