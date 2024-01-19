mod commands;
mod config;
mod image;
mod launcher;
mod network;
mod qmp;
mod storage;
mod template;

#[tokio::main]
async fn main() {
    if let Err(e) = commands::Commands::evaluate().await {
        println!("error: {}", e.to_string());
    }
}
