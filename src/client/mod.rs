use crate::Connection;

use bytes::Bytes;
use std::io;
use tokio::net::{TcpStream, ToSocketAddrs};

/// Mini asynchronous Redis client
pub struct Client {
    conn: Connection,
}

pub async fn connect<T: ToSocketAddrs>(addr: T) -> io::Result<Client> {
    let socket = TcpStream::connect(addr).await?;
    let conn = Connection::new(socket);

    Ok(Client { conn })
}

impl Client {
    pub async fn get(&mut self, key: &str) -> io::Result<Option<Bytes>> {
        unimplemented!();
    }

    pub async fn set(&mut self, key: &str, val: Bytes) -> io::Result<()> {
        unimplemented!();
    }
}
