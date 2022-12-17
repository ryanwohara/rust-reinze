extern crate reqwest;
extern crate select;

use anyhow::Result;
use format_num::NumberFormat;
use futures::prelude::*;
use irc::client::prelude::*;
use regex::Regex;
use serde::Deserialize;

#[derive(Deserialize)]
struct TotalRsPlayers {
    accounts: f32,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // We can also load the Config at runtime via Config::load("path/to/config.toml")
    let config = Config {
        nickname: Some("RustKick".to_owned()),
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
                    } else if cmd.eq("players") {
                        match players().await {
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
                }
            }
        }
    }

    Ok(())
}

async fn players() -> Result<String, ()> {
    let total_players = match get_rs3_players().await {
        Ok(resp) => resp,
        Err(_) => return Err(()),
    };
    let osrs_players = match get_osrs_players().await {
        Ok(resp) => resp,
        Err(_) => return Err(()),
    };

    let rs3_players = total_players - osrs_players;

    let total_registered = match get_total_players().await {
        Ok(resp) => resp,
        Err(_) => return Err(()),
    };

    let num = NumberFormat::new();

    // There are currently 81,203 OSRS players (68.88%) and 36,687 RS3 players (31.12%) online. (Total: 117,890) (Total Registered Accounts: 296,907,582)
    let string = format!("There are currently {} OSRS players ({}%) and {} RS3 players ({}%) online. (Total: {}) (Total Registered Accounts: {})", 
            num.format(",d", osrs_players), num.format(".2f", osrs_players / total_players * 100.0), 
            num.format(",d", rs3_players), num.format(".2f", rs3_players / total_players * 100.0), 
            num.format(",d", total_players), total_registered);

    Ok(string)
}

async fn get_rs3_players() -> Result<f32, ()> {
    let resp = match reqwest::get("https://www.runescape.com/player_count.js?varname=iPlayerCount&callback=jQuery36006339226594951519_1645569829067&_=1645569829068").await {
        Ok(resp) => resp,
        Err(e) => {
            println!("Error making HTTP request: {}", e);
        return Err(())
        },
    };

    let mut string = match resp.text().await {
        Ok(string) => string,
        Err(e) => {
            println!("Error getting text: {}", e);
            return Err(());
        }
    };

    // Remove the last two characters
    string.pop();
    string.pop();

    // Remove the first two characters
    let string = string.split("(").nth(1).unwrap();

    // Strip commas and convert to a float
    Ok(get_int(string))
}

async fn get_osrs_players() -> Result<f32, ()> {
    let resp = match reqwest::get("https://oldschool.runescape.com").await {
        Ok(resp) => resp,
        Err(e) => {
            println!("Error making HTTP request: {}", e);
            return Err(());
        }
    };

    let string = match resp.text().await {
        Ok(string) => string,
        Err(e) => {
            println!("Error getting text: {}", e);
            return Err(());
        }
    };

    let re = match Regex::new(
        r"<p class='player-count'>There are currently ([\d,]+) people playing!</p>",
    ) {
        Ok(re) => re,
        Err(e) => {
            println!("Error creating regex: {}", e);
            return Err(());
        }
    };
    let matched = re.captures(&string);
    let string = matched.unwrap().get(1).unwrap().as_str();

    // Strip commas and convert to a float
    Ok(get_int(string))
}

async fn get_total_players() -> Result<f32, ()> {
    let resp = match reqwest::get(
        "https://secure.runescape.com/m=account-creation-reports/rsusertotal.ws",
    )
    .await
    {
        Ok(resp) => resp,
        Err(e) => {
            println!("Error making HTTP request: {}", e);
            return Err(());
        }
    };

    let totaljson: TotalRsPlayers = match resp.json::<TotalRsPlayers>().await {
        Ok(totaljson) => totaljson,
        Err(e) => {
            println!("Error getting json: {}", e);
            return Err(());
        }
    };

    Ok(totaljson.accounts)
}

fn get_int(string: &str) -> f32 {
    // Strip commas and convert to a float
    string.replace(",", "").parse::<f32>().unwrap_or(0.0)
}
