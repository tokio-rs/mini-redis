pub const DEFAULT_PORT: &str = "6379";

pub mod client;

pub mod server;

mod conn;
use conn::Connection;

mod frame;
use frame::Frame;

mod kv;
use kv::Kv;

mod parse;
use parse::{Parse, ParseError};

mod shutdown;
use shutdown::Shutdown;
