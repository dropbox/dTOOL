//! Basic Slack tools example
//!
//! Demonstrates all 4 Slack tools:
//! - Send message
//! - Get messages
//! - Get channel info
//! - Schedule message
//!
//! # Setup
//!
//! 1. Create a Slack app at https://api.slack.com/apps
//! 2. Add Bot Token Scopes: chat:write, channels:history, channels:read
//! 3. Install app to workspace
//! 4. Set SLACK_BOT_TOKEN environment variable
//! 5. Invite bot to #test channel: /invite @YourBot
//!
//! # Run
//!
//! ```bash
//! export SLACK_BOT_TOKEN="xoxb-your-token-here"
//! cargo run --example slack_basic
//! ```

use chrono::{Duration, Utc};
use dashflow::core::tools::Tool;
use dashflow_slack::{SlackGetChannel, SlackGetMessage, SlackScheduleMessage, SlackSendMessage};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get Slack bot token from environment
    let token = match std::env::var("SLACK_BOT_TOKEN") {
        Ok(token) => token,
        Err(_) => {
            println!("SLACK_BOT_TOKEN environment variable not set.");
            println!("Get your token from https://api.slack.com/apps");
            println!("Then run: export SLACK_BOT_TOKEN=\"xoxb-your-token-here\"");
            return Ok(());
        }
    };

    println!("=== Slack Tools Demo ===\n");

    // 1. List channels
    println!("1. Listing channels...");
    let get_channel = SlackGetChannel::new(token.clone());
    match get_channel._call_str(r#"{"limit": 5}"#.to_string()).await {
        Ok(result) => println!("Channels:\n{}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    // 2. Get specific channel info
    println!("2. Getting #test channel info...");
    match get_channel
        ._call_str(r##"{"channel": "#test"}"##.to_string())
        .await
    {
        Ok(result) => println!("Channel info: {}\n", result),
        Err(e) => println!("Error: {} (Make sure bot is invited to #test)\n", e),
    }

    // 3. Send a message
    println!("3. Sending message to #test...");
    let send_message = SlackSendMessage::new(token.clone());
    match send_message
        ._call_str(r##"{"channel": "#test", "text": "Hello from dashflow-slack! 🤖"}"##.to_string())
        .await
    {
        Ok(result) => println!("{}\n", result),
        Err(e) => println!("Error: {} (Make sure bot is invited to #test)\n", e),
    }

    // 4. Get recent messages
    println!("4. Getting recent messages from #test...");
    let get_message = SlackGetMessage::new(token.clone());
    match get_message
        ._call_str(r##"{"channel": "#test", "limit": 3}"##.to_string())
        .await
    {
        Ok(result) => println!("Recent messages:\n{}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    // 5. Schedule a message for 1 minute from now
    println!("5. Scheduling message for 1 minute from now...");
    let schedule_message = SlackScheduleMessage::new(token.clone());
    let post_at = (Utc::now() + Duration::minutes(1)).timestamp();
    let schedule_input = format!(
        r##"{{"channel": "#test", "text": "This is a scheduled message! It was sent 1 minute after the demo ran.", "post_at": {}}}"##,
        post_at
    );
    match schedule_message._call_str(schedule_input).await {
        Ok(result) => println!("{}\n", result),
        Err(e) => println!("Error: {}\n", e),
    }

    println!("=== Demo Complete ===");
    println!("\nNote: Check your #test channel to see the messages!");

    Ok(())
}
