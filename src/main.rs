mod application;
mod common;
mod plugins;

extern crate chrono;
extern crate reqwest;
extern crate select;

use tokio;

#[tokio::main]
async fn main() {
    application::run().await
}
