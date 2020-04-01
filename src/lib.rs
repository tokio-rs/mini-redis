pub const DEFAULT_PORT: &str = "6379";

pub mod client;

pub mod cmd;
use cmd::Command;

mod conn;
use conn::Connection;

mod frame;
use frame::Frame;

mod db;
use db::Db;

mod parse;
use parse::{Parse, ParseError};

pub mod server;

mod shutdown;
use shutdown::Shutdown;

/// Error returned by most functions.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A specialized `Result` type for mini-redis operations.
pub type Result<T> = std::result::Result<T, Error>;
