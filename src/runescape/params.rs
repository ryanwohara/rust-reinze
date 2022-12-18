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

    let xp = match section.get(underscored) {
        Some(xp) => xp,
        None => {
            println!("Error getting param: {}", param);
            return Err(());
        }
    };

    let output = format!("[{}] {}: {}xp", capitalize(skill), param, xp);

    Ok(output)
}
