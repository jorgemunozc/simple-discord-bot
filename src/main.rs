use anyhow::{anyhow, Context as _};
use once_cell::sync::Lazy;
use regex::Regex;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tracing::{error, info};
use url::Url;

// Only matches actual status/tweet links: x.com/<user>/status/<id> (optionally with trailing query/fragment)
static TWITTER_STATUS_URL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"https?://(?:www\.|mobile\.)?(?:x|twitter)\.com/\w+/status/\d+\S*")
        .expect("valid regex")
});
struct Bot;

fn convert_twitter_links(content: &str) -> Vec<String> {
    TWITTER_STATUS_URL_RE
        .find_iter(content)
        .filter_map(|m| {
            let mut url = Url::parse(m.as_str()).ok()?;
            let host = url.host_str()?;
            if !matches!(
                host,
                "x.com"
                    | "www.x.com"
                    | "mobile.x.com"
                    | "twitter.com"
                    | "www.twitter.com"
                    | "mobile.twitter.com"
            ) {
                return None;
            }

            //The filter after path_segments is to discard the empty element when the url has a trailing slash
            let segments: Vec<&str> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
            let is_status_link = segments.len() == 3
                && segments[1] == "status"
                && segments[2].chars().all(|c| c.is_ascii_digit());
            if !is_status_link {
                return None;
            }

            url.set_host(Some("fxtwitter.com")).ok()?;
            url.set_query(None);
            url.set_fragment(None);

            let path = url.path().trim_end_matches('/').to_string(); //trim_end_matches just in case
            url.set_path(&format!("{path}/en"));

            Some(url.into())
        })
        .collect()
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        let converted: Vec<String> = convert_twitter_links(&msg.content);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_basic_x_status_link() {
        let input = "check this out https://x.com/someuser/status/123456789";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn converts_basic_twitter_status_link() {
        let input = "https://twitter.com/someuser/status/987654321";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/987654321/en"]
        );
    }

    #[test]
    fn strips_query_params() {
        let input = "https://x.com/someuser/status/123456789?s=20&t=abcDEF123";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn strips_fragment() {
        let input = "https://x.com/someuser/status/123456789#some-fragment";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn strips_query_and_fragment_together() {
        let input = "https://x.com/someuser/status/123456789?s=20#frag";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn handles_www_subdomain() {
        let input = "https://www.x.com/someuser/status/123456789";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn handles_mobile_subdomain() {
        let input = "https://mobile.twitter.com/someuser/status/123456789";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn handles_multiple_links_in_one_message() {
        let input =
            "look at https://x.com/user1/status/111 and also https://twitter.com/user2/status/222";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec![
                "https://fxtwitter.com/user1/status/111/en",
                "https://fxtwitter.com/user2/status/222/en",
            ]
        );
    }

    #[test]
    fn ignores_profile_links_without_status() {
        let input = "check out https://x.com/someuser";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_non_status_paths() {
        let input =
            "https://x.com/search?q=rust https://x.com/someuser/likes https://x.com/i/bookmarks";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_status_link_with_non_numeric_id() {
        // malformed/unusual — shouldn't match "status" with a non-digit ID
        let input = "https://x.com/someuser/status/abc123";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_unrelated_domains() {
        let input = "https://youtube.com/watch?v=abc123 https://github.com/someuser/status/123";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_lookalike_domain() {
        // must not match something like "fakex.com" or "x.com.evil.net"
        let input =
            "https://x.com.evil.net/someuser/status/123456789 https://notx.com/someuser/status/123";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn ignores_message_with_no_links() {
        let input = "just a normal message, no links here";
        let result = convert_twitter_links(input);
        assert!(result.is_empty());
    }

    #[test]
    fn handles_status_link_with_trailing_slash() {
        let input = "https://x.com/someuser/status/123456789/";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn handles_http_scheme_not_just_https() {
        let input = "http://x.com/someuser/status/123456789";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["http://fxtwitter.com/someuser/status/123456789/en"]
        );
    }

    #[test]
    fn handles_large_numeric_status_id() {
        // Twitter status IDs are snowflake-style and can be 19 digits
        let input = "https://x.com/someuser/status/1234567890123456789";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/someuser/status/1234567890123456789/en"]
        );
    }

    #[test]
    fn handles_username_with_underscores_and_digits() {
        let input = "https://x.com/some_user_123/status/555555555";
        let result = convert_twitter_links(input);
        assert_eq!(
            result,
            vec!["https://fxtwitter.com/some_user_123/status/555555555/en"]
        );
    }
}
