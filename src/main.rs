mod commands;
mod error;
mod image;
mod launcher;
mod storage;
mod template;

#[macro_use]
extern crate clap;
use tokio;

#[tokio::main]
async fn main() {
    let c = commands::Commands {};
    if let Err(e) = c.evaluate() {
        println!("error: {}", e.to_string());
    }
}
