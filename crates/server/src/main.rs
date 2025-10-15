mod backend;
mod config;
mod error;
mod http;
mod logging;
mod metrics;
mod otel;
mod service;

use shrinkray::{image, options};

use axum::{
    Router,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware,
    response::IntoResponse,
    routing::get,
};
use opentelemetry::trace::Status;
use std::future::ready;
use std::sync::Arc;
use tracing::{debug, error};
use tracing_opentelemetry::OpenTelemetrySpanExt;

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

#[tracing::instrument(skip_all)]
async fn handle_image_request(
    State(ctx): State<Arc<Service>>,
    request_path: String,
    mut options: Query<options::ImageOptions>,
    _headers: HeaderMap,
    endpoint: String,
    route_path: String,
) -> Result<impl IntoResponse> {
    let relative_path = request_path.replacen(&route_path, "", 1);
    let target = format!("{}{}", endpoint, relative_path);

    let image = get_file_from_backend(&target, &ctx.config)
        .await
        .inspect_err(|err| {
            tracing::Span::current().set_status(Status::Error {
                description: err.to_string().into(),
            });
            error!("failed to fetch image from backend: {}", err)
        })?;

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

    let span = tracing::Span::current();
    let image_config = ctx.config.to_image_config();
    rayon::spawn(move || {
        let result = image::process_image(&image, &mut options, &image_config, span.clone())
            .and_then(|vips_image| {
                let start = std::time::Instant::now();
                let output = image::output(&vips_image, &mut options, &image_config)?;
                metrics::output_duration(start.elapsed(), &output.content_type.to_string());
                Ok(output)
            })
            .map_err(|err| ctx.vips_error(err));
        let _ = send.send(result);
    });
    let image = recv
        .await
        .map_err(|err| {
            error::Error::Rayon(format!(
                "failed to receive image from processing thread: {}",
                err
            ))
        })?
        .inspect_err(|err| {
            tracing::Span::current().set_status(Status::Error {
                description: err.to_string().into(),
            });
        })?;

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
                handle_image_request(ctx, request_path, options, headers, endpoint, route_path)
                    .await
            }
        };

        router = router.route(&path, get(handler));
    }

    router
}

fn get_management_router() -> Router {
    let prom_handle = metrics::setup_metrics();
    Router::new()
        .route("/metrics", get(move || ready(prom_handle.render())))
        .route("/healthz", get(|| async { StatusCode::OK }))
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
    let router = get_management_router();

    let listener: tokio::net::TcpListener =
        tokio::net::TcpListener::bind(&service.config.management_address).await?;
    debug!("management listening on {}", &listener.local_addr()?);
    axum::serve(listener, router).await?;

    Ok(())
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let config = read_config();
    if config.is_err() {
        eprintln!("failed to read configuration: {}", config.unwrap_err());
        std::process::exit(1);
    }

    let service = Arc::new(Service::new(config.unwrap()));

    let tracer_provider = otel::setup_tracing(&service.config);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{self, Config, ConfigRouting};
    use crate::image::Image;
    use crate::options::ImageFormat;
    use axum::http::{HeaderValue, header};
    use axum_test::TestServer;

    #[test]
    fn test_get_headers_with_download() {
        let image = Image {
            bytes: vec![1, 2, 3],
            content_type: ImageFormat::Png,
        };

        let headers = get_headers(&image, Some("test.png".to_string())).unwrap();

        assert_eq!(
            headers.get(header::CONTENT_TYPE).unwrap(),
            &HeaderValue::from_static("image/png")
        );
        assert_eq!(
            headers.get(header::CACHE_CONTROL).unwrap(),
            &HeaderValue::from_static("public, max-age=31536000")
        );
        assert_eq!(
            headers.get(header::CONTENT_DISPOSITION).unwrap(),
            &HeaderValue::from_str("attachment; filename=\"test.png\"").unwrap()
        );
    }

    #[tokio::test]
    async fn test_invalid_route() {
        let config = Config::default();
        let service = Arc::new(Service::new(config));
        let router = get_router(Box::leak(Box::new(service.config.clone()))).with_state(service);

        let test_server = TestServer::new(router).unwrap();

        let response = test_server.get("/invalid/path").await;
        response.assert_status(StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_image_handler() {
        let mut server = mockito::Server::new_async().await;

        let config: config::Config = config::Config {
            routing: vec![ConfigRouting {
                path: "images/{*path}".to_string(),
                endpoint: format!("{}/", server.url()),
            }],
            ..Default::default()
        };

        let mock = server
            .mock("GET", "/file.jpg")
            .with_status(200)
            .with_body(b"image data")
            .create_async()
            .await;

        let service = Arc::new(Service::new(config.clone()));
        let router = get_router(Box::leak(Box::new(config))).with_state(service);

        let test_server = TestServer::new(router).unwrap();

        let response = test_server.get("/images/file.jpg").await;
        response.assert_status_ok();
        response.assert_header(header::CONTENT_TYPE, "image/jpeg");
        response.assert_text("image data");

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_management_server_healthz() {
        let router = get_management_router();
        let test_server = TestServer::new(router).unwrap();

        let response = test_server.get("/healthz").await;
        response.assert_status_ok();
    }

    #[tokio::test]
    async fn test_management_server_metrics() {
        let router = get_management_router();
        let test_server = TestServer::new(router).unwrap();

        ::metrics::counter!("shrinkray_test").increment(1);

        let response = test_server.get("/metrics").await;
        response.assert_status_ok();
        let body = response.text();
        assert!(body.contains("TYPE shrinkray_test"));
    }

    #[tokio::test]
    async fn test_run_management_server() {
        let config: config::Config = config::Config {
            server_address: "0.0.0.0:9020".parse().unwrap(),
            management_address: "0.0.0.0:9021".parse().unwrap(),
            ..Default::default()
        };

        let service = Arc::new(Service::new(config));

        let server_handle = tokio::spawn(async move {
            run_management_server(&service)
                .await
                .expect("failed to run management server");
        });

        // Wait a bit for the server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let client = reqwest::Client::new();

        let response = client.get("http://127.0.0.1:9021/healthz").send().await;

        assert!(response.is_ok());
        assert_eq!(response.unwrap().status(), StatusCode::OK);

        server_handle.abort();
    }

    #[tokio::test]
    async fn test_run_server() {
        let mut server = mockito::Server::new_async().await;

        let config: config::Config = config::Config {
            server_address: "0.0.0.0:9030".parse().unwrap(),
            management_address: "0.0.0.0:9031".parse().unwrap(),
            routing: vec![ConfigRouting {
                path: "images/{*path}".to_string(),
                endpoint: format!("{}/", server.url()),
            }],
            ..Default::default()
        };

        let mock = server
            .mock("GET", "/file.jpg")
            .with_status(200)
            .with_body(b"image data")
            .create_async()
            .await;

        let service = Arc::new(Service::new(config));
        let server_handle = tokio::spawn(async move {
            run_server(&service).await.expect("failed to run server");
        });

        // Wait a bit for the server to start
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let client = reqwest::Client::new();
        let response = client
            .get("http://127.0.0.1:9030/images/file.jpg")
            .send()
            .await;
        assert!(response.is_ok());
        let response = response.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        mock.assert_async().await;

        server_handle.abort();
    }
}
