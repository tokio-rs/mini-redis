use crate::{Connection, Frame, Kv, Parse, ParseError};

use std::io;
use tracing::{event, Level};

#[derive(Debug)]
pub struct Get {
    key: String,
}

impl Get {
    pub(crate) fn parse(parse: &mut Parse) -> Result<Get, ParseError> {
        let key = parse.next_string()?;

        // adding this debug event allows us to see what key is parsed
        // the ? sigil tells `tracing` to use the `Debug` implementation
        // get events can be filtered by running
        // RUST_LOG=mini_redis::cmd::get=debug cargo run --bin server
        event!(Level::DEBUG, ?key);

        Ok(Get { key })
    }

    pub(crate) async fn apply(self, kv: &Kv, dst: &mut Connection) -> io::Result<()> {
        let response = if let Some(value) = kv.get(&self.key) {
            Frame::Bulk(value)
        } else {
            Frame::Null
        };
        event!(Level::DEBUG, ?response);
        dst.write_frame(&response).await
    }
}
