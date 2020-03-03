mod get;
pub use get::Get;

mod publish;
pub use publish::Publish;

mod set;
pub use set::Set;

mod subscribe;
pub use subscribe::{Subscribe, Unsubscribe};

use crate::{Connection, Frame, Kv, Parse, ParseError, Shutdown};

use std::io;

#[derive(Debug)]
pub(crate) enum Command {
    Get(Get),
    Publish(Publish),
    Set(Set),
    Subscribe(Subscribe),
    Unsubscribe(Unsubscribe),
}

impl Command {
    pub(crate) fn from_frame(frame: Frame) -> Result<Command, ParseError> {
        let mut parse = Parse::new(frame)?;

        let command_name = parse.next_string()?.to_lowercase();

        let command = match &command_name[..] {
            "get" => Command::Get(Get::parse(&mut parse)?),
            "publish" => Command::Publish(Publish::parse(&mut parse)?),
            "set" => Command::Set(Set::parse(&mut parse)?),
            "subscribe" => Command::Subscribe(Subscribe::parse(&mut parse)?),
            "unsubscribe" => Command::Unsubscribe(Unsubscribe::parse(&mut parse)?),
            _ => return Err(ParseError::UnknownCommand(command_name)),
        };

        parse.finish()?;
        Ok(command)
    }

    pub(crate) async fn apply(
        self,
        kv: &Kv,
        dst: &mut Connection,
        shutdown: &mut Shutdown,
    ) -> io::Result<()> {
        use Command::*;

        match self {
            Get(cmd) => cmd.apply(kv, dst).await,
            Publish(cmd) => cmd.apply(kv, dst).await,
            Set(cmd) => cmd.apply(kv, dst).await,
            Subscribe(cmd) => cmd.apply(kv, dst, shutdown).await,
            // `Unsubscribe` cannot be applied. It may only be received from the
            // context of a `Subscribe` command.
            Unsubscribe(_) => unimplemented!(),
        }
    }
}
