use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args();
    let uid = args
        .nth(1)
        .expect("Expected a UID")
        .parse::<u32>()
        .expect("Expected a UID");

    let gid = args
        .next()
        .expect("Expected a GID")
        .parse::<u32>()
        .expect("Expected a GID");

    let mut server = emu_cli::helper::UnixServer::new(uid, gid).await?;
    server.listen().await;
    Ok(())
}
