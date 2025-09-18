use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

const BUCKET_VALUES: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 10.0,
];

pub fn setup_metrics() -> PrometheusHandle {
    static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();
    PROMETHEUS_HANDLE
        .get_or_init(|| {
            let mut builder = PrometheusBuilder::new();
            builder = builder.upkeep_timeout(Duration::from_secs(300));
            builder = builder
                .set_buckets_for_metric(
                    Matcher::Full("shrinkray_fetch_duration_seconds_bucket".to_string()),
                    BUCKET_VALUES,
                )
                .expect("error creating metric bucket");

            builder = builder
                .set_buckets_for_metric(
                    Matcher::Full("shrinkray_http_response_seconds_bucket".to_string()),
                    BUCKET_VALUES,
                )
                .expect("error creating metric bucket");

            builder
                .install_recorder()
                .expect("error installing prometheus recorder")
        })
        .clone()
}

pub async fn middleware(req: Request, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let uri = req.uri().to_string();

    if ["/", "/metrics", "/healthz", "/favicon.ico"].contains(&uri.as_str()) {
        return next.run(req).await;
    }

    let response = next.run(req).await;
    match response.status() {
        StatusCode::OK => {
            metrics::counter!("shrinkray_http_response_200").increment(1);
            let elapsed = start.elapsed().as_secs_f64();
            metrics::histogram!("shrinkray_http_response_seconds_bucket").record(elapsed);
        }
        StatusCode::UNAUTHORIZED => {
            metrics::counter!("shrinkray_http_response_401").increment(1);
        }
        StatusCode::NOT_FOUND => {
            metrics::counter!("shrinkray_http_response_404").increment(1);
        }
        StatusCode::INTERNAL_SERVER_ERROR => {
            metrics::counter!("shrinkray_http_response_500").increment(1);
        }
        _ => {}
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, http::StatusCode, middleware::from_fn, routing::get};
    use axum_test::TestServer;
    use std::future::ready;

    fn test_router() -> Router {
        let prom_handle = setup_metrics();
        Router::new()
            .route("/metrics", get(move || ready(prom_handle.render())))
            .route("/200", get(|| async { StatusCode::OK }))
            .route("/401", get(|| async { StatusCode::UNAUTHORIZED }))
            .route("/404", get(|| async { StatusCode::NOT_FOUND }))
            .route("/500", get(|| async { StatusCode::INTERNAL_SERVER_ERROR }))
            .layer(from_fn(middleware))
    }

    #[tokio::test]
    async fn test_middleware_records_metrics() {
        setup_metrics();
        let app = test_router();
        let server = TestServer::new(app).unwrap();

        server.get("/200").await;
        server.get("/401").await;
        server.get("/404").await;
        server.get("/500").await;

        let response = server.get("/metrics").await;
        response.assert_status_ok();
        let body = response.text();
        assert!(body.contains("TYPE shrinkray_http_response_200"));
        assert!(body.contains("shrinkray_http_response_200 1"));
        assert!(body.contains("TYPE shrinkray_http_response_401"));
        assert!(body.contains("shrinkray_http_response_401 1"));
        assert!(body.contains("TYPE shrinkray_http_response_404"));
        assert!(body.contains("shrinkray_http_response_404 1"));
        assert!(body.contains("TYPE shrinkray_http_response_500"));
        assert!(body.contains("shrinkray_http_response_500 1"));
    }
}
