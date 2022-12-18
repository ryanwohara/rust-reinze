extern crate ini;

use crate::common::c1;
use crate::common::c2;
use crate::common::capitalize;
use crate::common::l;
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

    let mut output = l(&capitalize(skill)).to_string();

    let mut found_params: Vec<String> = vec![];

    for (k, v) in section.iter() {
        if k.to_ascii_lowercase()
            .contains(&underscored.to_ascii_lowercase())
        {
            found_params.push(format!(
                "{}: {}",
                c1(&k.replace("_", " ")),
                c2(&format!("{}xp", v.to_string()))
            ));
        }

        if found_params.len() >= 10 {
            break;
        }
    }

    if found_params.len() == 0 {
        return Err(());
    }

    output = format!("{} {}", output, found_params.join(&c1(" | ")));

    Ok(output)
}
