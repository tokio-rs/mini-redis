use crate::{Connection, Frame};
use crate::cmd::{Get, Set};

use bytes::Bytes;
use std::io::{Error, ErrorKind};
use std::time::Duration;
use tokio::net::{TcpStream, ToSocketAddrs};
use tracing::{debug, instrument};

/// Mini asynchronous Redis client
pub struct Client {
    conn: Connection,
}

pub async fn connect<T: ToSocketAddrs>(addr: T) -> crate::Result<Client> {
    let socket = TcpStream::connect(addr).await?;
    let conn = Connection::new(socket);

    Ok(Client { conn })
}

impl Client {
    /// Get the value of a key
    #[instrument(skip(self))]
    pub async fn get(&mut self, key: &str) -> crate::Result<Option<Bytes>> {
        // Create a `Get` command for the `key` and convert it to a frame.
        let frame = Get::new(key).into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket.
        self.conn.write_frame(&frame).await?;

        // Wait for the response.
        match self.read_response().await? {
            Frame::Simple(value) => Ok(Some(value.into())),
            Frame::Bulk(value) => Ok(Some(value)),
            Frame::Null => Ok(None),
            frame => Err(frame.to_error()),
        }
    }

    /// Set the value of a key to `value`.
    #[instrument(skip(self))]
    pub async fn set(&mut self, key: &str, value: Bytes) -> crate::Result<()> {
        self.set_cmd(Set {
            key: key.to_string(),
            value: value,
            expire: None,
        }).await
    }

    /// Set the value of a key to `value`. The value expires after `expiration`.
    #[instrument(skip(self))]
    pub async fn set_expires(
        &mut self,
        key: &str,
        value: Bytes,
        expiration: Duration,
    ) -> crate::Result<()> {
        self.set_cmd(Set {
            key: key.to_string(),
            value: value.into(),
            expire: Some(expiration),
        }).await
    }

    async fn set_cmd(&mut self, cmd: Set) -> crate::Result<()> {
        // Convert the `Set` command into a frame
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket
        self.conn.write_frame(&frame).await?;

        // Read the response
        match self.read_response().await? {
            Frame::Simple(response) if response == "OK" => Ok(()),
            frame => Err(frame.to_error()),
        }
    }

    /// Reads a response frame from the socket. If an `Error` frame is read, it
    /// is converted to `Err`.
    async fn read_response(&mut self) -> crate::Result<Frame> {
        let response = self.conn.read_frame().await?;

        debug!(response = ?response);

        match response {
            Some(Frame::Error(msg)) => {
                Err(msg.into())
            }
            Some(frame) => Ok(frame),
            None => {
                // Receiving `None` here indicates the server has closed the
                // connection without sending a frame. This is unexpected and is
                // represented as a "connection reset by peer" error.
                let err = Error::new(
                    ErrorKind::ConnectionReset,
                    "connection reset by server");

                Err(err.into())
            }
        }
    }
}
