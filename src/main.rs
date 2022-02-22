use futures::prelude::*;
use irc::client::prelude::*;
use regex::Regex;

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
    // We can also load the Config at runtime via Config::load("path/to/config.toml")
    let config = Config {
        nickname: Some("the-irc-crate".to_owned()),
        server: Some("fiery.swiftirc.net".to_owned()),
        channels: vec!["#asdfghj".to_owned()],
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

                    if cmd.eq("ping") {
                        client.send_privmsg(target, "pong!")?;
                    }
                }
            }
        }
    }

    Ok(())
}
