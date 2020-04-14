use crate::cmd::{Parse, ParseError, Unknown};
use crate::{Command, Connection, Db, Frame, Shutdown};

use bytes::Bytes;
use tokio::select;
use tokio::stream::{StreamExt, StreamMap};

/// Subscribes the client to one or more channels.
///
/// Once the client enters the subscribed state it is not supposed to issue any
/// other commands, except for additional SUBSCRIBE, PSUBSCRIBE, UNSUBSCRIBE,
/// PUNSUBSCRIBE, PING and QUIT commands.
#[derive(Debug)]
pub struct Subscribe {
    channels: Vec<String>,
}

/// Unsubscribes the client from one or more channels.
///
/// When no channels are specified, the client is unsubscribed from all the
/// previously subscribed channels. In this case, a message for every
/// unsubscribed channel will be sent to the client.
#[derive(Clone, Debug)]
pub struct Unsubscribe {
    channels: Vec<String>,
}

impl Subscribe {
    /// Creates a new `Subscribe` command to listen on the specified channels.
    pub(crate) fn new(channels: &[String]) -> Subscribe {
        Subscribe { channels: channels.to_vec() }
    }

    /// Parse a `Subscribe` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `SUBSCRIBE` string has already been consumed.
    ///
    /// # Returns
    ///
    /// On success, the `Subscribe` value is returned. If the frame is
    /// malformed, `Err` is returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing two or more entries.
    ///
    /// ```text
    /// SUBSCRIBE channel [channel ...]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Subscribe> {
        use ParseError::EndOfStream;

        // The `SUBSCRIBE` string has already been consumed. At this point,
        // there is one or more strings remaining in `parse`. These represent
        // the channels to subscribe to.
        //
        // Extract the first string. If there is none, the the frame is
        // malformed and the error is bubbled up.
        let mut channels = vec![parse.next_string()?];

        // Now, the remainder of the frame is consumed. Each value must be a
        // string or the frame is malformed. Once all values in the frame have
        // been consumed, the command is fully parsed.
        loop {
            match parse.next_string() {
                // A string has been consumed from the `parse`, push it into the
                // list of channels to subscribe to.
                Ok(s) => channels.push(s),
                // The `EndOfStream` error indicates there is no further data to
                // parse.
                Err(EndOfStream) => break,
                // All other errors are bubbled up, resulting in the connection
                // being terminated.
                Err(err) => return Err(err.into()),
            }
        }

