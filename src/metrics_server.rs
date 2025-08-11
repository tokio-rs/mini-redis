use crate::Metrics;
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info};

/// HTTP metrics server for exposing Prometheus metrics
#[derive(Debug)]
pub struct MetricsServer {
    metrics: Arc<Metrics>,
    port: u16,
    handle: Option<JoinHandle<()>>,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(metrics: Arc<Metrics>, port: u16) -> Self {
        Self {
            metrics,
            port,
            handle: None,
        }
    }

    /// Start the metrics server
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));
        let metrics = Arc::clone(&self.metrics);

        // Create a service to handle requests
        let make_svc = make_service_fn(move |_conn| {
            let metrics = Arc::clone(&metrics);
            async move {
                Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                    let metrics = Arc::clone(&metrics);
                    async move {
                        handle_request(req, metrics).await
                    }
                }))
            }
        });

        // Create the server
        let server = Server::bind(&addr).serve(make_svc);

        info!("Metrics server listening on http://{}", addr);

        // Spawn the server task
        let handle = tokio::spawn(async move {
            if let Err(e) = server.await {
                error!("Metrics server error: {}", e);
            }
        });

        self.handle = Some(handle);

        Ok(())
    }

    /// Stop the metrics server
    pub async fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            // Wait for the task to finish, but don't check if it was aborted
            let _ = handle.await;
        }
    }

    /// Get the port the server is configured to use
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Handle HTTP requests to the metrics server
async fn handle_request(
    req: Request<Body>,
    metrics: Arc<Metrics>,
) -> Result<Response<Body>, Infallible> {
    match (req.method().as_str(), req.uri().path()) {
        ("GET", "/metrics") => {
            let body = metrics.prometheus_string();
            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/plain; version=0.0.4; charset=utf-8")
                .body(Body::from(body))
                .unwrap())
        }
        ("GET", "/") => {
            let body = "Mini-Redis Metrics Server\n\nAvailable endpoints:\n- /metrics - Prometheus metrics\n";
            Ok(Response::builder()
                .status(200)
                .header("Content-Type", "text/plain")
                .body(Body::from(body))
                .unwrap())
        }
        _ => {
            Ok(Response::builder()
                .status(404)
                .body(Body::from("Not Found"))
                .unwrap())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::{Body, Request};

    #[tokio::test]
    async fn test_metrics_server_creation() {
        let metrics = Arc::new(Metrics::new());
        let server = MetricsServer::new(metrics, 9123);
        assert_eq!(server.port(), 9123);
        assert!(server.handle.is_none());
    }

    #[tokio::test]
    async fn test_handle_request_metrics() {
        let metrics = Arc::new(Metrics::new());
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost:9123/metrics")
            .body(Body::empty())
            .unwrap();
        
        let response = handle_request(req, metrics).await.unwrap();
        
        assert_eq!(response.status(), 200);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.starts_with(b"# HELP"));
    }

    #[tokio::test]
    async fn test_handle_request_root() {
        let metrics = Arc::new(Metrics::new());
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost:9123/")
            .body(Body::empty())
            .unwrap();
        
        let response = handle_request(req, metrics).await.unwrap();
        
        assert_eq!(response.status(), 200);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.starts_with(b"Mini-Redis Metrics Server"));
    }

    #[tokio::test]
    async fn test_handle_request_not_found() {
        let metrics = Arc::new(Metrics::new());
        let req = Request::builder()
            .method("GET")
            .uri("http://localhost:9123/nonexistent")
            .body(Body::empty())
            .unwrap();
        
        let response = handle_request(req, metrics).await.unwrap();
        
        assert_eq!(response.status(), 404);
        let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
        assert!(body.starts_with(b"Not Found"));
    }
} 