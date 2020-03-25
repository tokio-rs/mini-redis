use crate::cmd::{Parse, ParseError};
use crate::{Connection, Frame, Db};

use bytes::Bytes;
use std::io;
use std::time::Duration;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Set {
    key: String,
    value: Bytes,
    expire: Option<Duration>,
}

impl Set {
    #[instrument]
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

        debug!(?key, ?value, ?expire);

        Ok(Set { key, value, expire })
    }

    #[instrument]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> io::Result<()> {
        // Set the value
        db.set(self.key, self.value, self.expire);

        let response = Frame::Simple("OK".to_string());
        debug!(?response);
        dst.write_frame(&response).await
    }
}