        Ok(Subscribe { channels })
    }

    /// Apply the `Subscribe` command to the specified `Db` instance.
    ///
    /// This function is the entry point and includes the initial list of
    /// channels to subscribe to. Additional `subscribe` and `unsubscribe`
    /// commands may be received from the client and the list of subscriptions
    /// are updated accordingly.
    ///
    /// [here]: https://redis.io/topics/pubsub
    pub(crate) async fn apply(
        mut self,
        db: &Db,
        dst: &mut Connection,
        shutdown: &mut Shutdown,
    ) -> crate::Result<()> {
        // Each individual channel subscription is handled using a
        // `sync::broadcast` channel. Messages are then fanned out to all
        // clients currently subscribed to the channels.
        //
        // An individual client may subscribe to multiple channels and may
        // dynamically add and remove channels from its subscription set. To
        // handle this, a `StreamMap` is used to track active subscriptions. The
        // `StreamMap` merges messages from individual broadcast channels as
        // they are received.
        let mut subscriptions = StreamMap::new();

        loop {
            // `self.channels` is used to track additional channels to subscribe
            // to. When new `SUBSCRIBE` commands are received during the
            // execution of `apply`, the new channels are pushed onto this vec.
            for channel in self.channels.drain(..) {
                // Build response frame to respond to the client with.
                let mut response = Frame::array();
                response.push_bulk(Bytes::from_static(b"subscribe"));
                response.push_bulk(Bytes::copy_from_slice(channel.as_bytes()));
                response.push_int(subscriptions.len().saturating_add(1) as u64);

                // Subscribe to channel
                let rx = db.subscribe(channel.clone());

                // Track subscription in this client's subscription set.
                subscriptions.insert(channel, rx);

                // Respond with the successful subscription
                dst.write_frame(&response).await?;
            }

            // Wait for one of the following to happen:
            //
            // - Receive a message from one of the subscribed channels.
            // - Receive a subscribe or unsubscribe command from the client.
            // - A server shutdown signal.
            select! {
                // Receive messages from subscribed channels
                Some((channel, msg)) = subscriptions.next() => {
                    use tokio::sync::broadcast::RecvError;

                    let msg = match msg {
                        Ok(msg) => msg,
                        Err(RecvError::Lagged(_)) => continue,
                        Err(RecvError::Closed) => unreachable!(),
                    };

                    let mut response = Frame::array();
                    response.push_bulk(Bytes::from_static(b"message"));
                    response.push_bulk(Bytes::copy_from_slice(channel.as_bytes()));
                    response.push_bulk(msg);

                    dst.write_frame(&response).await?;
                }
                res = dst.read_frame() => {
                    let frame = match res? {
                        Some(frame) => frame,
                        // How to handle remote client closing write half?
                        None => return Ok(())
                    };

                    // A command has been received from the client.
                    //
                    // Only `SUBSCRIBE` and `UNSUBSCRIBE` commands are permitted
                    // in this context.
                    match Command::from_frame(frame)? {
                        Command::Subscribe(subscribe) => {
                            // Subscribe to the channels on next iteration
                            self.channels.extend(subscribe.channels.into_iter());
                        }
                        Command::Unsubscribe(mut unsubscribe) => {
                            // If no channels are specified, this requests
                            // unsubscribing from **all** channels. To implement
                            // this, the `unsubscribe.channels` vec is populated
                            // with the list of channels currently subscribed
                            // to.
                            if unsubscribe.channels.is_empty() {
                                unsubscribe.channels = subscriptions
                                    .keys()
                                    .map(|channel| channel.to_string())
                                    .collect();
                            }

                            for channel in unsubscribe.channels.drain(..) {
                                subscriptions.remove(&channel);

                                let mut response = Frame::array();
                                response.push_bulk(Bytes::from_static(b"unsubscribe"));
                                response.push_bulk(Bytes::copy_from_slice(channel.as_bytes()));
                                response.push_int(subscriptions.len() as u64);

                                dst.write_frame(&response).await?;
                            }
                        }
                        command => {
                            let cmd = Unknown::new(command.get_name());
                            cmd.apply(dst).await?;
                        }
                    }
                }
                // Receive additional commands from the client
                _ = shutdown.recv() => {
                    return Ok(());
                }
            };
        }
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding a `Subscribe` command to send
    /// to the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("subscribe".as_bytes()));
        for channel in self.channels {
            frame.push_bulk(Bytes::from(channel.into_bytes()));
        }
        frame
    }
}

impl Unsubscribe {
    /// Create a new `Unsubscribe` command with the given `channels`.
    pub(crate) fn new(channels: &[String]) -> Unsubscribe {
        Unsubscribe { channels: channels.to_vec() }
    }

    /// Parse a `Unsubscribe` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `UNSUBSCRIBE` string has already been consumed.
    ///
    /// # Returns
    ///
    /// On success, the `Unsubscribe` value is returned. If the frame is
    /// malformed, `Err` is returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing at least one entry.
    ///
    /// ```text
    /// UNSUBSCRIBE [channel [channel ...]]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Unsubscribe, ParseError> {
        use ParseError::EndOfStream;

        // There may be no channels listed, start with an empty vec.
        let mut channels = vec![];

        // Each entry in the frame must be a string or the frame is malformed.
        // Once all values in the frame have been consumed, the command is fully
        // parsed.
        loop {
            match parse.next_string() {
                // A string has been consumed from the `parse`, push it into the
                // list of channels to unsubscribe from.
                Ok(s) => channels.push(s),
                // The `EndOfStream` error indicates there is no further data to
                // parse.
                Err(EndOfStream) => break,
                // All other errors are bubbled up, resulting in the connection
                // being terminated.
                Err(err) => return Err(err),
            }
        }

        Ok(Unsubscribe { channels })
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding an `Unsubscribe` command to
    /// send to the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("unsubscribe".as_bytes()));

        for channel in self.channels {
            frame.push_bulk(Bytes::from(channel.into_bytes()));
        }

        frame
    }
}
