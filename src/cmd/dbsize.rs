use crate::{Connection, Db, Frame, Parse};

use bytes::Bytes;
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct DbSize {}

impl DbSize {
    pub fn new() -> DbSize {
        Self {}
    }

    pub(crate) fn parse_frames(_: &Parse) -> crate::Result<DbSize> {
        Ok(DbSize {})
    }

    #[instrument(skip(self, db, dst))]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        // Get the value of size
        let response = Frame::Integer(db.size() as u64);

        debug!(?response);

        // Write the response back to the client
        dst.write_frame(&response).await?;

        Ok(())
    }

    pub(crate) fn into_frame(self) -> Frame {
        let mut frame = Frame::array();
        frame.push_bulk(Bytes::from("dbsize".as_bytes()));
        frame
    }
}
