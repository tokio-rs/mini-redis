//! Minimal Redis server implementation
//!
//! Provides an async `run` function that listens for inbound connections,
//! spawning a task per connection.

use crate::{Command, Connection, Db, DbDropGuard, Shutdown, MetricsServer, Frame};

use std::future::Future;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Semaphore};
use tokio::time::{self, Duration};
use tracing::{debug, error, info};

/// Server listener state. Created in the `run` call. It includes a `run` method
/// which performs the TCP listening and initialization of per-connection state.
#[derive(Debug)]
struct Listener {
    /// Shared database handle.
    ///
    /// Contains the key / value store as well as the broadcast channels for
    /// pub/sub.
    ///
    /// This holds a wrapper around an `Arc`. The internal `Db` can be
    /// retrieved and passed into the per connection state (`Handler`).
    db_holder: DbDropGuard,

    /// TCP listener supplied by the `run` caller.
    listener: TcpListener,

    /// Limit the max number of connections.
    ///
    /// A `Semaphore` is used to limit the max number of connections. Before
    /// attempting to accept a new connection, a permit is acquired from the
    /// semaphore. If none are available, the listener waits for one.
    ///
    /// When handlers complete processing a connection, the permit is returned
    /// to the semaphore.
    limit_connections: Arc<Semaphore>,

    /// Broadcasts a shutdown signal to all active connections.
    ///
    /// The initial `shutdown` trigger is provided by the `run` caller. The
    /// server is responsible for gracefully shutting down active connections.
    /// When a connection task is spawned, it is passed a broadcast receiver
    /// handle. When a graceful shutdown is initiated, a `()` value is sent via
    /// the broadcast::Sender. Each active connection receives it, reaches a
    /// safe terminal state, and completes the task.
    notify_shutdown: broadcast::Sender<()>,

    /// Used as part of the graceful shutdown process to wait for client
    /// connections to complete processing.
    ///
    /// Tokio channels are closed once all `Sender` handles go out of scope.
    /// When a channel is closed, the receiver receives `None`. This is
    /// leveraged to detect all connection handlers completing. When a
    /// connection handler is initialized, it is assigned a clone of
    /// `shutdown_complete_tx`. When the listener shuts down, it drops the
    /// sender held by this `shutdown_complete_tx` field. Once all handler tasks
    /// complete, all clones of the `Sender` are also dropped. This results in
    /// `shutdown_complete_rx.recv()` completing with `None`. At this point, it
    /// is safe to exit the server process.
    shutdown_complete_tx: mpsc::Sender<()>,

    /// Metrics server for exposing Prometheus metrics
    metrics_server: Option<MetricsServer>,
}

/// Per-connection handler. Reads requests from `connection` and applies the
/// commands to `db`.
#[derive(Debug)]
struct Handler {
    /// Shared database handle.
    ///
    /// When a command is received from `connection`, it is applied with `db`.
    /// The implementation of the command is in the `cmd` module. Each command
    /// will need to interact with `db` in order to complete the work.
    db: Db,

    /// The TCP connection decorated with the redis protocol encoder / decoder
    /// implemented using a buffered `TcpStream`.
    ///
    /// When `Listener` receives an inbound connection, the `TcpStream` is
    /// passed to `Connection::new`, which initializes the associated buffers.
    /// `Connection` allows the handler to operate at the "frame" level and keep
    /// the byte level protocol parsing details encapsulated in `Connection`.
    connection: Connection,

    /// Listen for shutdown notifications.
    ///
    /// A wrapper around the `broadcast::Receiver` paired with the sender in
    /// `Listener`. The connection handler processes requests from the
    /// connection until the peer disconnects **or** a shutdown notification is
    /// received from `shutdown`. In the latter case, any in-flight work being
    /// processed for the peer is continued until it reaches a safe state, at
    /// which point the connection is terminated.
    shutdown: Shutdown,

    /// Not used directly. Instead, when `Handler` is dropped...?
    _shutdown_complete: mpsc::Sender<()>,
}

