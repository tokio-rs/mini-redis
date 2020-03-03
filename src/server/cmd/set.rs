use crate::server::cmd::{Parse, ParseError};
use crate::{Connection, Frame, Kv};

use bytes::Bytes;
use std::io;
use std::time::Duration;
use tracing::{event, Level};

#[derive(Debug)]
pub struct Set {
    key: String,
    value: Bytes,
    expire: Option<Duration>,
}

impl Set {
    pub(crate) fn parse(parse: &mut Parse) -> Result<Set, ParseError> {
        use ParseError::EndOfStream;

        let key = parse.next_string()?;
        let value = parse.next_bytes()?;
        let mut expire = None;

        match parse.next_string() {
            Ok(s) if s == "EX" => {
                let secs = parse.next_int()?;
                expire = Some(Duration::from_secs(secs));
            }
            Ok(s) if s == "PX" => {
                let ms = parse.next_int()?;
                expire = Some(Duration::from_millis(ms));
            }
            Ok(_) => unimplemented!(),
            Err(EndOfStream) => {}
            Err(err) => return Err(err),
        }

        event!(Level::DEBUG, ?key, ?value, ?expire);

        Ok(Set { key, value, expire })
    }

    pub(crate) async fn apply(self, kv: &Kv, dst: &mut Connection) -> io::Result<()> {
        // Set the value
        kv.set(self.key, self.value, self.expire);

        let response = Frame::Simple("OK".to_string());
        event!(Level::DEBUG, ?response);
        dst.write_frame(&response).await
    }
}
