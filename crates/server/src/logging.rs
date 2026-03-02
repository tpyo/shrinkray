use crate::http::HeaderMapExt;
use crate::service::Service;
use axum::extract::State;
use axum::{extract::Request, middleware::Next, response::IntoResponse};
use axum_extra::TypedHeader;
use headers::Host;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;

pub async fn middleware(
    TypedHeader(host): TypedHeader<Host>,
    State(ctx): State<Arc<Service>>,
    req: Request,
    next: Next,
) -> impl IntoResponse {
    let domain = host.hostname();
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
    use axum_test::{TestResponse, TestServer};
    use tracing_test::traced_test;

    #[traced_test]
    #[tokio::test]
    async fn test_middleware_logging() {
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

        let server = TestServer::new(app);

        let response: TestResponse = server
            .get("/test")
            .add_header("Host", "example.com")
            .add_header("User-Agent", "test-agent")
            .add_header("Referer", "https://example.com/")
            .await;

        response.assert_status_ok();

        // Give a moment for the log to be written
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;

        assert!(logs_contain("method=GET"));
        assert!(logs_contain("request_uri=/test"));
        assert!(logs_contain("domain=example.com"));
        assert!(logs_contain("status=200"));
        assert!(logs_contain("http_user_agent=test-agent"));
        assert!(logs_contain("http_referrer=https://example.com/"));
        assert!(logs_contain("response_time="));
    }
}
