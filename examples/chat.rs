//! Cli chat using redis example.
//!
//! A minimal chat example using redis. A user connects to the redis instance
//! and subscribes, and publishes messages across a channel
//!
//! You can test this out by running:
//!
//!     cargo run --bin server
//!
//! Then in as many other terminals as you want, run:
//!
//!     cargo run --example sub
//!
//! And then in another terminal run:
//!
//!     cargo run --example pub

#![warn(rust_2018_idioms)]

use mini_redis::client::{self, Client};
use mini_redis::Result;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin};

#[tokio::main]
pub async fn main() -> Result<()> {
    // Start by reading the username to then use as the messages author
    // when publishing them
    let mut stdout = io::stdout();
    let mut stdin = BufReader::new(io::stdin());
    let mut username = String::new();
    stdout.write_all(b"What is your username: ").await?;
    stdout.flush().await?;
    stdin
        .read_line(&mut username)
        .await
        .map_err(|err| format!("invalid username, {}", err))?;
    // Trim /n from input
    username = username.trim().to_string();

    // Open connections to the mini-redis address.
    let addr = "127.0.0.1:6379";
    let mut client = client::connect(addr).await?;

    // we need a dedicated connection for the subscriber, as `subscribe` consumes the Client.
    // We subscribe the chat channel, it's also the channel where client will publish
    // messages read from user input
    let mut subscriber = client::connect(addr)
        .await?
        .subscribe(vec!["chat".into()])
        .await?;

    // Loop receiving new messages on subcriber
    let usernamec = username.clone();
    tokio::spawn(async move {
        loop {
            match subscriber.next_message().await {
                Ok(Some(message)) => {
                    let content = String::from_utf8_lossy(&message.content);
                    // If message comes from own client discard it
                    // as it's already printed on the screen
                    if !content.starts_with(&usernamec) {
                        println!("{}", content);
                    }
                }
                Err(err) => {
                    println!("error: {}", err);
                    break;
                }
                Ok(None) => {
                    println!("server disconnected");
                    break;
                }
            }
        }
    });

    loop {
        if let Err(err) = read_send_message(&username, &mut stdin, &mut client).await {
            println!("error: {}", err);
        }
    }
}

// Read input from user and publish it as `username: message`
// on the redis server instance
async fn read_send_message(
    username: &str,
    stdin: &mut BufReader<Stdin>,
    client: &mut Client,
) -> Result<()> {
    let mut input = String::new();
    stdin.read_line(&mut input).await?;
    client
        .publish("chat", format!("{}: {}", username, input.trim()).into())
        .await?;
    Ok(())
}
