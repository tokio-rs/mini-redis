use crate::{Command, Connection, Kv, Shutdown};

use tokio::io;
use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::broadcast;

struct Server {
    /// Database state
    kv: Kv,

    /// TCP listener
    listener: TcpListener,

    /// Listen for shutdown
    notify_shutdown: broadcast::Sender<()>,
}

/// Handles a connections
struct Handler {
    /// Database state
    kv: Kv,

    /// The TCP connection decorated with the redis protocol encoder / decoder
    connection: Connection,

    /// Listen for shutdown notifications
    shutdown: Shutdown,
}

/// Run the mini-redis server.
pub async fn run(port: &str) -> io::Result<()> {
    let (notify_shutdown, _) = broadcast::channel(1);

    let mut server = Server {
        listener: TcpListener::bind(&format!("127.0.0.1:{}", port)).await?,
        kv: Kv::new(),
        notify_shutdown,
    };

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                // TODO: gracefully handle this error
                eprintln!("failed to accept; err = {}", err);
            }
        }
        _ = signal::ctrl_c() => {
            println!("shutting down");
        }
    }

    Ok(())
}

impl Server {
    /// Run the server
    async fn run(&mut self) -> io::Result<()> {
        loop {
            let (socket, _) = self.listener.accept().await?;

            let mut handler = Handler {
                kv: self.kv.clone(),
                connection: Connection::new(socket),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
            };

            tokio::spawn(async move {
                if let Err(e) = handler.run().await {
                    eprintln!("client err = {:?}", e);
                }
            });
        }
    }
}

impl Handler {
    async fn run(&mut self) -> io::Result<()> {
        while !self.shutdown.is_shutdown() {
            let maybe_frame = tokio::select! {
                res = self.connection.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    break;
                }
            };

            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };

            let cmd = Command::from_frame(frame)?;

            cmd.apply(&self.kv, &mut self.connection, &mut self.shutdown)
                .await?;
        }

        Ok(())
    }
}
