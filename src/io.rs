use std::{net::SocketAddr, pin::Pin};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) type DynIo = Pin<Box<dyn Io>>;
pub(crate) type DynListener = Box<dyn Listener + Send + Sync>;

pub trait Io: AsyncRead + AsyncWrite + Send + Sync + 'static {}

impl<T: AsyncRead + AsyncWrite + Send + Sync + 'static> Io for T {}

#[async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn accept(&self) -> std::io::Result<(DynIo, SocketAddr)>;
}

#[async_trait]
impl Listener for tokio::net::TcpListener {
    async fn accept(&self) -> std::io::Result<(DynIo, SocketAddr)> {
        let (t, s) = self.accept().await?;
        Ok((Box::pin(t), s))
    }
}

#[cfg(feature = "sim")]
#[async_trait]
impl Listener for turmoil::net::TcpListener {
    async fn accept(&self) -> std::io::Result<(DynIo, SocketAddr)> {
        let (t, s) = self.accept().await?;
        Ok((Box::pin(t), s))
    }
}
