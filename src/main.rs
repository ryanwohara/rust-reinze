use anyhow::Result;
use format_num::NumberFormat;
use futures::prelude::*;
use irc::client::prelude::*;
use regex::Regex;

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
                        client = players(client, target).await?;
                    }
                }
            }
        }
    }

    Ok(())
}

async fn players(client: Client, target: &str) -> Result<Client, anyhow::Error> {
    let rs3resp = get_url("https://www.runescape.com/player_count.js?varname=iPlayerCount&callback=jQuery36006339226594951519_1645569829067&_=1645569829068").await;
    let osrsresp = get_url("https://oldschool.runescape.com").await;

    // Remove the last two characters
    let mut rs3string = rs3resp?;
    rs3string.pop();
    rs3string.pop();

    let total_players = get_int(rs3string.split("(").nth(1).unwrap());

    let osrs_string = osrsresp?;
    let osrs_re =
        Regex::new(r"<p class='player-count'>There are currently ([\d,]+) people playing!</p>")
            .unwrap();
    let matched = osrs_re.captures(&osrs_string);
    let osrs_players = matched.unwrap().get(1).unwrap().as_str();
    let rs3_players = total_players - get_int(osrs_players);

    let totalresp = get_url("https://secure.runescape.com/m=account-creation-reports/rsusertotal.ws?callback=jQuery36004266025351340994_1645574453620&_=1645574453621").await;
    let mut totalstring = totalresp?;
    // Remove the last four characters
    totalstring.pop();
    totalstring.pop();
    totalstring.pop();
    totalstring.pop();
    let total_registered = totalstring.split("\"").nth(5).unwrap();

    let num = NumberFormat::new();

    // There are currently 81,203 OSRS players (68.88%) and 36,687 RS3 players (31.12%) online. (Total: 117,890) (Total Registered Accounts: 296,907,582)
    client.send_privmsg(target, format!("There are currently {} OSRS players ({}%) and {} RS3 players ({}%) online. (Total: {}) (Total Registered Accounts: {})", osrs_players, num.format(".2f", get_int(osrs_players) / total_players * 100.0), num.format(",d", rs3_players), num.format(".2f", rs3_players / total_players * 100.0), num.format(",d", total_players), total_registered))?;

    Ok(client)
}

async fn curl(url: &str) -> Result<String, reqwest::Error> {
    Ok(reqwest::get(url).await?.text().await?)
}

async fn get_url(url: &str) -> Result<String, reqwest::Error> {
    match curl(url).await {
        Ok(s) => Ok(s),
        Err(e) => Ok(e.to_string()),
    }
}

fn get_int(string: &str) -> f32 {
    // Strip commas and convert to a float
    return string.replace(",", "").parse::<f32>().unwrap_or(0.0);
}
