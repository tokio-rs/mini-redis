use crate::{Connection, Db, Frame, Parse, ParseError};

use bytes::Bytes;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Get {
    key: String,
}

impl Get {
    /// Create a new `Get` command which fetches `key`.
    pub(crate) fn new(key: impl ToString) -> Get {
        Get { key: key.to_string() }
    }

    // instrumenting functions will log all of the arguments passed to the function
    // with their debug implementations
    // see https://docs.rs/tracing/0.1.13/tracing/attr.instrument.html
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Get, ParseError> {
        let key = parse.next_string()?;

        // adding this debug event allows us to see what key is parsed
        // the ? sigil tells `tracing` to use the `Debug` implementation
        // get parse events can be filtered by running
        // RUST_LOG=mini_redis::cmd::get[parse_frames]=debug cargo run --bin server
        // see https://docs.rs/tracing/0.1.13/tracing/#recording-fields
        debug!(?key);

        Ok(Get { key })
    }

    #[instrument(skip(self, db, dst))]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        let response = if let Some(value) = db.get(&self.key) {
            Frame::Bulk(value)
        } else {
            Frame::Null
        };

        debug!(?response);

        dst.write_frame(&response).await?;
        Ok(())
    }

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("get".as_bytes()));
        frame.push_bulk(Bytes::from(self.key.into_bytes()));
        frame
    }
}
