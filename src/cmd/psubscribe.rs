use crate::cmd::{Parse, ParseError};
use crate::{Command, Connection, Db, Frame, Shutdown};

use bytes::Bytes;
use std::pin::Pin;
use tokio::sync::broadcast;
use tokio_stream::{Stream, StreamExt, StreamMap};

/// Subscribes the client to one or more patterns.
///
/// Once the client enters the subscribed state, it is not supposed to issue any
/// other commands, except for additional SUBSCRIBE, PSUBSCRIBE, UNSUBSCRIBE,
/// PUNSUBSCRIBE, PING and QUIT commands.
#[derive(Debug)]
pub struct PSubscribe {
    patterns: Vec<String>,
}

/// Unsubscribes the client from one or more patterns.
///
/// When no patterns are specified, the client is unsubscribed from all the
/// previously subscribed patterns.
#[derive(Clone, Debug)]
pub struct PUnsubscribe {
    patterns: Vec<String>,
}

/// Stream of messages. The stream receives messages from the
/// `broadcast::Receiver`. We use `stream!` to create a `Stream` that consumes
/// messages. Because `stream!` values cannot be named, we box the stream using
/// a trait object.
type Messages = Pin<Box<dyn Stream<Item = Bytes> + Send>>;

impl PSubscribe {
    /// Create a new `PSubscribe` command which subscribes to the specified patterns.
    pub(crate) fn new(patterns: Vec<String>) -> PSubscribe {
        PSubscribe { patterns }
    }

    /// Parse a `PSubscribe` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `PSUBSCRIBE` string has already been consumed.
    ///
    /// # Returns
    ///
    /// On success, the `PSubscribe` value is returned. If the frame is malformed,
    /// `Err` is returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing one or more strings.
    ///
    /// ```text
    /// PSUBSCRIBE pattern1 [pattern2 ...]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<PSubscribe> {
        use ParseError::EndOfStream;

        // The `PSUBSCRIBE` string has already been consumed. At this point,
        // there is one or more strings remaining in `parse`. These represent
        // the patterns to subscribe to.
        //
        // Extract the first string. If there is none, the frame is
        // malformed and the error is bubbled up.
        let mut patterns = vec![parse.next_string()?];

        // Now, the remainder of the frame is consumed. Each value must be a
        // string or the frame is malformed. Once all values in the frame have
        // been consumed, the command is fully parsed.
        loop {
            match parse.next_string() {
                // A string has been consumed from the `parse`, push it into the
                // list of patterns to subscribe to.
                Ok(s) => patterns.push(s),
                // The `EndOfStream` error indicates there is no further data to
                // parse.
                Err(EndOfStream) => break,
                // All other errors are bubbled up, resulting in the connection
                // being terminated.
                Err(err) => return Err(err.into()),
            }
        }

        Ok(PSubscribe { patterns })
    }

    /// Apply the `PSubscribe` command to the specified `Db` instance.
    ///
    /// This function is the entry point and includes the initial list of
    /// patterns to subscribe to. Additional `psubscribe` and `punsubscribe`
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
        // Each individual pattern subscription is handled using a
        // `sync::broadcast` channel. Messages are then fanned out to all
        // clients currently subscribed to the patterns.
        //
        // An individual client may subscribe to multiple patterns and may
        // dynamically add and remove patterns from its subscription set. To
        // handle this, a `StreamMap` is used to track active subscriptions. The
        // `StreamMap` merges messages from individual broadcast channels as
        // they are received.
        let mut subscriptions = StreamMap::new();

        loop {
            // `self.patterns` is used to track additional patterns to subscribe
            // to. When new `PSUBSCRIBE` commands are received during the
            // execution of `apply`, the new patterns are pushed onto this vec.
            for pattern_name in self.patterns.drain(..) {
                subscribe_to_pattern(pattern_name, &mut subscriptions, db, dst).await?;
            }

            // Wait for one of the following to happen:
            //
            // - Receive a message from one of the subscribed patterns.
            // - Receive a psubscribe or punsubscribe command from the client.
            // - A server shutdown signal.
            tokio::select! {
                // Receive a message from one of the subscribed patterns
                result = subscriptions.next() => {
                    let (pattern, msg) = result.unwrap();
                    dst.write_frame(&make_message_frame(pattern, msg)).await?;
                }
                // Wait for a command from the client
                cmd = dst.read_frame() => {
                    let frame = cmd?.ok_or("Connection closed")?;
                    let cmd = Command::from_frame(frame)?;

                    match cmd {
                        Command::PSubscribe(cmd) => {
                            // The client wants to subscribe to more patterns
                            self.patterns.extend(cmd.patterns);
                        }
                        Command::PUnsubscribe(cmd) => {
                            // The client wants to unsubscribe from patterns
                            for pattern in cmd.patterns {
                                subscriptions.remove(&pattern);
                            }
                        }
                        Command::Ping(cmd) => {
                            // Respond to PING
                            cmd.apply(dst).await?;
                        }
                        Command::Quit(_) => {
                            // Client wants to quit
                            return Ok(());
                        }
                        _ => {
                            // Any other command is an error
                            let response = Frame::Error(
                                "only (P)SUBSCRIBE / (P)UNSUBSCRIBE / PING / QUIT allowed in this context".into()
                            );
                            dst.write_frame(&response).await?;
                        }
                    }
                }
                // Wait for the shutdown signal
                _ = shutdown.recv() => {
                    return Ok(());
                }
            }
        }
    }
}

