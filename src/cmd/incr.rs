use crate::{Connection, Db, Frame, Parse};

use tracing::debug;

#[derive(Debug)]
pub struct Incr {
    key: String,
}

impl Incr {
    pub fn new(key: impl ToString) -> Incr {
        Incr {
            key: key.to_string(),
        }
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Incr> {
        let key = parse.next_string()?;

        Ok(Incr { key })
    }

    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        let response = match db.incr(&self.key) {
            Ok(value) => Frame::Integer(value),
            Err(err) => Frame::Error(err.to_string()),
        };

        debug!(?response);

        dst.write_frame(&response).await?;

        Ok(())
    }
}
