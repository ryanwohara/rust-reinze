extern crate ini;

use crate::common::capitalize;
use ini::Ini;

pub async fn params(skill: &str, param: &str) -> Result<String, ()> {
    let database = match Ini::load_from_file("lib/Database.ini") {
        Ok(database) => database,
        Err(e) => {
            println!("Error loading Database.ini: {}", e);
            return Err(());
        }
    };

    let section = match database.section(Some(capitalize(skill))) {
        Some(section) => section,
        None => {
            println!("Error getting section: {}", skill);
            return Err(());
        }
    };

    let underscored = param.replace(" ", "_");

    let mut output = format!("[{}]", capitalize(skill));

    let mut found_params: Vec<String> = vec![];

    for (k, v) in section.iter() {
        if k.to_ascii_lowercase()
            .contains(&underscored.to_ascii_lowercase())
        {
            found_params.push(format!("{}: {}xp", k.replace("_", " "), v.to_string()));
        }

        if found_params.len() >= 10 {
            break;
        }
    }

    if found_params.len() == 0 {
        return Err(());
    }

    for param in found_params.iter() {
        output = format!("{} {}", output, param);
    }

    Ok(output)
}
