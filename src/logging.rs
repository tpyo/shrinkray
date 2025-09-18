use crate::http::HeaderMapExt;
use crate::service::Service;
use axum::extract::State;
use axum::{extract::Request, middleware::Next, response::IntoResponse};
use axum_extra::extract::Host;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

pub async fn middleware(
    Host(domain): Host,
    State(ctx): State<Arc<Service>>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let uri = req.uri().clone();
    let mut request_uri = uri.path();
    if let Some(path_and_query) = uri.path_and_query() {
        request_uri = path_and_query.as_str();
    }

    if ["/", "/metrics", "/healthz", "/favicon.ico"].contains(&request_uri) {
        return next.run(req).await;
    }

    let start = Instant::now();
    let method = req.method().to_string();
    let headers = req.headers();
    let remote_addr = headers
        .get_x_forwarded_for(&ctx.config.proxies)
        .unwrap_or_default();
    let http_user_agent = headers.get_user_agent().unwrap_or_default();
    let http_referrer = headers.get_referrer().unwrap_or_default();

    let response = next.run(req).await;

    info!(
        %method,
        %request_uri,
        %domain,
        %remote_addr,
        status = %response.status().as_u16(),
        response_time = start.elapsed().as_secs_f64(),
        %http_user_agent,
        %http_referrer
    );

    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use axum::{Router, routing::get};
    use axum_test::TestServer;
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::fmt::MakeWriter;

    struct TestWriter {
        buffer: Arc<Mutex<Vec<u8>>>,
    }

    impl TestWriter {
        fn new() -> (Self, Arc<Mutex<Vec<u8>>>) {
            let buffer = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    buffer: buffer.clone(),
                },
                buffer,
            )
        }
    }

    impl Write for TestWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.buffer.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl MakeWriter<'_> for TestWriter {
        type Writer = Self;

        fn make_writer(&self) -> Self::Writer {
            Self {
                buffer: self.buffer.clone(),
            }
        }
    }

    #[tokio::test]
    async fn test_middleware_logging() {
        let (test_writer, buffer) = TestWriter::new();

        // Set up tracing subscriber with our test writer
        let _guard = tracing_subscriber::fmt()
            .with_writer(test_writer)
            .with_ansi(false)
            .try_init();

        let config = Config::default();
        let service = Arc::new(Service::new(config));

        async fn handler() -> &'static str {
            "test response"
        }

        let app = Router::new()
            .route("/test", get(handler))
            .layer(axum::middleware::from_fn_with_state(
                service.clone(),
                middleware,
            ))
            .with_state(service);

        let server = TestServer::new(app).unwrap();

        // Make a request
        let response = server
            .get("/test")
            .add_header("Host", "example.com")
            .add_header("User-Agent", "test-agent")
            .add_header("Referer", "https://example.com/")
            .await;

        response.assert_status_ok();

        // Give a moment for the log to be written
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        let binding = buffer.lock().unwrap();
        let logged_output = String::from_utf8_lossy(&binding);

        assert!(logged_output.contains("method=GET"));
        assert!(logged_output.contains("request_uri=/test"));
        assert!(logged_output.contains("domain=example.com"));
        assert!(logged_output.contains("status=200"));
        assert!(logged_output.contains("http_user_agent=test-agent"));
        assert!(logged_output.contains("http_referrer=https://example.com/"));
        assert!(logged_output.contains("response_time="));
    }
}
