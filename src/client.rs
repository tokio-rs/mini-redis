use crate::cmd::{Get, Publish, Set, Subscribe, Unsubscribe};
use crate::{Connection, Frame};

use bytes::Bytes;
use std::io::{Error, ErrorKind};
use std::iter::FromIterator;
use std::collections::HashSet;
use std::time::Duration;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::stream::Stream;
use tracing::{debug, instrument};
use async_stream::stream;

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
        })
        .await
    }

    /// publish `message` on the `channel`
    #[instrument(skip(self))]
    pub async fn publish(&mut self, channel: &str, message: Bytes) -> crate::Result<u64> {
        self.publish_cmd(Publish {
            channel: channel.to_string(),
            message: message,
        })
        .await
    }

    /// subscribe to the list of channels
    /// when client sends a `SUBSCRIBE` command, server's handle for client enters a mode where only
    /// `SUBSCRIBE` and `UNSUBSCRIBE` commands are allowed, so we consume client and return Subscribe type
    /// which only allows `SUBSCRIBE` and `UNSUBSCRIBE` commands
    #[instrument(skip(self))]
    pub async fn subscribe(mut self, channels: Vec<String>) -> crate::Result<Subscriber> {
        let channels = self.subscribe_cmd(Subscribe { channels: channels }).await?;
        let subscribed_channels = HashSet::from_iter(channels);

        Ok(Subscriber {
            conn: self.conn,
            subscribed_channels,
        })
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
        })
        .await
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

    async fn publish_cmd(&mut self, cmd: Publish) -> crate::Result<u64> {
        // Convert the `Publish` command into a frame
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket
        self.conn.write_frame(&frame).await?;

        // Read the response
        match self.read_response().await? {
            Frame::Integer(response) => Ok(response),
            frame => Err(frame.to_error()),
        }
    }

    async fn subscribe_cmd(&mut self, cmd: Subscribe) -> crate::Result<Vec<String>> {
        // Convert the `Subscribe` command into a frame
        let channels = cmd.channels.clone();
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket
        self.conn.write_frame(&frame).await?;

        // Read the response
        for channel in &channels {
            let response = self.read_response().await?;
            match response {
                Frame::Array(ref frame) => match frame.as_slice() {
                    [subscribe, schannel]
                        if subscribe.to_string() == "subscribe"
                            && &schannel.to_string() == channel =>
                    {
                        ()
                    }
                    _ => return Err(response.to_error()),
                },
                frame => return Err(frame.to_error()),
            };
        }

        Ok(channels)
    }

    /// Reads a response frame from the socket. If an `Error` frame is read, it
    /// is converted to `Err`.
    async fn read_response(&mut self) -> crate::Result<Frame> {
        let response = self.conn.read_frame().await?;

        debug!(?response);

        match response {
            Some(Frame::Error(msg)) => Err(msg.into()),
            Some(frame) => Ok(frame),
            None => {
                // Receiving `None` here indicates the server has closed the
                // connection without sending a frame. This is unexpected and is
                // represented as a "connection reset by peer" error.
                let err = Error::new(ErrorKind::ConnectionReset, "connection reset by server");

                Err(err.into())
            }
        }
    }
}

pub struct Subscriber {
    conn: Connection,
    subscribed_channels: HashSet<String>,
}

impl Subscriber {

    /// await for next message published on the subscribed channels
    pub async fn next_message(&mut self) -> crate::Result<Message> {
        match self.receive_message().await {
            Some(message) => message,
            None => {
                // Receiving `None` here indicates the server has closed the
                // connection without sending a frame. This is unexpected and is
                // represented as a "connection reset by peer" error.
                let err = Error::new(ErrorKind::ConnectionReset, "connection reset by server");

                Err(err.into())
            }
        }
    }

