use crate::{Command, Connection, Db, Shutdown};

use std::future::Future;
use tokio::net::TcpListener;
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
///
/// Accepts connections from the supplied listener. For each inbound connection,
/// a task is spawned to handle that connection. The server runs until the
/// `shutdown` future completes, at which point the server shuts down
/// gracefully.
///
/// `tokio::signal::ctrl_c()` can be used as the `shutdown` argument. This will
/// listen for a SIGINT signal.
pub async fn run(listener: TcpListener, shutdown: impl Future) -> crate::Result<()> {
    // A broadcast channel is used to signal shutdown to each of the active
    // connections. When the provided `shutdown` future completes
    let (notify_shutdown, _) = broadcast::channel(1);

    let mut server = Server {
        listener,
        db: Db::new(),
        notify_shutdown,
    };

    // Concurrently run the server and listen for the `shutdown` signal. The
    // server task runs until an error is encountered, so under normal
    // circumstances, this `select!` statement runs until the `shutdown` signal
    // is received.
    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                // TODO: gracefully handle this error
                error!(cause = %err, "failed to accept");
            }
        }
        _ = shutdown => {
            info!("shutting down");
        }
    }

    Ok(())
}

impl Server {
    /// Run the server
    ///
    /// Listen for inbound connections. For each inbound connection, spawn a
    /// task to process that connection.
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
