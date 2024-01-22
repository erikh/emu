#[tokio::main]
async fn main() {
    if let Err(e) = emu_cli::v2::evaluate().await {
        println!("error: {}", e.to_string());
    }
}
