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
    #[serde(skip_serializing_if = "is_none")]
    pub high: Option<u32>,
    #[serde(skip_serializing_if = "is_none")]
    pub low: Option<u32>,
}

fn is_none(field: &Option<u32>) -> bool {
    *field == None
}

/*
{
  "item": {
    "icon": "https://secure.runescape.com/m=itemdb_oldschool/1670930996399_obj_sprite.gif?id=1055",
    "icon_large": "https://secure.runescape.com/m=itemdb_oldschool/1670930996399_obj_big.gif?id=1055",
    "id": 1055,
    "type": "Default",
    "typeIcon": "https://www.runescape.com/img/categories/Default",
    "name": "Blue halloween mask",
    "description": "Aaaarrrghhh ... I'm a monster.",
    "current": {
      "trend": "neutral",
      "price": "10.2k"
    },
    "today": {
      "trend": "neutral",
      "price": 0
    },
    "members": "false",
    "day30": {
      "trend": "positive",
      "change": "+5.0%"
    },
    "day90": {
      "trend": "positive",
      "change": "+16.0%"
    },
    "day180": {
      "trend": "positive",
      "change": "+68.0%"
    }
  }
}
*/
#[derive(Deserialize, Serialize, Debug)]
pub struct Ge<'a> {
    #[serde(borrow)]
    pub item: GeItem<'a>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GeItem<'a> {
    pub icon: String,
    pub icon_large: String,
    pub id: u32,
    #[serde(rename = "type")]
    pub item_type: String,
    #[serde(rename = "typeIcon")]
    pub type_icon: String,
    pub name: String,
    pub description: String,
    #[serde(borrow)]
    pub current: GeItemPrice<'a>,
    #[serde(borrow)]
    pub today: GeItemPrice<'a>,
    pub members: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct GeItemPrice<'a> {
    pub trend: String,
    #[serde(borrow)]
    pub price: StrOrNum<'a>,
}

#[derive(Deserialize, Serialize, Debug, Copy, Clone)]
#[serde(untagged)]
pub enum StrOrNum<'a> {
    Str(&'a str),
    Num(u32),
}
// https://play.rust-lang.org/?version=stable&mode=debug&edition=2018&gist=939c50d5e1945dae3855bdf02e1e12bd
// https://stackoverflow.com/questions/56582722/serde-json-deserialize-any-number
