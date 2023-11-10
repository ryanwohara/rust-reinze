mod application;
mod plugins;

extern crate chrono;
extern crate common;
extern crate reqwest;
extern crate select;

use tokio;

#[tokio::main]
async fn main() {
    application::run().await
}
