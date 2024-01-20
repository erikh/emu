#[tokio::main]
async fn main() {
    if let Err(e) = emu_cli::commands::Commands::evaluate().await {
        println!("error: {}", e.to_string());
    }
}
