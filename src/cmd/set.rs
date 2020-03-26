use crate::cmd::{
    utils::{bytes_from_str, duration_from_ms_str},
    Parse, ParseError,
};
use crate::{Connection, Db, Frame};
use clap::Clap;

use bytes::Bytes;
use std::io;
use std::time::Duration;
use tracing::{debug, instrument};

#[derive(Clap, Debug)]
pub struct Set {
    /// the lookup key
    pub(crate) key: String,

    /// the value to be stored
    #[clap(parse(from_str = bytes_from_str))]
    pub(crate) value: Bytes,

    /// duration in milliseconds
    #[clap(parse(try_from_str = duration_from_ms_str))]
    pub(crate) expire: Option<Duration>,
}

impl Set {
    #[instrument]
    pub(crate) fn parse_frames(parse: &mut Parse) -> Result<Set, ParseError> {
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

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("set".as_bytes()));
        frame.push_bulk(Bytes::from(self.key.into_bytes()));
        frame.push_bulk(self.value);
        frame
    }
}
