use crate::{Command, Connection, Db, Shutdown};

use tokio::net::TcpListener;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{debug, error, instrument, info};

#[derive(Debug)]
struct Server {
    /// Database state
    db: Db,

    /// TCP listener
    listener: TcpListener,

    /// Listen for shutdown
    notify_shutdown: broadcast::Sender<()>,
}

/// Handles a connections
#[derive(Debug)]
struct Handler {
    /// Database state
    db: Db,

    /// The TCP connection decorated with the redis protocol encoder / decoder
    connection: Connection,

    /// Listen for shutdown notifications
    shutdown: Shutdown,
}

/// Run the mini-redis server.
pub async fn run(port: &str) -> crate::Result<()> {
    let (notify_shutdown, _) = broadcast::channel(1);

    let mut server = Server {
        listener: TcpListener::bind(&format!("127.0.0.1:{}", port)).await?,
        db: Db::new(),
        notify_shutdown,
    };

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                // TODO: gracefully handle this error
                error!(cause = %err, "failed to accept");
            }
        }
        _ = signal::ctrl_c() => {
            info!("shutting down");
        }
    }

    Ok(())
}

impl Server {
    /// Run the server
    async fn run(&mut self) -> crate::Result<()> {
        info!("accepting inbound connections");

        loop {
            let (socket, _) = self.listener.accept().await?;

            let mut handler = Handler {
                db: self.db.clone(),
                connection: Connection::new(socket),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
            };

            tokio::spawn(async move {
                if let Err(err) = handler.run().await {
                    error!(cause = ?err, "connection error");
                }
            });
        }
    }
}

impl Handler {
    #[instrument(skip(self))]
    async fn run(&mut self) -> crate::Result<()> {
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

            debug!(?cmd);

            cmd.apply(&self.db, &mut self.connection, &mut self.shutdown)
                .await?;
        }

        Ok(())
    }
}
