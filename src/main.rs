use std::env;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use serenity::all::{
    ChannelType, Colour, Command, Context, CreateAttachment, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, EditMember, EventHandler, GatewayIntents, Guild,
    GuildChannel, GuildId, GuildMemberUpdateEvent, Interaction, Member, Message, Presence, Ready,
    User, VoiceState,
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
                    if valid_extensions.contains(&extension.to_str().unwrap_or("").to_lowercase().as_str()) {
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
            .field("/snort", "Snort some brightdust!", false);

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
                                            true // Successfully incremented, attach meme
                                        )
                                    }
                                    Err(e) => {
                                        error!("Failed to increment snort counter: {}", e);
                                        ("Failed to snort brightdust! Database error.".to_string(), false)
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
                                        
                                        let attachment = CreateAttachment::bytes(file_contents, filename);
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
