use crate::network::NetworkManagerType;
use anyhow::{anyhow, Result};
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
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
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
                        return Some((x + 1, res));
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

fn socket_filename(uid: u32) -> PathBuf {
    PathBuf::from(format!("/tmp/emu-{}.sock", uid))
}

async fn handle_stream<T>(stream: Arc<Mutex<UnixStream>>, f: impl Fn(HelperMessage) -> T)
where
    T: std::future::Future<Output = Result<Option<HelperMessage>>>,
{
    let mut buf = [0u8; 4096];
    let mut message = Vec::with_capacity(4096);

    loop {
        let lock = stream.lock().await;
        let res = lock.try_read(&mut buf);
        drop(lock);
        match res {
            Ok(size) => {
                if size > 0 {
                    message.append(&mut buf[..size].to_vec());
                    while let Some((pos, msg)) = extract_message(&message) {
                        message = message.iter().skip(pos).copied().collect::<Vec<u8>>();
                        match f(msg).await {
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
            }
            Err(_) => {}
        }

        tokio::time::sleep(std::time::Duration::new(0, 500)).await;
    }
}

async fn send_message(stream: Arc<Mutex<UnixStream>>, message: HelperMessage) -> Result<()> {
    let mut lock = stream.lock().await;
    lock.write_all(&serde_json::to_vec(&message)?).await?;
    lock.write_all(vec![b'\n', b'\n'].as_slice()).await?;
    Ok(lock.flush().await?)
}

#[derive(Debug, Clone)]
pub struct UnixClient {
    stream: Arc<Mutex<UnixStream>>,
    replies: Arc<Mutex<UnboundedReceiver<HelperMessage>>>,
}

impl UnixClient {
    pub async fn new(uid: u32) -> Result<Self> {
        let stream = match UnixStream::connect(socket_filename(uid)).await {
            Ok(stream) => Arc::new(Mutex::new(stream)),
            Err(e) => return Err(anyhow!("Couldn't connect to socket: {}", e)),
        };

        let sclone = stream.clone();

        let (s, r) = unbounded_channel();

        tokio::spawn(async move {
            handle_stream(sclone, |msg| Self::process_message(s.clone(), msg)).await
        });

        Ok(Self {
            stream,
            replies: Arc::new(Mutex::new(r)),
        })
    }

    async fn process_message(
        sender: UnboundedSender<HelperMessage>,
        message: HelperMessage,
    ) -> Result<Option<HelperMessage>> {
        match message {
            HelperMessage::Response(_) => {
                sender.send(message)?;
                Ok(None)
            }
            HelperMessage::Request(_) => Err(anyhow!("got out-of-order response")),
        }
    }

    pub async fn ping(&self) -> Result<()> {
        send_message(
            self.stream.clone(),
            HelperMessage::Request(HelperRequest::Ping),
        )
        .await?;

        match self.replies.lock().await.recv().await {
            Some(_) => Ok(()),
            None => Err(anyhow!("No response")),
        }
    }
}

pub struct UnixServer {
    listener: UnixListener,
}

impl UnixServer {
    pub async fn new(uid: u32, gid: u32, _network: NetworkManagerType) -> Result<Self> {
        let filename = socket_filename(uid);
        let _ = std::fs::remove_file(filename.clone());
        let obj = Self {
            listener: UnixListener::bind(filename.clone())?,
        };

        std::fs::set_permissions(filename.clone(), Permissions::from_mode(0o0660))?;
        chown(filename, Some(uid.into()), Some(gid.into()))?;
        Ok(obj)
    }

    async fn process_message(message: HelperMessage) -> Result<Option<HelperMessage>> {
        match message {
            HelperMessage::Request(req) => match req {
                HelperRequest::Ping => Ok(Some(HelperMessage::Response(HelperResponse::Pong))),
            },
            HelperMessage::Response(_) => Err(anyhow!("got out-of-order response")),
        }
    }

    pub async fn listen(&mut self) {
        while let Ok((stream, _)) = self.listener.accept().await {
            tokio::spawn(async move {
                handle_stream(Arc::new(Mutex::new(stream)), Self::process_message).await
            });
        }
    }
}
