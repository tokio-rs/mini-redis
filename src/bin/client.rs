use clap::Clap;
use mini_redis::{client, cmd::Set, DEFAULT_PORT};
use std::str;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let port = cli.port.unwrap_or(DEFAULT_PORT.to_string());
    let mut client = client::connect(&format!("127.0.0.1:{}", port)).await?;
    match cli.command {
        Client::Get { key } => {
            let result = client.get(&key).await?;
            if let Some(result) = result {
                println!("\"{}\"", str::from_utf8(&result).unwrap());
            } else {
                println!("(nil)");
            }
            Ok(())
        }
        Client::Set(opts) => match client.set_with_opts(opts).await {
            Ok(_) => {
                println!("OK");
                Ok(())
            }
            Err(e) => {
                eprintln!("{}", e);
                Err(e)
            }
        },
    }
}

#[derive(Clap, Debug)]
#[clap(name = "mini-redis-client", version = env!("CARGO_PKG_VERSION"), author = env!("CARGO_PKG_AUTHORS"), about = "Opens a connection to a Redis server")]
struct Cli {
    #[clap(subcommand)]
    command: Client,
    #[clap(name = "port", long = "--port")]
    port: Option<String>,
}

#[derive(Clap, Debug)]
enum Client {
    /// Gets a value associated with a key
    Get { key: String },

    /// Associates a value with a key
    Set(Set),
}
