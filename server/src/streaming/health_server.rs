use crate::health::{HealthMonitor, HealthStatus, ReadinessStatus};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use serde_json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{debug, error, info};

pub struct HealthServer {
    health_monitor: Arc<HealthMonitor>,
    bind_addr: SocketAddr,
}

impl HealthServer {
    pub fn new(health_monitor: Arc<HealthMonitor>, port: u16) -> Self {
        let bind_addr = SocketAddr::from(([0, 0, 0, 0], port));
        Self {
            health_monitor,
            bind_addr,
        }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let health_monitor = Arc::clone(&self.health_monitor);

        let make_svc = make_service_fn(move |_conn| {
            let health_monitor = Arc::clone(&health_monitor);
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    let health_monitor = Arc::clone(&health_monitor);
                    async move { handle_request(req, health_monitor).await }
                }))
            }
        });

        let server = Server::bind(&self.bind_addr).serve(make_svc);

        info!("Health server listening on {}", self.bind_addr);

        if let Err(e) = server.await {
            error!("Health server error: {}", e);
        }

        Ok(())
    }
}

async fn handle_request(
    req: Request<Body>,
    health_monitor: Arc<HealthMonitor>,
) -> Result<Response<Body>, Infallible> {
    let response = match (req.method(), req.uri().path()) {
        (&Method::GET, "/health") => handle_health_check(health_monitor).await,
        (&Method::GET, "/ready") => handle_readiness_check(health_monitor).await,
        (&Method::GET, "/metrics") => handle_metrics().await,
        (&Method::GET, "/") => handle_root().await,
        _ => handle_not_found().await,
    };

    Ok(response)
}

async fn handle_health_check(health_monitor: Arc<HealthMonitor>) -> Response<Body> {
    debug!("Processing health check request");

    match serde_json::to_string(&health_monitor.get_health_status().await) {
        Ok(json) => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            error!("Failed to serialize health status: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal server error"))
                .unwrap()
        }
    }
}

async fn handle_readiness_check(health_monitor: Arc<HealthMonitor>) -> Response<Body> {
    debug!("Processing readiness check request");

    let readiness_status = health_monitor.get_readiness_status().await;
    let status_code = if readiness_status.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    match serde_json::to_string(&readiness_status) {
        Ok(json) => Response::builder()
            .status(status_code)
            .header("content-type", "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => {
            error!("Failed to serialize readiness status: {}", e);
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Internal server error"))
                .unwrap()
        }
    }
}

async fn handle_metrics() -> Response<Body> {
    debug!("Processing metrics request");

    // TODO: Implement proper Prometheus metrics format
    let metrics = r#"
# HELP dpstream_sessions_total Total number of streaming sessions
# TYPE dpstream_sessions_total counter
dpstream_sessions_total 0

# HELP dpstream_connected_clients Number of currently connected clients
# TYPE dpstream_connected_clients gauge
dpstream_connected_clients 0

# HELP dpstream_latency_histogram_bucket Streaming latency histogram
# TYPE dpstream_latency_histogram_bucket histogram
dpstream_latency_histogram_bucket{le="10"} 0
dpstream_latency_histogram_bucket{le="25"} 0
dpstream_latency_histogram_bucket{le="50"} 0
dpstream_latency_histogram_bucket{le="100"} 0
dpstream_latency_histogram_bucket{le="+Inf"} 0

# HELP dpstream_video_bytes_total Total video bytes transmitted
# TYPE dpstream_video_bytes_total counter
dpstream_video_bytes_total 0

# HELP dpstream_audio_bytes_total Total audio bytes transmitted
# TYPE dpstream_audio_bytes_total counter
dpstream_audio_bytes_total 0

# HELP dpstream_packets_lost_total Total packets lost
# TYPE dpstream_packets_lost_total counter
dpstream_packets_lost_total 0

# HELP dpstream_frame_drops_total Total frames dropped
# TYPE dpstream_frame_drops_total counter
dpstream_frame_drops_total 0
"#;

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/plain; version=0.0.4; charset=utf-8")
        .body(Body::from(metrics))
        .unwrap()
}

async fn handle_root() -> Response<Body> {
    let response_body = r#"
<!DOCTYPE html>
<html>
<head>
    <title>dpstream Health Check</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 40px; }
        .endpoint { margin: 10px 0; }
        .endpoint a { text-decoration: none; color: #0066cc; }
        .endpoint a:hover { text-decoration: underline; }
    </style>
</head>
<body>
    <h1>dpstream Health Check Endpoints</h1>
    <div class="endpoint">
        <strong>Health Check:</strong> <a href="/health">/health</a> - Overall service health status
    </div>
    <div class="endpoint">
        <strong>Readiness Check:</strong> <a href="/ready">/ready</a> - Service readiness for traffic
    </div>
    <div class="endpoint">
        <strong>Metrics:</strong> <a href="/metrics">/metrics</a> - Prometheus metrics
    </div>
</body>
</html>
"#;

    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/html")
        .body(Body::from(response_body))
        .unwrap()
}

async fn handle_not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::{Client, Uri};

    #[tokio::test]
    async fn test_health_endpoints() {
        let health_monitor = Arc::new(HealthMonitor::new("1.0.0".to_string()));
        let server = HealthServer::new(health_monitor, 8081);

        // Start server in background
        tokio::spawn(async move {
            server.run().await.unwrap();
        });

        // Wait for server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let client = Client::new();

        // Test health endpoint
        let uri: Uri = "http://127.0.0.1:8081/health".parse().unwrap();
        let response = client.get(uri).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Test readiness endpoint
        let uri: Uri = "http://127.0.0.1:8081/ready".parse().unwrap();
        let response = client.get(uri).await.unwrap();
        // Readiness might be SERVICE_UNAVAILABLE depending on checks
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::SERVICE_UNAVAILABLE
        );

        // Test metrics endpoint
        let uri: Uri = "http://127.0.0.1:8081/metrics".parse().unwrap();
        let response = client.get(uri).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
