#[tokio::main]
async fn main() {
    if let Err(e) = emu_cli::evaluate().await {
        println!("error: {}", e);
    }
}
