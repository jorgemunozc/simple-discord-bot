use anyhow::{anyhow, Context as _};
use once_cell::sync::Lazy;
use regex::Regex;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{error, info};

static TWITTER_LINK_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"https?://(?:www\.|mobile\.)?(?:x|twitter)\.com(?P<rest>/\S*)"#)
        .expect("valid regex")
});
struct Bot;

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let converted: Vec<String> = TWITTER_LINK_RE
            .captures_iter(&msg.content)
            .map(|caps| format!("https://fxtwitter.com{}", &caps["rest"]))
            .collect();

        if converted.is_empty() {
            return;
        }

        let reply = converted.join("\n");

        if let Err(error) = msg.channel_id.say(&ctx.http, reply).await {
            error!("Error sending message: {error:?}");
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
