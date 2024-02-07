use anyhow::Result;
use clap::Parser;

use emu_cli::{helper::UnixServer, network::NetworkManagerType};

#[derive(Debug, Parser, Clone)]
#[command(author, version, about, long_about=None)]
pub struct Commands {
    /// User ID of process which can communicate with this helper
    pub uid: u32,
    /// Group ID of process which can communicate with this helper
    pub gid: u32,
    /// Name of backend to use when implementing network calls
    pub network: NetworkManagerType,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Commands::parse();
    let mut server = UnixServer::new(args.uid, args.gid, args.network).await?;
    server.listen().await;
    Ok(())
}
