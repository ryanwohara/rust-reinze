use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;

/*
  {
    "examine": "Fabulously ancient mage protection enchanted in the 3rd Age.",
    "id": 10344,
    "members": true,
    "lowalch": 20200,
    "limit": 8,
    "value": 50500,
    "highalch": 30300,
    "icon": "3rd age amulet.png",
    "name": "3rd age amulet"
  },
*/

#[derive(Deserialize, Serialize, Debug)]
pub struct Mapping {
    pub id: u32,
    pub name: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Data {
    pub data: HashMap<u32, Price>,
}

/*
"2": {
  "high": 166,
  "highTime": 1671372935,
  "low": 162,
  "lowTime": 1671372944
},
*/
#[derive(Deserialize, Serialize, Debug)]
pub struct Price {
    #[serde(skip_serializing_if = "is_zero")]
    pub high: Option<u32>,
    #[serde(skip_serializing_if = "is_zero")]
    pub low: Option<u32>,
}

fn is_zero(field: &Option<u32>) -> bool {
    *field == None
}
