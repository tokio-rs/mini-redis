use mini_redis::{client, DEFAULT_PORT};

use bytes::Bytes;
use clap::Clap;
use std::num::ParseIntError;
use std::str;
use std::time::Duration;

#[derive(Clap, Debug)]
#[clap(name = "mini-redis-cli", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Issue Redis commands")]
struct Cli {
    #[clap(subcommand)]
    command: Command,

    #[clap(name = "hostname", long = "--host", default_value = "127.0.0.1")]
    host: String,

    #[clap(name = "port", long = "--port", default_value = DEFAULT_PORT)]
    port: String,
}

#[derive(Clap, Debug)]
enum Command {
    /// Get the value of key.
    Get {
        /// Name of key to get
        key: String
    },
    /// Set key to hold the string value.
    Set {
        /// Name of key to set
        key: String,

        /// Value to set.
        #[clap(parse(from_str = bytes_from_str))]
        value: Bytes,

        /// Expire the value after specified amount of time
        #[clap(parse(try_from_str = duration_from_ms_str))]
        expires: Option<Duration>,
    },
}

/// Entry point for CLI tool.
///
/// The `[tokio::main]` annotation signals that the Tokio runtime should be
/// started when the function is called. The body of the function is executed
/// within the newly spawned runtime.
///
/// `basic_scheduler` is used here to avoid spawning background threads. The CLI
/// tool use case benefits more by being lighter instead of multi-threaded.
#[tokio::main(basic_scheduler)]
async fn main() -> mini_redis::Result<()> {
    // Enable logging
    tracing_subscriber::fmt::try_init()?;

    // Parse command line arguments
    let cli = Cli::parse();

    // Get the remote address to connect to
    let addr = format!("{}:{}", cli.host, cli.port);

    // Establish a connection
    let mut client = client::connect(&addr).await?;

    match cli.command {
        Command::Get { key } => {
            if let Some(value) = client.get(&key).await? {
                if let Ok(string) = str::from_utf8(&value) {
                    println!("\"{}\"", string);
                } else {
                    println!("{:?}", value);
                }
            } else {
                println!("(nil)");
            }
        }
        Command::Set { key, value, expires: None } => {
            client.set(&key, value).await?;
            println!("OK");
        }
        Command::Set { key, value, expires: Some(expires) } => {
            client.set_expires(&key, value, expires).await?;
            println!("OK");
        }
    }

    Ok(())
}

fn duration_from_ms_str(src: &str) -> Result<Duration, ParseIntError> {
    let ms = src.parse::<u64>()?;
    Ok(Duration::from_millis(ms))
}

fn bytes_from_str(src: &str) -> Bytes {
    Bytes::from(src.to_string())
}
