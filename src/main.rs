use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    mini_redis::server::run().await
}
