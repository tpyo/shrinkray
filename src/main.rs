mod backend;
mod config;
mod error;
mod http;
mod image;
mod logging;
mod metrics;
mod options;
mod otel;
mod service;

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
};
use opentelemetry::trace::{Span, Status, TraceContextExt, Tracer};
use opentelemetry::{Context as TraceContext, InstrumentationScope, KeyValue, global};
use std::future::ready;
use std::sync::Arc;
use tracing::debug;

use backend::get_file_from_backend;
use config::read_config;
use error::Result;
use service::Service;

pub struct Routing {
    pub routes: Vec<Route>,
}

pub struct Route {
    pub path: String,
    pub endpoint: String,
}

fn get_headers(image: &image::Image, download: Option<String>) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(image.content_type.content_type())?,
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000"),
    );
    if let Some(filename) = download {
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename))?,
        );
    }

    Ok(headers)
}

async fn handle_image_request(
    State(ctx): State<Arc<Service>>,
    request_path: String,
    mut options: Query<options::ImageOptions>,
    _headers: HeaderMap,
    endpoint: String,
    route_path: String,
    cx: TraceContext,
) -> Result<impl IntoResponse> {
    let relative_path = request_path.replacen(&route_path, "", 1);
    let target = format!("{}{}", endpoint, relative_path);

    debug!("fetching image from backend: {}", target);
    let mut span = global::tracer("shrinkray").start_with_context("get_file_from_backend", &cx);
    let image = get_file_from_backend(&target, &ctx.config)
        .await
        .inspect_err(|err| {
            span.set_status(Status::Error {
                description: err.to_string().into(),
            });
        })?;
    span.end();

    if !options.any_set() {
        // If no options are set, return the original image
        let image = image::Image {
            bytes: image,
            content_type: options::ImageFormat::Jpeg,
        };
        return Ok((get_headers(&image, options.download.clone())?, image.bytes));
    }

    let download = options.download.clone();

    if let Some(signing_secret) = &ctx.config.signing_secret
        && !options.verify_signature(signing_secret)
    {
        return Err(error::Error::InvalidSignature);
    }

    debug!("processing image: {}", target);
    let (send, recv) = tokio::sync::oneshot::channel();
    rayon::spawn(move || {
        let span = global::tracer("shrinkray").start_with_context("process_image", &cx);
        let cx = TraceContext::current_with_span(span);
        let image = image::process_image(&image, &mut options, &ctx.config, &cx)
            .map_err(|err| ctx.vips_error(err));
        let _ = send.send(image);
    });
    let image = recv
        .await
        .map_err(|err| {
            span.set_status(Status::Error {
                description: err.to_string().into(),
            });
            error::Error::Rayon("failed to receive image from processing thread".into())
        })?
        .inspect_err(|err| {
            span.set_status(Status::Error {
                description: err.to_string().into(),
            });
        })?;

    span.set_status(Status::Ok);
    Ok((get_headers(&image, download)?, image.bytes))
}

fn get_router(config: &'static config::Config) -> Router<Arc<Service>> {
    let mut router: Router<Arc<Service>> =
        Router::new().route("/favicon.ico", get(|| async { StatusCode::NOT_FOUND }));

    for route in &config.routing {
        let path = format!("/{}", &route.path);
        let endpoint = route.endpoint.clone();
        let route_path = route.path.clone();

        let handler = move |ctx: State<Arc<Service>>,
                            Path(request_path): Path<String>,
                            options: Query<options::ImageOptions>,
                            headers: HeaderMap| {
            async move {
                let scope = InstrumentationScope::builder("basic")
                    .with_version("1.0")
                    .build();
                let tracer = global::tracer_with_scope(scope.clone());

                let mut span = tracer.start("handle_image_request");
                span.set_attributes([
                    KeyValue::new("shrinkray.request_path", request_path.clone()),
                    KeyValue::new("shrinkray.endpoint", endpoint.clone()),
                    KeyValue::new("shrinkray.route_path", route_path.clone()),
                ]);
                let cx = TraceContext::current_with_span(span);

                handle_image_request(
                    ctx,
                    request_path,
                    options,
                    headers,
                    endpoint,
                    route_path,
                    cx,
                )
                .await
            }
        };

        router = router.route(&path, get(handler));
    }

    router
}

async fn run_server(
    service: &Arc<service::Service>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let config = service.config.clone();
    let router = get_router(Box::leak(Box::new(config)))
        .route_layer(middleware::from_fn(metrics::middleware))
        .layer(middleware::from_fn_with_state(
            service.clone(),
            logging::middleware,
        ))
        .with_state(service.clone());

    let listener = tokio::net::TcpListener::bind(&service.config.server_address).await?;
    debug!("listening on {}", &listener.local_addr()?);

    axum::serve(listener, router)
        .with_graceful_shutdown(service::shutdown())
        .await?;
    Ok(())
}

async fn run_management_server(
    service: &Arc<service::Service>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let prom_handle = metrics::setup_metrics();
    let router = Router::new()
        .route("/metrics", get(move || ready(prom_handle.render())))
        .route("/healthz", get(|| async { StatusCode::OK }));

    let listener: tokio::net::TcpListener =
        tokio::net::TcpListener::bind(&service.config.management_address).await?;
    debug!("management listening on {}", &listener.local_addr()?);
    axum::serve(listener, router).await?;

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    otel::setup_logging();

    let config = read_config();
    if config.is_err() {
        eprintln!("failed to read configuration: {}", config.unwrap_err());
        std::process::exit(1);
    }

    let service = Arc::new(Service::new(config.unwrap()));

    let tracer_provider = otel::setup_tracing(&service.config);

    global::set_tracer_provider(tracer_provider.clone());

    let service_clone = service.clone();
    tokio::spawn(async move {
        run_management_server(&service_clone)
            .await
            .expect("failed to run management server");
    });

    run_server(&service).await.expect("failed to run server");

    tracer_provider
        .shutdown()
        .expect("failed to shutdown tracer provider");
}
