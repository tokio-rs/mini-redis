use crate::{
    cmd::{
        utils::{bytes_from_str, duration_from_ms_str},
        Set,
    },
    Command, Connection,
};

use bytes::Bytes;
use tokio::net::{TcpStream, ToSocketAddrs};

/// Mini asynchronous Redis client
pub struct Client {
    conn: Connection,
}

pub async fn connect<T: ToSocketAddrs>(addr: T) -> Result<Client, Box<dyn std::error::Error>> {
    let socket = TcpStream::connect(addr).await?;
    let conn = Connection::new(socket);

    Ok(Client { conn })
}

impl Client {
    pub async fn get(&mut self, key: &str) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
        unimplemented!();
    }

    pub async fn set(&mut self, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
        let opts = Set {
            key: key.to_string(),
            value: bytes_from_str(value),
            expire: None,
        };
        self.set_with_opts(opts).await
    }

    pub async fn set_with_expiration(
        &mut self,
        key: &str,
        value: &str,
        expiration: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let opts = Set {
            key: key.to_string(),
            value: bytes_from_str(value),
            expire: Some(duration_from_ms_str(expiration)?),
        };
        self.set_with_opts(opts).await
    }

    pub async fn set_with_opts(&mut self, opts: Set) -> Result<(), Box<dyn std::error::Error>> {
        let frame = Command::Set(opts).into_frame()?;
        Ok(self.conn.write_frame(&frame).await?)
    }
}
