mod commands;
mod error;
mod image;
mod launcher;
mod storage;
mod template;

use error::Error;

#[macro_use]
extern crate clap;

fn main() -> Result<(), Error> {
    let c = commands::Commands {};
    if let Err(e) = c.evaluate() {
        println!("error: {}", e.to_string());
    }
    return Ok(());
}
