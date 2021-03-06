mod commands;
mod config;
mod error;
mod image;
mod ini_writer;
mod launcher;
mod network;
mod qmp;
mod storage;
mod template;

#[macro_use]
extern crate clap;
use tokio;

#[tokio::main]
async fn main() {
    let c = commands::Commands {};
    if let Err(e) = c.evaluate().await {
        println!("error: {}", e.to_string());
    }
}
