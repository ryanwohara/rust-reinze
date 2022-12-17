mod runescape;

extern crate reqwest;
extern crate select;

use anyhow::Result;
use futures::prelude::*;
use irc::client::prelude::*;
use regex::Regex;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // We can also load the Config at runtime via Config::load("path/to/config.toml")
    let config = Config {
        nickname: Some("RustKick".to_string()),
        server: Some("fiery.swiftirc.net".to_string()),
        channels: vec!["#asdfghj".to_string()],
        ..Config::default()
    };

    let mut client = Client::from_config(config).await?;
    client.identify()?;

    let mut stream = client.stream()?;

    while let Some(message) = stream.next().await.transpose()? {
        print!("{}", message);

        if let Command::PRIVMSG(ref _channel, ref _message) = message.command {
            if let Some(target) = message.response_target() {
                let re = Regex::new(r"^[-+](\w+)\s*").unwrap();
                let matched = re.captures(_message);
                if matched.is_some() {
                    let cmd = matched.unwrap().get(1).unwrap().as_str();

                    match cmd {
                        "ping" => {
                            client.send_privmsg(target, "pong!")?;
                        }
                        "players" => {
                            match runescape::players().await {
                                Ok(message) => {
                                    match client.send_privmsg(target, message) {
                                        Ok(_) => {}
                                        Err(e) => {
                                            println!("Error sending message: {}", e);
                                        }
                                    };
                                }
                                Err(_) => {
                                    client.send_privmsg(target, "Error getting player count")?;
                                }
                            };
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    Ok(())
}
