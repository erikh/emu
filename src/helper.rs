use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs::Permissions,
    os::unix::fs::{chown, PermissionsExt},
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    io::AsyncWriteExt,
    net::{UnixListener, UnixStream},
    sync::Mutex,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HelperMessage {
    Request(HelperRequest),
    Response(HelperResponse),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HelperRequest {
    Ping,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum HelperResponse {
    Pong,
}

fn extract_message(message: &[u8]) -> Option<(usize, HelperMessage)> {
    let mut half = false;

    for (x, c) in message.iter().enumerate() {
        // messages are terminated by two newlines in succession. If parsing fails after detecting
        // this, assume it was a part of inner content and continue to the next one.
        if *c == b'\n' {
            if half {
                match serde_json::from_slice(&message[..x]) {
                    Ok(res) => {
                        return Some((x, res));
                    }
                    Err(_) => half = false,
                }
            } else {
                half = true;
            }
        } else if half {
            half = false;
        }
    }

    None
}

fn socket_filename(uid: u16) -> PathBuf {
    PathBuf::from(format!("/tmp/emu-{}.sock", uid))
}

async fn handle_stream(
    stream: Arc<Mutex<UnixStream>>,
    mut f: impl FnMut(HelperMessage) -> Result<Option<HelperMessage>>,
) {
    let mut buf = [0u8; 4096];
    let mut message = Vec::with_capacity(4096);

    while let Ok(size) = stream.lock().await.try_read(&mut buf) {
        if size > 0 {
            message.append(&mut buf[..size].to_vec());

            while let Some((pos, msg)) = extract_message(&message) {
                message = message.iter().skip(pos).copied().collect::<Vec<u8>>();
                match f(msg) {
                    Ok(Some(response)) => {
                        if send_message(stream.clone(), response).await.is_err() {
                            return;
                        }
                    }
                    Ok(None) => {}
                    Err(_) => return,
                }
            }
        }

        tokio::time::sleep(std::time::Duration::new(0, 500)).await;
    }
}

async fn send_message(stream: Arc<Mutex<UnixStream>>, message: HelperMessage) -> Result<()> {
    let mut lock = stream.lock().await;
    lock.write_all(&serde_json::to_vec(&message)?).await?;
    Ok(lock.write_all(vec![b'\n', b'\n'].as_slice()).await?)
}

#[allow(dead_code)]
pub struct UnixClient {
    stream: Arc<Mutex<UnixStream>>,
}

#[allow(dead_code)]
impl UnixClient {
    pub async fn new(uid: u16) -> Result<Self> {
        let stream = Arc::new(Mutex::new(UnixStream::connect(socket_filename(uid)).await?));
        let s = stream.clone();
        tokio::spawn(async move { handle_stream(s, Self::process_message).await });
        Ok(Self { stream })
    }

    pub fn process_message(_message: HelperMessage) -> Result<Option<HelperMessage>> {
        Ok(None)
    }
}

pub struct UnixServer {
    listener: UnixListener,
}

impl UnixServer {
    pub async fn new(uid: u16, gid: u16) -> Result<Self> {
        let filename = socket_filename(uid);
        let obj = Self {
            listener: UnixListener::bind(filename.clone())?,
        };

        std::fs::set_permissions(filename.clone(), Permissions::from_mode(0o0660))?;
        chown(filename, Some(uid.into()), Some(gid.into()))?;
        Ok(obj)
    }

    pub fn process_message(_message: HelperMessage) -> Result<Option<HelperMessage>> {
        Ok(None)
    }

    pub async fn listen(&mut self) {
        while let Ok((stream, _)) = self.listener.accept().await {
            tokio::spawn(async move {
                handle_stream(Arc::new(Mutex::new(stream)), Self::process_message).await
            });
        }
    }
}
