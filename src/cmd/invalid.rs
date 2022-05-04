use crate::{Connection, Frame, ParseError};

use tracing::instrument;

/// Represents a malformed frame. This is not a real `Redis` command.
#[derive(Debug)]
pub struct Invalid {
    error: ParseError,
}

impl Invalid {
    /// Create a new `Invalid` command which responds to frames that could not
    /// be successfully parsed as commands.
    pub(crate) fn new(error: ParseError) -> Self {
        Self { error }
    }

    /// Responds to the client, indicating the command could not be parsed.
    #[instrument(level = "trace", name = "ParseError::apply", skip(dst))]
    pub(crate) async fn apply(self, dst: &mut Connection) -> crate::Result<()> {
        let response = Frame::Error(self.error.to_string());
        dst.write_frame(&response).await?;
        Ok(())
    }
}
