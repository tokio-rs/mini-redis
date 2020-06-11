use crate::client::Client;
use crate::cmd::{Command, Get, Set};
use crate::Result;
use bytes::Bytes;
use std::time::Duration;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot::{self, channel};
use tracing::error;

/// create a new connection Pool from a Client
pub fn create(client: Client) -> Pool {
    let (tx, rx) = unbounded_channel();
    tokio::spawn(async move { run(client, rx).await });

    Pool { tx }
}

/// await for commands send through the channel and forward them to client, then send the result back to the oneshot Receiver
async fn run(
    mut client: Client,
    mut rx: UnboundedReceiver<(Command, oneshot::Sender<Result<Option<Bytes>>>)>,
) {
    while let Some((cmd, tx)) = rx.recv().await {
        match cmd {
            Command::Get(get) => {
                let key = get.key();
                let result = client.get(&key).await;
                if let Err(_) = tx.send(result) {
                    error!("failed to send Client result, receiver has already been dropped");
                }
            }
            Command::Set(set) => {
                let key = set.key();
                let value = set.value();
                let expires = set.expire();
                let result = match expires {
                    None => client.set(&key, value).await,
                    Some(exp) => client.set_expires(&key, value, exp).await,
                };
                if let Err(_) = tx.send(result.map(|_| None)) {
                    error!("failed to send Client result, receiver has already been dropped");
                }
            }
            _ => unreachable!(),
        }
    }
}

pub struct Pool {
    tx: UnboundedSender<(Command, oneshot::Sender<Result<Option<Bytes>>>)>,
}

impl Pool {
    /// get a Connection like object to the mini-redis server instance
    pub fn get_connection(&self) -> Connection {
        Connection {
            tx: self.tx.clone(),
        }
    }
}

/// a Connection like object that proxies commands to the real connection
/// Commands are send trough mspc Channel, along with the requested Command a oneshot Sender is sent
/// the Result from the actual Client requested command is then sent through the oneshot Sender and Received on the Connection Receiver
pub struct Connection {
    tx: UnboundedSender<(Command, oneshot::Sender<Result<Option<Bytes>>>)>,
}

impl Connection {
    pub async fn get(&self, key: &str) -> Result<Option<Bytes>> {
        let get = Get::new(key);
        let (tx, rx) = channel();
        self.tx.send((Command::Get(get), tx))?;
        match rx.await {
            Ok(res) => res,
            Err(err) => Err(err.into()),
        }
    }

    pub async fn set(&self, key: &str, value: Bytes) -> Result<()> {
        let get = Set::new(key, value, None);
        let (tx, rx) = channel();
        self.tx.send((Command::Set(get), tx))?;
        match rx.await {
            Ok(res) => res.map(|_| ()),
            Err(err) => Err(err.into()),
        }
    }

    pub async fn set_expires(
        &self,
        key: &str,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        let get = Set::new(key, value, Some(expiration));
        let (tx, rx) = channel();
        self.tx.send((Command::Set(get), tx))?;
        match rx.await {
            Ok(res) => res.map(|_| ()),
            Err(err) => Err(err.into()),
        }
    }
}
