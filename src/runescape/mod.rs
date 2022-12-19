pub mod ge;
mod items;
pub mod params;
pub mod players;
pub mod prices;

pub async fn players() -> Result<String, ()> {
    players::players().await
}

pub async fn params(skill: &str, param: &str) -> Result<String, ()> {
    params::params(skill, param).await
}

pub async fn prices(query: &str) -> Result<String, ()> {
    prices::prices(query).await
}

pub async fn ge(query: &str) -> Result<String, ()> {
    ge::ge(query).await
}
