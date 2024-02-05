use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let uid = std::env::args()
        .nth(1)
        .expect("Expected a UID")
        .parse::<u16>()
        .expect("Expected a UID");

    let gid = std::env::args()
        .next()
        .expect("Expected a GID")
        .parse::<u16>()
        .expect("Expected a GID");

    let mut server = emu_cli::helper::UnixServer::new(uid, gid).await?;
    server.listen().await;
    Ok(())
}
