use crate::common::c1;
use crate::common::c2;
use crate::common::c3;
use crate::common::c4;
use crate::common::c5;
use crate::common::l;
use crate::common::p;
use crate::runescape::items::Ge;
use crate::runescape::items::GeItemPrice;
use crate::runescape::items::Mapping;
use crate::runescape::items::StrOrNum;
use regex::Regex;
use serde_json;
use std::fs::read_to_string;

// Scan lib/item_db.json for up to 10 items that match the query
// and hit the OSRS Grand Exchange API to get the price of each item
pub async fn ge(query: &str) -> Result<String, ()> {
    let mapping_filename = "lib/item_db.json";

    let mut output = l("Grand Exchange");
    let mut found_items: Vec<String> = vec![];

    let mapping_file_contents = match read_to_string(mapping_filename) {
        Ok(file) => file,
        Err(e) => {
            println!("Error opening item_db.json: {}", e);
            return Err(());
        }
    };

    let mapping_json = match serde_json::from_str::<Vec<Mapping>>(&mapping_file_contents) {
        Ok(json) => json,
        Err(e) => {
            println!("Error parsing item_db.json into JSON: {}", e);
            return Err(());
        }
    };

    for item in mapping_json.iter() {
        let regex_string = format!(r"(?i){}", query);
        let re = match Regex::new(&regex_string) {
            Ok(re) => re,
            Err(e) => {
                println!("Error creating regex: {}", e);
                return Err(());
            }
        };

        let matched = re.captures(&item.name);
        if matched.is_some() {
            // Hit the OSRS API with &item.id
            let url = format!(
                "http://services.runescape.com/m=itemdb_oldschool/api/catalogue/detail.json?item={}",
                &item.id
            );

            let response = match reqwest::get(&url).await {
                Ok(response) => response,
                Err(e) => {
                    println!("Error getting response from OSRS API: {}", e);
                    return Err(());
                }
            };

            let j: String = match response.text().await {
                Ok(j) => j,
                Err(e) => {
                    println!("Error parsing response from OSRS API into JSON: {}", e);
                    return Err(());
                }
            };

            let json: Ge = match serde_json::from_str(&j) {
                Ok(json) => json,
                Err(e) => {
                    println!("Error parsing response from OSRS API into JSON: {}", e);
                    return Err(());
                }
            };

            let ge_json = &json.item;

            found_items.push(format!(
                "{}: {}{} {}",
                c1(&item.name),
                c2(&str_from_enum(ge_json.current.price)),
                c1("gp"),
                price_change(&ge_json.today)
            ));
        }

        if found_items.len() >= 6 {
            break;
        }
    }

    if found_items.len() == 0 {
        return Err(());
    }

    output = format!("{} {}", output, found_items.join(&c1(" | ")));

    Ok(output)
}

fn price_change(today: &GeItemPrice) -> String {
    let mut output = String::new();

    if today.trend == "neutral" {
        output = format!("{}{}", c5(&str_from_enum(today.price)), c5("▬"));
    } else if today.trend == "positive" {
        output = format!("{}{}", c4(&str_from_enum(today.price)), c4("▲"));
    } else if today.trend == "negative" {
        output = format!(
            "{}{}",
            c3(&str_from_enum(today.price).replace(" ", "")),
            c3("▼")
        );
    }

    p(&output)
}

fn str_from_enum(price: StrOrNum) -> String {
    match price {
        StrOrNum::Str(s) => s.to_string(),
        StrOrNum::Num(n) => n.to_string(),
    }
}
