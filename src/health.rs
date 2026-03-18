/// Minimal HTTP health endpoint.
///
/// GET /healthz  → 200 OK  (liveness)
/// GET /readyz   → 200 OK  (readiness — same response, Kubernetes uses separate paths)
///
/// Uses only tokio's built-in TCP support to avoid pulling in a full web
/// framework for two endpoints.
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, warn};

const RESPONSE_OK: &[u8] =
    b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 2\r\nConnection: close\r\n\r\nOK";

const RESPONSE_404: &[u8] =
    b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

pub async fn serve(port: u16) {
    let addr = format!("0.0.0.0:{}", port);
    let listener = match TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            warn!(addr = %addr, error = %e, "Failed to bind health endpoint");
            return;
        }
    };

    loop {
        match listener.accept().await {
            Ok((mut stream, peer)) => {
                debug!(peer = %peer, "Health check connection");
                tokio::spawn(async move {
                    let mut buf = [0u8; 256];
                    let n = match stream.read(&mut buf).await {
                        Ok(n) => n,
                        Err(_) => return,
                    };
                    let request = std::str::from_utf8(&buf[..n]).unwrap_or("");
                    // Match on the request path
                    let response = if request.starts_with("GET /healthz")
                        || request.starts_with("GET /readyz")
                    {
                        RESPONSE_OK
                    } else {
                        RESPONSE_404
                    };
                    let _ = stream.write_all(response).await;
                });
            }
            Err(e) => {
                warn!(error = %e, "Health endpoint accept error");
            }
        }
    }
}
