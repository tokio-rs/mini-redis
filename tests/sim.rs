#![cfg(sim)]

use mini_redis::{client, server};
use turmoil::Builder;

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
        let mut client = client::connect(HOST).await.unwrap();

        client.set("hello", "world".into()).await.unwrap();
        let result = client.get("hello").await.unwrap().unwrap();

        assert_eq!(&result[..], &b"world"[..]);

        Ok(())
    });
}
