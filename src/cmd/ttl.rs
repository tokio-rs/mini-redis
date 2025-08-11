use crate::cmd::Parse;
use crate::{Connection, Db, Frame};
use tracing::{debug, instrument};

#[derive(Debug)]
pub struct Ttl {
    key: String,
}

#[derive(Debug)]
pub struct Pttl {
    key: String,
}

impl Ttl {
    pub fn new(key: impl ToString) -> Ttl {
        Ttl { key: key.to_string() }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Ttl> {
        let key = parse.next_string()?;
        Ok(Ttl { key })
    }

    #[instrument(skip(self, db, dst))]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        let remaining = db.ttl(&self.key);
        let exists = db.contains_key(&self.key);

        let reply = match (exists, remaining) {
            (false, _) => -2,
            (true, None) => -1,
            (true, Some(dur)) => dur.as_secs() as i64,
        };

        let response = Frame::Integer(reply);
        debug!(?response);
        dst.write_frame(&response).await?;
        Ok(())
    }
}

impl Pttl {
    pub fn new(key: impl ToString) -> Pttl {
        Pttl { key: key.to_string() }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub(crate) fn parse_frames(parse: &mut Parse) -> crate::Result<Pttl> {
        let key = parse.next_string()?;
        Ok(Pttl { key })
    }

    #[instrument(skip(self, db, dst))]
    pub(crate) async fn apply(self, db: &Db, dst: &mut Connection) -> crate::Result<()> {
        let remaining = db.ttl(&self.key);
        let exists = db.contains_key(&self.key);

        let reply = match (exists, remaining) {
            (false, _) => -2,
            (true, None) => -1,
            (true, Some(dur)) => dur.as_millis() as i64,
        };

        let response = Frame::Integer(reply);
        debug!(?response);
        dst.write_frame(&response).await?;
        Ok(())
    }
}

