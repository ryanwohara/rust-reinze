pub mod players;

pub async fn players() -> Result<String, ()> {
    return players::players().await;
}