impl PUnsubscribe {
    /// Create a new `PUnsubscribe` command which unsubscribes from the specified patterns.
    pub(crate) fn new(patterns: Vec<String>) -> PUnsubscribe {
        PUnsubscribe { patterns }
    }

    /// Parse a `PUnsubscribe` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `PUNSUBSCRIBE` string has already been consumed.
    ///
    /// # Returns
    ///
    /// On success, the `PUnsubscribe` value is returned. If the frame is malformed,
    /// `Err` is returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing zero or more strings.
    ///
    /// ```text
    /// PUNSUBSCRIBE [pattern1 [pattern2 ...]]
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<PUnsubscribe> {
        use ParseError::EndOfStream;

        let mut patterns = Vec::new();

        // The `PUNSUBSCRIBE` string has already been consumed. At this point,
        // there may be zero or more strings remaining in `parse`. These represent
        // the patterns to unsubscribe from.
        loop {
            match parse.next_string() {
                // A string has been consumed from the `parse`, push it into the
                // list of patterns to unsubscribe from.
                Ok(s) => patterns.push(s),
                // The `EndOfStream` error indicates there is no further data to
                // parse.
                Err(EndOfStream) => break,
                // All other errors are bubbled up, resulting in the connection
                // being terminated.
                Err(err) => return Err(err.into()),
            }
        }

        Ok(PUnsubscribe { patterns })
    }
}

/// Subscribe to a pattern
async fn subscribe_to_pattern(
    pattern_name: String,
    subscriptions: &mut StreamMap<String, Messages>,
    db: &Db,
    dst: &mut Connection,
) -> crate::Result<()> {
    let mut rx = db.psubscribe(pattern_name.clone());

    // Subscribe to the pattern.
    let rx = Box::pin(async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(msg) => yield msg,
                // If we lagged in consuming messages, just resume.
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(_) => break,
            }
        }
    });

    // Track subscription in this client's subscription set.
    subscriptions.insert(pattern_name.clone(), rx);

    // Respond with the successful subscription
    let response = make_psubscribe_frame(pattern_name, subscriptions.len());
    dst.write_frame(&response).await?;

    Ok(())
}

/// Create a frame for a successful pattern subscription
fn make_psubscribe_frame(pattern: String, num_subs: usize) -> Frame {
    let mut frame = Frame::array();
    frame.push_bulk(Bytes::from("psubscribe".as_bytes()));
    frame.push_bulk(Bytes::from(pattern.into_bytes()));
    frame.push_int(num_subs as u64);
    frame
}

/// Create a frame for a message received on a pattern
fn make_message_frame(pattern: String, msg: Bytes) -> Frame {
    let mut frame = Frame::array();
    frame.push_bulk(Bytes::from("pmessage".as_bytes()));
    frame.push_bulk(Bytes::from(pattern.into_bytes()));
    frame.push_bulk(msg);
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    #[test]
    fn test_psubscribe_parse_frames() {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("psubscribe".as_bytes()));
        frame.push_bulk(Bytes::from("news.*".as_bytes()));
        frame.push_bulk(Bytes::from("user:?123".as_bytes()));

        let mut parse = Parse::new(frame).unwrap();
        // Simulate the command name being consumed
        let _ = parse.next_string().unwrap();
        let cmd = PSubscribe::parse_frames(&mut parse).unwrap();

        assert_eq!(cmd.patterns, vec!["news.*", "user:?123"]);
    }

    #[test]
    fn test_punsubscribe_parse_frames() {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("punsubscribe".as_bytes()));
        frame.push_bulk(Bytes::from("news.*".as_bytes()));

        let mut parse = Parse::new(frame).unwrap();
        // Simulate the command name being consumed
        let _ = parse.next_string().unwrap();
        let cmd = PUnsubscribe::parse_frames(&mut parse).unwrap();

        assert_eq!(cmd.patterns, vec!["news.*"]);
    }

    #[test]
    fn test_punsubscribe_parse_frames_no_patterns() {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("punsubscribe".as_bytes()));

        let mut parse = Parse::new(frame).unwrap();
        // Simulate the command name being consumed
        let _ = parse.next_string().unwrap();
        let cmd = PUnsubscribe::parse_frames(&mut parse).unwrap();

        assert_eq!(cmd.patterns, Vec::<String>::new());
    }
} 