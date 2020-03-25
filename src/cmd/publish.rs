use crate::{Connection, Frame, Db, Parse, ParseError};

use bytes::Bytes;
use std::io;

#[derive(Debug)]
pub struct Publish {
    channel: String,
    message: Bytes,
}

impl Publish {
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Publish, ParseError> {
        let channel = parse.next_string()?;
        let message = parse.next_bytes()?;

        Ok(Publish { channel, message })
    }

    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> io::Result<()> {
        // Set the value
        let num_subscribers = db.publish(&self.channel, self.message);

        let response = Frame::Integer(num_subscribers as u64);
        dst.write_frame(&response).await
    }
}
