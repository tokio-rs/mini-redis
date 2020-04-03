use mini_redis::server;

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::{self, Duration};

/// A basic "hello world" style test. A server instance is started in a
/// background task. A client TCP connection is then established and raw redis
/// commands are sent to the server. The response is evaluated at the byte
/// level.
#[tokio::test]
async fn key_value_get_set() {
    let (addr, _handle) = start_server().await;

    // Establish a connection to the server
    let mut stream = TcpStream::connect(addr).await.unwrap();

    // Get a key, data is missing
    stream.write_all(b"*2\r\n$3\r\nGET\r\n$5\r\nhello\r\n").await.unwrap();

    // Read nil response
    let mut response = [0; 5];

    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"$-1\r\n", &response);

    // Set a key
    stream.write_all(b"*3\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n").await.unwrap();

    // Read OK
    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"+OK\r\n", &response);

    // Get the key, data is present
    stream.write_all(b"*2\r\n$3\r\nGET\r\n$5\r\nhello\r\n").await.unwrap();

    // Read "world" response
    let mut response = [0; 11];

    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"$5\r\nworld\r\n", &response);
}

/// Similar to the basic key-value test, however, this time timeouts will be
/// tested. This test demonstrates how to test time related behavior.
///
/// When writing tests, it is useful to remove sources of non-determinism. Time
/// is a source of non determism. Here, we "pause" time using the
/// `time::pause()` function. This function is available with the `test-util`
/// feature flag. This allows us to deterministically control how time appears
/// to advance to the application.
#[tokio::test]
async fn key_value_timeout() {
    tokio::time::pause();

    let (addr, _handle) = start_server().await;

    // Establish a connection to the server
    let mut stream = TcpStream::connect(addr).await.unwrap();

    // Set a key
    stream.write_all(b"*5\r\n$3\r\nSET\r\n$5\r\nhello\r\n$5\r\nworld\r\n\
                     +EX\r\n:1\r\n").await.unwrap();

    let mut response = [0; 5];

    // Read OK
    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"+OK\r\n", &response);

    // Get the key, data is present
    stream.write_all(b"*2\r\n$3\r\nGET\r\n$5\r\nhello\r\n").await.unwrap();

    // Read "world" response
    let mut response = [0; 11];

    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"$5\r\nworld\r\n", &response);

    // Wait for the key to expire
    time::advance(Duration::from_secs(1)).await;

    // Get a key, data is missing
    stream.write_all(b"*2\r\n$3\r\nGET\r\n$5\r\nhello\r\n").await.unwrap();

    // Read nil response
    let mut response = [0; 5];

    stream.read_exact(&mut response).await.unwrap();

    assert_eq!(b"$-1\r\n", &response);
}

async fn start_server() -> (SocketAddr, JoinHandle<mini_redis::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        server::run(listener, tokio::signal::ctrl_c()).await
    });

    (addr, handle)
}
