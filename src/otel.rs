use crate::config::Config;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use std::sync::OnceLock;
use tracing_subscriber::EnvFilter;

fn get_resource() -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    RESOURCE
        .get_or_init(|| Resource::builder().with_service_name("shrinkray").build())
        .clone()
}

pub fn setup_logging() {
    let filter = EnvFilter::from_default_env()
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("opentelemetry=info".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_thread_names(true)
        .init();
}

pub fn setup_tracing(config: &Config) -> SdkTracerProvider {
    let mut exporter = SpanExporter::builder()
        .with_tonic()
        .with_timeout(std::time::Duration::from_secs(5));

    if let Some(endpoint) = &config.otel_collector_endpoint {
        exporter = exporter
            .with_protocol(opentelemetry_otlp::Protocol::Grpc)
            .with_endpoint(endpoint);
    } else {
        tracing::warn!("OTel collector endpoint is not set, tracing will not be exported");
    }

    let exporter = exporter.build().expect("failed to create span exporter");

    let mut provider = SdkTracerProvider::builder().with_resource(get_resource());

    if config.otel_collector_endpoint.is_some() {
        provider = provider.with_batch_exporter(exporter);
    } else {
        tracing::warn!("OTel collector endpoint is not set, tracing will not be exported");
    }

    provider.build()
}
