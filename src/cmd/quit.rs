use crate::{Connection, Frame, Parse};

/// Gracefully terminates the connection.
///
/// This command allows clients to cleanly disconnect from the server.
#[derive(Debug)]
pub struct Quit;

impl Quit {
    /// Create a new `Quit` command.
    pub(crate) fn new() -> Quit {
        Quit
    }

    /// Parse a `Quit` instance from a received frame.
    ///
    /// The `Parse` argument provides a cursor-like API to read fields from the
    /// `Frame`. At this point, the entire frame has already been received from
    /// the socket.
    ///
    /// The `QUIT` string has already been consumed.
    ///
    /// # Returns
    ///
    /// Returns the `Quit` value on success. If the frame is malformed, `Err` is
    /// returned.
    ///
    /// # Format
    ///
    /// Expects an array frame containing just `QUIT`.
    ///
    /// ```text
    /// QUIT
    /// ```
    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Quit> {
        // The `QUIT` string has already been consumed. There should be no
        // additional fields in the frame.
        parse.finish()?;
        Ok(Quit)
    }

    /// Apply the `Quit` command.
    ///
    /// The response is written to `dst`. This is called by the server in order
    /// to execute a received command.
    ///
    /// # Note
    ///
    /// The actual connection termination is handled by the connection handler
    /// when this command returns. This command just sends an OK response.
    pub(crate) async fn apply(self, dst: &mut Connection) -> crate::Result<()> {
        // Send OK response
        let response = Frame::Simple("OK".to_string());
        dst.write_frame(&response).await?;
        
        Ok(())
    }

    /// Converts the command into an equivalent `Frame`.
    ///
    /// This is called by the client when encoding a `Quit` command to send
    /// to the server.
    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk("quit".into());
        frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Frame;

    #[test]
    fn test_quit_parse_frames() {
        let mut frame = Frame::array();
        frame.push_bulk("quit".into());

        let mut parse = Parse::new(frame).unwrap();
        let cmd = Quit::parse_frames(&mut parse).unwrap();

        assert!(matches!(cmd, Quit));
    }

    #[test]
    fn test_quit_parse_frames_with_extra_fields() {
        let mut frame = Frame::array();
        frame.push_bulk("quit".into());
        frame.push_bulk("extra".into());

        let mut parse = Parse::new(frame).unwrap();
        let result = Quit::parse_frames(&mut parse);
        assert!(result.is_err());
    }
} 