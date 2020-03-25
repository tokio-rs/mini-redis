use crate::{Connection, Frame, Db, Parse, ParseError};

use std::io;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Get {
    key: String,
}

impl Get {
    // instrumenting functions will log all of the arguments passed to the function
    // with their debug implementations
    // see https://docs.rs/tracing/0.1.13/tracing/attr.instrument.html
    #[instrument]
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Get, ParseError> {
        let key = parse.next_string()?;

        // adding this debug event allows us to see what key is parsed
        // the ? sigil tells `tracing` to use the `Debug` implementation
        // get parse events can be filtered by running
        // RUST_LOG=mini_redis::cmd::get[parse]=debug cargo run --bin server
        // see https://docs.rs/tracing/0.1.13/tracing/#recording-fields
        debug!(?key);

        Ok(Get { key })
    }

    #[instrument]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> io::Result<()> {
        let response = if let Some(value) = db.get(&self.key) {
            Frame::Bulk(value)
        } else {
            Frame::Null
        };
        debug!(?response);
        dst.write_frame(&response).await
    }
}
