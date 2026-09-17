use anyhow::{anyhow, Context as ct};
use regex::Regex;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{error, info};

struct Bot;

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.contains("https://x.com") || msg.content.contains("https://twitter.com") {
            let re =
                Regex::new(r#"((?<protocol>https://)(?<host>x|twitter)(?<rest>.com\S*))"#).unwrap();
            let result = re.captures(&msg.content).unwrap();
            let new_url = format!("{}fxtwitter{}", &result["protocol"], &result["rest"]);
            if let Err(error) = msg.channel_id.say(&ctx.http, new_url).await {
                error!("Error sending message: {error:?}");
            }
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN")
        .context("Environment variable for discord token not found.")?;

    if token.is_empty() {
        return Err(anyhow!("Discord token env variable is set but empty."));
    }

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Bot)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        error!("Client error: {why:?}");
    }
    Ok(())
}
