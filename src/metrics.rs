use axum::{extract::Request, http::StatusCode, middleware::Next, response::IntoResponse};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::time::{Duration, Instant};

const BUCKET_VALUES: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 10.0,
];

pub fn setup_metrics() -> PrometheusHandle {
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