/// Maximum number of concurrent connections the redis server will accept.
const MAX_CONNECTIONS: usize = 250;

/// Run the mini-redis server.
///
/// `listener` accepts inbound connections. The server will continue to run
/// until `shutdown` completes.
///
/// `tokio::signal::ctrl_c()` can be used as the `shutdown` argument. This will
/// listen for a SIGINT signal.
pub async fn run(listener: TcpListener, shutdown: impl Future, metrics_port: Option<u16>) {
    // When the provided `shutdown` future completes, we must send a shutdown
    // message to all active connections. We use a broadcast channel for this
    // purpose. The call below ignores the receiver of the broadcast pair, and when
    // a receiver is needed, the subscribe() method on the sender is used to create
    // one.
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel(1);

    // Initialize the listener state
    let db_holder = DbDropGuard::new();
    let metrics = db_holder.db().get_metrics();
    
    let mut server = Listener {
        listener,
        db_holder,
        limit_connections: Arc::new(Semaphore::new(MAX_CONNECTIONS)),
        notify_shutdown,
        shutdown_complete_tx,
        metrics_server: metrics_port.map(|port| MetricsServer::new(metrics, port)),
    };

    // Start the metrics server if a port is specified
    if let Some(ref mut metrics_server) = server.metrics_server {
        if let Err(e) = metrics_server.start().await {
            error!("Failed to start metrics server: {}", e);
        }
    }

    // Concurrently run the server and listen for the `shutdown` signal. The
    // server task runs until an error is encountered, so under normal
    // circumstances, this `select!` statement runs until the `shutdown` signal
    // is received.
    //
    // `select!` statements are written in the form of:
    //
    // ```
    // <result of async op> = <async op> => <step to perform with result>
    // ```
    //
    // All `<async op>` statements are executed concurrently. Once the **first**
    // op completes, its associated `<step to perform with result>` is
    // performed.
    //
    // The `select!` macro is a foundational building block for writing
    // asynchronous Rust. See the API docs for more details:
    //
    // https://docs.rs/tokio/*/tokio/macro.select.html
    tokio::select! {
        res = server.run() => {
            // If an error is received here, accepting connections from the TCP
            // listener failed multiple times and the server is giving up and
            // shutting down.
            //
            // Errors encountered when handling individual connections do not
            // bubble up to this point.
            if let Err(err) = res {
                error!(cause = %err, "failed to accept");
            }
        }
        _ = shutdown => {
            // The shutdown signal has been received.
            info!("shutting down");
        }
    }

    // Stop the metrics server if it was started
    if let Some(ref mut metrics_server) = server.metrics_server {
        metrics_server.stop().await;
    }

    // Extract the `shutdown_complete` receiver and transmitter
    // explicitly drop `shutdown_transmitter`. This is important, as the
    // `.await` below would otherwise never complete.
    let Listener {
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;

    // When `notify_shutdown` is dropped, all tasks which have `subscribe`d will
    // receive the shutdown signal and can exit
    drop(notify_shutdown);
    // Drop final `Sender` so the `Receiver` below can complete
    drop(shutdown_complete_tx);

    // Wait for all active connections to finish processing. As the `Sender`
    // handle held by the listener has been dropped above, the only remaining
    // `Sender` instances are held by connection handler tasks. When those drop,
    // the `mpsc` channel will close and `recv()` will return `None`.
    let _ = shutdown_complete_rx.recv().await;
}

impl Listener {
    /// Run the server
    ///
    /// Listen for inbound connections. For each inbound connection, spawn a
    /// task to process that connection.
    ///
    /// # Errors
    ///
    /// Returns `Err` if accepting returns an error. This can happen for a
    /// number reasons that resolve over time. For example, if the underlying
    /// operating system has reached an internal limit for max number of
    /// sockets, accept will fail.
    ///
    /// The process is not able to detect when a transient error resolves
    /// itself. One strategy for handling this is to implement a back off
    /// strategy, which is what we do here.
    async fn run(&mut self) -> crate::Result<()> {
        info!("accepting inbound connections");

        loop {
            // Wait for an inbound connection. This involves acquiring a permit
            // from the semaphore.
            //
            // The `accept` call itself does not wait. Instead, if all
            // `MAX_CONNECTIONS` are busy, either the semaphore will reject the
            // permit acquisition or the `accept` call will wait until there is
            // capacity.
            let socket = self.accept().await?;

            // Wait for a permit to become available
            let _permit = self.limit_connections.acquire().await.unwrap();

            // Create a new task to process the socket
            //
            // The socket and permit are moved to the task and dropped when the
            // task completes.
            let mut handler = Handler {
                connection: Connection::new(socket),
                db: self.db_holder.db(),
                shutdown: Shutdown::new(self.notify_shutdown.subscribe()),
                _shutdown_complete: self.shutdown_complete_tx.clone(),
            };

            tokio::spawn(async move {
                // Process the connection. If an error is encountered, log it.
                if let Err(err) = handler.run().await {
                    error!(cause = ?err, "connection error");
                }
            });
        }
    }

    /// Accept an inbound connection.
    ///
    /// Errors are handled by backing off and retrying. An exponential backoff
    /// strategy is used. After the first failure, the server waits 1 second,
    /// after the second failure, the server waits 2 seconds, etc.
    async fn accept(&mut self) -> crate::Result<TcpStream> {
        let mut backoff = 1;

        loop {
            // Perform the accept operation. If a socket is successfully
            // accepted, return it. Otherwise, save the error.
            match self.listener.accept().await {
                Ok((socket, _)) => return Ok(socket),
                Err(err) => {
                    if backoff > 64 {
                        // Accept has been failing for too long. Return the error.
                        return Err(err.into());
                    }
                }
            }

            // Pause execution until the back off period elapses.
            time::sleep(Duration::from_secs(backoff)).await;
            backoff *= 2;
        }
    }
}

impl Handler {
    /// Process a single connection.
    ///
    /// Request frames are read from the socket and processed. Responses are
    /// written back to the socket.
    ///
    /// Currently, pipelining is not implemented. Pipelining is the ability to
    /// process multiple frames from the client without interleaving frames from
    /// multiple connections.
    async fn run(&mut self) -> crate::Result<()> {
        // As long as the connection has not received a shutdown signal, the
        // server continues to process requests.
        while !self.shutdown.is_shutdown() {
            // While reading a request frame, also listen for the shutdown
            // signal.
            let maybe_frame = tokio::select! {
                res = self.connection.read_frame() => res?,
                _ = self.shutdown.recv() => {
                    // If a shutdown signal is received, return from `run`.
                    // This will result in the task terminating.
                    return Ok(());
                }
            };

            // `maybe_frame` will be `None` if the connection has closed.
            let frame = match maybe_frame {
                Some(frame) => frame,
                None => return Ok(()),
            };

            // Convert the frame to a command. If this fails, respond with an
            // error frame to the client.
            let cmd_result = Command::from_frame(frame);

            // Get the command name for metrics before consuming the command
            let cmd_name = cmd_result.as_ref().map(|c| c.get_name()).unwrap_or("unknown").to_string();

            // Track command execution start time for latency metrics
            let _start_time = std::time::Instant::now();

            // Execute the command and track metrics
            let result = match cmd_result {
                Ok(cmd) => {
                    // Apply the command to the database
                    cmd.apply(&self.db, &mut self.connection, &mut self.shutdown).await
                }
                Err(cause) => {
                    // The command was malformed. The `error` was stored in the
                    // frame's error field and will be written to the client.
                    let response = Frame::Error(cause.to_string());
                    self.connection.write_frame(&response).await?;
                    Ok(())
                }
            };

            // Track command execution result
            if result.is_ok() {
                self.db.get_metrics().inc_ops_ok();
            } else {
                self.db.get_metrics().inc_ops_err();
            }

            // Log the command execution
            debug!(?cmd_name, ?result, "command executed");
        }

        Ok(())
    }
}
