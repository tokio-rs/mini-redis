use crate::{Connection, Frame, Kv, Parse, ParseError};

use std::io;

#[derive(Debug)]
pub struct Get {
    key: String,
}

impl Get {
    pub(crate) fn parse(parse: &mut Parse) -> Result<Get, ParseError> {
        let key = parse.next_string()?;
        Ok(Get { key })
    }

    pub(crate) async fn apply(self, kv: &Kv, dst: &mut Connection) -> io::Result<()> {
        let response = if let Some(value) = kv.get(&self.key) {
            Frame::Bulk(value)
        } else {
            Frame::Null
        };

        dst.write_frame(&response).await
    }
}
