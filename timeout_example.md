# Discord Member Timeout in Serenity v0.12

## Correct Implementation

In Serenity v0.12, to timeout a Discord member, you need to use the `edit_member` function with the `EditMember` builder. Here's the correct approach:

```rust
use serenity::all::{Context, EditMember, GuildId, UserId};
use chrono::Utc;

// Timeout a user for a specified number of minutes
async fn timeout_user(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
    duration_minutes: u64,
) -> Result<(), serenity::Error> {
    // Discord's maximum timeout duration is 28 days
    const MAX_TIMEOUT_MINUTES: u64 = 28 * 24 * 60;
    
    if duration_minutes > MAX_TIMEOUT_MINUTES {
        return Err(serenity::Error::Other("Timeout duration exceeds maximum"));
    }
    
    // Calculate the timeout end time
    let timeout_until = Utc::now() + chrono::Duration::minutes(duration_minutes as i64);
    let timeout_str = timeout_until.to_rfc3339();
    
    // Create the EditMember builder with the timeout
    let edit_member = EditMember::new().disable_communication_until(timeout_str);
    
    // Apply the timeout
    guild_id.edit_member(&ctx.http, user_id, edit_member).await?;
    
    Ok(())
}

// Remove a timeout (re-enable communication)
async fn remove_timeout(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<(), serenity::Error> {
    let edit_member = EditMember::new().enable_communication();
    guild_id.edit_member(&ctx.http, user_id, edit_member).await?;
    Ok(())
}
```

## Key Points

1. **Method Name**: Use `disable_communication_until()` which expects an ISO8601 timestamp string
2. **Maximum Duration**: Discord allows a maximum timeout of 28 days (40,320 minutes)
3. **Permissions Required**: The bot needs the `MODERATE_MEMBERS` permission
4. **Builder Pattern**: Serenity v0.12 uses direct builder types instead of closures
5. **Timestamp Format**: The method expects an RFC3339/ISO8601 formatted string

## Common Errors

- Passing a `Timestamp` object instead of a string to `disable_communication_until()`
- Exceeding the 28-day maximum timeout duration
- Missing the `MODERATE_MEMBERS` permission
- Using the old closure-based pattern from Serenity v0.11

## Implementation in Your Bot

The timeout functionality has been correctly implemented in `/sentinel/src/commands.rs` in the `handle_timeout` method. It includes:

- Validation of timeout duration (1 minute to 28 days)
- Proper error handling and user feedback
- Whitelist checking for authorization
- Correct usage of the EditMember builder pattern