    /// Convert the subscriber into a Stream
    /// yielding new messages published on subscribed channels
    pub fn into_stream(mut self) -> impl Stream<Item = crate::Result<Message>> {
        stream! {
            while let Some(message) = self.receive_message().await {
                yield message;
            }
        }
    }

    /// Subscribe to a list of new channels
    #[instrument(skip(self))]
    pub async fn subscribe(&mut self, channels: Vec<String>) -> crate::Result<()> {
        let cmd = Subscribe { channels: channels };

        let channels = cmd.channels.clone();
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket
        self.conn.write_frame(&frame).await?;

        // Read the response
        for channel in &channels {
            let response = self.read_response().await?;
            match response {
                Frame::Array(ref frame) => match frame.as_slice() {
                    [subscribe, schannel]
                        if &subscribe.to_string() == "subscribe"
                            && &schannel.to_string() == channel =>
                    {
                        ()
                    }
                    _ => return Err(response.to_error()),
                },
                frame => return Err(frame.to_error()),
            };
        }

        self.subscribed_channels.extend(channels);

        Ok(())
    }

    /// Unsubscribe to a list of new channels
    #[instrument(skip(self))]
    pub async fn unsubscribe(&mut self, channels: Vec<String>) -> crate::Result<()> {
        let cmd = Unsubscribe { channels: channels };

        let mut channels = cmd.channels.clone();
        let frame = cmd.into_frame();

        debug!(request = ?frame);

        // Write the frame to the socket
        self.conn.write_frame(&frame).await?;

        // if the input channel list is empty, server acknowledges as unsubscribing
        // from all subscribed channels, so we assert that the unsubscribe list received
        // matches the client subscribed one
        if channels.is_empty() {
            channels = Vec::from_iter(self.subscribed_channels.clone());
        }

        // Read the response
        for _channel in &channels {
            let response = self.read_response().await?;
            match response {
                Frame::Array(ref frame) => match frame.as_slice() {
                    [unsubscribe, uchannel] if &unsubscribe.to_string() == "unsubscribe" => {
                        //unsubscribed channel should exist in the subscribed list at this point
                        if self.subscribed_channels.remove(&uchannel.to_string()) == false {
                            return Err(response.to_error());
                        }
                    },
                    _ => return Err(response.to_error()),
                },
                frame => return Err(frame.to_error()),
            };
        }

        Ok(())
    }

    /// Receives a frame published from server on socket and convert it to a `Message`
    /// if frame is not `Frame::Array` with proper message structure return Err
    async fn receive_message(&mut self) -> Option<crate::Result<Message>> {
        match self.conn.read_frame().await {
            Ok(None) => None,
            Err(err) => Some(Err(err.into())),
            Ok(Some(mframe)) => {
                debug!(?mframe);
                match mframe {
                    Frame::Array(ref frame) => match frame.as_slice() {
                        [message, channel, content] if &message.to_string() == "message" => {
                            Some(Ok(Message {
                                channel: channel.to_string(),
                                content: Bytes::from(content.to_string()),
                            }))
                        }
                        _ => Some(Err(mframe.to_error())),
                    },
                    frame => Some(Err(frame.to_error())),
                }
            }
        }
    }

    /// Reads a response frame to a command from the socket. If an `Error` frame is read, it
    /// is converted to `Err`.
    async fn read_response(&mut self) -> crate::Result<Frame> {
        let response = self.conn.read_frame().await?;

        debug!(?response);

        match response {
            Some(Frame::Error(msg)) => Err(msg.into()),
            Some(frame) => Ok(frame),
            None => {
                // Receiving `None` here indicates the server has closed the
                // connection without sending a frame. This is unexpected and is
                // represented as a "connection reset by peer" error.
                let err = Error::new(ErrorKind::ConnectionReset, "connection reset by server");

                Err(err.into())
            }
        }
    }
}

/// A message received on a subscribed channel
#[derive(Debug, Clone)]
pub struct Message {
    pub channel: String,
    pub content: Bytes,
}
