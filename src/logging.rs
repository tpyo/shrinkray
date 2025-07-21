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
