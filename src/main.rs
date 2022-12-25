mod application;
mod common;
mod plugins;

extern crate chrono;
extern crate reqwest;
extern crate select;

use anyhow::Result;
use tokio;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    match application::run().await {
        Ok(_) => return Ok(()),
        Err(e) => {
            println!("Error: {}", e);
            return Err(e);
        }
    }
}
