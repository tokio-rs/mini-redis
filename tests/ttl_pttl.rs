use mini_redis::{server, Connection, Frame};
use bytes::Bytes;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::time::{self, Duration};

async fn start_server() -> (String, oneshot::Sender<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        server::run(listener, async move {
            let _ = shutdown_rx.await;
        })
        .await;
    });

    (format!("{}:{}", addr.ip(), addr.port()), shutdown_tx)
}

async fn send_cmd(conn: &mut Connection, parts: Vec<Frame>) -> Frame {
    let frame = Frame::Array(parts);
    conn.write_frame(&frame).await.unwrap();
    let resp = conn.read_frame().await.unwrap().unwrap();
    resp
}

async fn connect(addr: &str) -> Connection {
    let stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    Connection::new(stream)
}

#[tokio::test(flavor = "current_thread")]
async fn ttl_missing_key_returns_minus_2() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("ttl")), Frame::Bulk(Bytes::from("missing"))]).await;
    match resp {
        Frame::Integer(v) => assert_eq!(v, -2),
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}

#[tokio::test(flavor = "current_thread")]
async fn ttl_no_expire_returns_minus_1() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    // SET key value
    let resp = send_cmd(
        &mut conn,
        vec![
            Frame::Bulk(Bytes::from("set")),
            Frame::Bulk(Bytes::from("k1")),
            Frame::Bulk(Bytes::from("v")),
        ],
    )
    .await;
    assert!(matches!(resp, Frame::Simple(ref s) if s == "OK"));

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("ttl")), Frame::Bulk(Bytes::from("k1"))]).await;
    match resp {
        Frame::Integer(v) => assert_eq!(v, -1),
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}

#[tokio::test(flavor = "current_thread")]
async fn ttl_with_ex_two_seconds_is_1_or_2_immediately() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    // SET k2 v EX 2
    let resp = send_cmd(
        &mut conn,
        vec![
            Frame::Bulk(Bytes::from("set")),
            Frame::Bulk(Bytes::from("k2")),
            Frame::Bulk(Bytes::from("v")),
            Frame::Bulk(Bytes::from("ex")),
            Frame::Bulk(Bytes::from("2")),
        ],
    )
    .await;
    assert!(matches!(resp, Frame::Simple(ref s) if s == "OK"));

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("ttl")), Frame::Bulk(Bytes::from("k2"))]).await;
    match resp {
        Frame::Integer(v) => assert!(v == 1 || v == 2, "expected 1 or 2, got {}", v),
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}

#[tokio::test(flavor = "current_thread")]
async fn ttl_after_expiration_returns_minus_2() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    // SET k3 v EX 2
    let resp = send_cmd(
        &mut conn,
        vec![
            Frame::Bulk(Bytes::from("set")),
            Frame::Bulk(Bytes::from("k3")),
            Frame::Bulk(Bytes::from("v")),
            Frame::Bulk(Bytes::from("ex")),
            Frame::Bulk(Bytes::from("2")),
        ],
    )
    .await;
    assert!(matches!(resp, Frame::Simple(ref s) if s == "OK"));

    time::sleep(Duration::from_millis(2100)).await;

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("ttl")), Frame::Bulk(Bytes::from("k3"))]).await;
    match resp {
        Frame::Integer(v) => assert_eq!(v, -2),
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}

#[tokio::test(flavor = "current_thread")]
async fn pttl_immediate_is_positive_and_leq_2000() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    // SET k4 v EX 2
    let resp = send_cmd(
        &mut conn,
        vec![
            Frame::Bulk(Bytes::from("set")),
            Frame::Bulk(Bytes::from("k4")),
            Frame::Bulk(Bytes::from("v")),
            Frame::Bulk(Bytes::from("ex")),
            Frame::Bulk(Bytes::from("2")),
        ],
    )
    .await;
    assert!(matches!(resp, Frame::Simple(ref s) if s == "OK"));

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("pttl")), Frame::Bulk(Bytes::from("k4"))]).await;
    match resp {
        Frame::Integer(v) => {
            assert!(v > 0 && v <= 2000, "expected (0,2000], got {}", v);
        }
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}

#[tokio::test(flavor = "current_thread")]
async fn pttl_no_expire_and_missing() {
    let (addr, shutdown) = start_server().await;
    let mut conn = connect(&addr).await;

    // missing
    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("pttl")), Frame::Bulk(Bytes::from("zzz"))]).await;
    match resp {
        Frame::Integer(v) => assert_eq!(v, -2),
        f => panic!("unexpected frame: {:?}", f),
    }

    // set key with no expire
    let resp = send_cmd(
        &mut conn,
        vec![
            Frame::Bulk(Bytes::from("set")),
            Frame::Bulk(Bytes::from("k5")),
            Frame::Bulk(Bytes::from("v")),
        ],
    )
    .await;
    assert!(matches!(resp, Frame::Simple(ref s) if s == "OK"));

    let resp = send_cmd(&mut conn, vec![Frame::Bulk(Bytes::from("pttl")), Frame::Bulk(Bytes::from("k5"))]).await;
    match resp {
        Frame::Integer(v) => assert_eq!(v, -1),
        f => panic!("unexpected frame: {:?}", f),
    }

    let _ = shutdown.send(());
}


