use mini_redis::server;

use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

#[tokio::test]
async fn basic_kv_usage() {
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

async fn start_server() -> (SocketAddr, JoinHandle<mini_redis::Result<()>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let handle = tokio::spawn(async move {
        server::run(listener, tokio::signal::ctrl_c()).await
    });

    (addr, handle)
}
