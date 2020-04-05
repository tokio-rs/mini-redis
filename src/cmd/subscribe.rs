use crate::cmd::{Parse, ParseError};
use crate::{Command, Connection, Db, Frame, Shutdown};

use bytes::Bytes;
use tokio::select;
use tokio::stream::{StreamExt, StreamMap};

#[derive(Debug)]
pub struct Subscribe {
    pub(crate) channels: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Unsubscribe {
    pub(crate) channels: Vec<String>,
}

impl Subscribe {
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Subscribe, ParseError> {
        use ParseError::EndOfStream;

        // There must be at least one channel
        let mut channels = vec![parse.next_string()?];

        loop {
            match parse.next_string() {
                Ok(s) => channels.push(s),
                Err(EndOfStream) => break,
                Err(err) => return Err(err),
            }
        }

        Ok(Subscribe { channels })
    }

    /// Implements the "subscribe" half of Redis' Pub/Sub feature documented
    /// [here].
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
                    let mut response = Frame::array();
                    response.push_bulk(Bytes::from_static(b"message"));
                    response.push_bulk(Bytes::copy_from_slice(channel.as_bytes()));
                    // TODO: handle lag error
                    response.push_bulk(msg.unwrap());

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

                                dst.write_frame(&response).await?;
                            }
                        }
                        _ => {
                            // TODO: received invalid command
                            unimplemented!();
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
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Unsubscribe, ParseError> {
        use ParseError::EndOfStream;

        // There may be no channels listed.
        let mut channels = vec![];

        loop {
            match parse.next_string() {
                Ok(s) => channels.push(s),
                Err(EndOfStream) => break,
                Err(err) => return Err(err),
            }
        }

        Ok(Unsubscribe { channels })
    }

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("unsubscribe".as_bytes()));
        for channel in self.channels {
            frame.push_bulk(Bytes::from(channel.into_bytes()));
        }
        frame
    }
}
