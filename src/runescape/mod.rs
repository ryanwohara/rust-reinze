pub mod params;
pub mod players;

pub async fn players() -> Result<String, ()> {
    players::players().await
}

pub async fn params(skill: &str, param: &str) -> Result<String, ()> {
    params::params(skill, param).await
}
