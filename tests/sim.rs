#![cfg(feature = "sim")]

use mini_redis::{client, server};
use turmoil::{net::TcpStream, Builder};

#[test]
fn smoke() {
    let mut sim = Builder::new().build();

    const HOST: (&str, u16) = ("127.0.0.0", 6379);

    sim.host("server", || async {
        let listener = turmoil::net::TcpListener::bind(HOST).await.unwrap();

        server::run(listener, std::future::pending::<()>()).await;
    });

    sim.client("client", async {
        // TODO: ? doesn't work here for some reason
        let stream = TcpStream::connect(HOST).await.unwrap();
        let mut client = client::Client::new(stream);

        client.set("hello", "world".into()).await.unwrap();
        let result = client.get("hello").await.unwrap().unwrap();

        assert_eq!(&result[..], &b"world"[..]);

        Ok(())
    });
}
