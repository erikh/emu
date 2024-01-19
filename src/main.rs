mod commands;
mod config;
mod image;
mod ini_writer;
mod launcher;
mod network;
mod qmp;
mod storage;
mod template;

use tokio;

#[tokio::main]
async fn main() {
    if let Err(e) = commands::Commands::evaluate() {
        println!("error: {}", e.to_string());
    }
}
