//! Type erased IO types to allow turmoil types to be used during specific
//! simulation tests.

use std::{net::SocketAddr, pin::Pin};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

pub(crate) type DynStream = Pin<Box<dyn Io>>;
pub(crate) type DynListener = Box<dyn Accept>;

/// An IO type that can be used as a trait object.
///
/// Rust only allows you to use one non-auto trait when creating
/// a trait object. To get around this we create a new trait that
/// depends on the traits we wanted implemented for the trait object.
pub trait Io: AsyncRead + AsyncWrite + Send + Sync + 'static {}
impl<T: AsyncRead + AsyncWrite + Send + Sync + 'static> Io for T {}

/// A trait to abstract types that can accept dynamic streams.
#[async_trait]
pub trait Accept: Send + Sync + 'static {
    async fn accept(&self) -> crate::Result<(DynStream, SocketAddr)>;
}

#[async_trait]
impl Accept for tokio::net::TcpListener {
    async fn accept(&self) -> crate::Result<(DynStream, SocketAddr)> {
        let (t, s) = self.accept().await?;
        Ok((Box::pin(t), s))
    }
}

#[cfg(feature = "sim")]
#[async_trait]
impl Accept for turmoil::net::TcpListener {
    async fn accept(&self) -> crate::Result<(DynStream, SocketAddr)> {
        let (t, s) = self.accept().await?;
        Ok((Box::pin(t), s))
    }
}
