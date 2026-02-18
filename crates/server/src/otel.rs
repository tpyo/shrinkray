use crate::config::{Config, LogFormat};
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::SpanExporter;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::{SdkTracerProvider, TracerProviderBuilder};
use std::sync::OnceLock;
use tracing::Level;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{EnvFilter, Layer, layer::SubscriberExt, util::SubscriberInitExt};

fn get_resource() -> Resource {
    static RESOURCE: OnceLock<Resource> = OnceLock::new();
    RESOURCE
        .get_or_init(|| Resource::builder().with_service_name("shrinkray").build())
        .clone()
}

fn get_env_filter() -> EnvFilter {
    EnvFilter::from_default_env()
        .add_directive("hyper=off".parse().unwrap())
        .add_directive("tonic=off".parse().unwrap())
        .add_directive("h2=off".parse().unwrap())
        .add_directive("opentelemetry=info".parse().unwrap())
        .add_directive("reqwest=off".parse().unwrap())
}

fn setup_tracing_provider(config: &Config) -> TracerProviderBuilder {
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

    provider
}

pub fn setup_tracing(config: &Config) -> SdkTracerProvider {
    let tracer_provider = setup_tracing_provider(config).build();

    let tracer = tracer_provider.tracer("shrinkray");

    let logging_layer: Box<dyn tracing_subscriber::Layer<_> + Send + Sync> =
        match config.log_format {
            LogFormat::Default => tracing_subscriber::fmt::layer()
                .with_thread_names(true)
                .with_filter(get_env_filter())
                .boxed(),
            LogFormat::Json => tracing_subscriber::fmt::layer()
                .json()
                .with_filter(get_env_filter())
                .boxed(),
            LogFormat::Logfmt => tracing_logfmt::builder()
                .layer()
                .with_filter(get_env_filter())
                .boxed(),
        };

    let otel_layer = OpenTelemetryLayer::new(tracer).with_filter(
        tracing_subscriber::filter::LevelFilter::from_level(Level::INFO),
    );

    let _ = tracing_subscriber::registry()
        .with(logging_layer)
        .with(otel_layer)
        .try_init();

    tracer_provider
}

#[cfg(test)]
mod tests {
    use super::*;
    use opentelemetry::trace::Span;
    use opentelemetry::trace::Tracer;
    use opentelemetry_sdk::error::OTelSdkResult;
    use opentelemetry_sdk::trace::{
        BatchConfigBuilder, BatchSpanProcessor, SpanData, SpanExporter,
    };
    use regex::Regex;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    #[derive(Debug)]
    struct MockSpanExporter {
        exported_spans: Arc<Mutex<Vec<SpanData>>>,
    }

    impl MockSpanExporter {
        fn new() -> Self {
            Self {
                exported_spans: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl SpanExporter for MockSpanExporter {
        async fn export(&self, batch: Vec<SpanData>) -> OTelSdkResult {
            self.exported_spans.lock().unwrap().extend(batch);
            Ok(())
        }

        fn shutdown(&mut self) -> OTelSdkResult {
            Ok(())
        }
    }

    #[test]
    fn test_get_env_filter() {
        let filter = get_env_filter();
        let filter_str = filter.to_string();
        let re = Regex::new(r"^([a-z0-9_]+(=[^,]+)?)(,([a-z0-9_]+(=[^,]+)?))*$").unwrap();
        assert!(re.is_match(&filter_str));
    }

    #[test]
    fn test_get_resource() {
        let resource = get_resource();

        // Test that resource contains the service name
        let service_name = resource.get(&opentelemetry::Key::new("service.name"));
        assert!(service_name.is_some());
        assert_eq!(service_name.unwrap().as_str(), "shrinkray");

        // Test resource is consistent (using OnceLock)
        let resource2 = get_resource();
        assert_eq!(resource.len(), resource2.len());
    }

    #[tokio::test]
    async fn test_setup_tracing() {
        let config = Config::default();

        let provider = setup_tracing(&config);

        let tracer = provider.tracer("shrinkray");
        let mut span = tracer.start("test_span");
        span.set_attribute(opentelemetry::KeyValue::new("key", "value"));
        span.end();

        assert!(span.span_context().is_valid());
        assert!(provider.force_flush().is_ok());
        drop(tracer);
        assert!(provider.shutdown().is_ok());
    }

    #[tokio::test]
    async fn test_tracing_provider() {
        let mock_exporter = MockSpanExporter::new();
        let exported_spans = mock_exporter.exported_spans.clone();

        let batch_config = BatchConfigBuilder::default()
            .with_max_queue_size(10)
            .with_max_export_batch_size(10)
            .with_scheduled_delay(Duration::from_millis(100))
            .build();

        let processor = BatchSpanProcessor::new(mock_exporter, batch_config);

        let config = Config::default();
        let provider = setup_tracing_provider(&config)
            .with_span_processor(processor)
            .build();

        let tracer = provider.tracer("shrinkray");

        let mut span = tracer.start("test_span");
        span.set_attribute(opentelemetry::KeyValue::new("key", "value"));
        span.add_event(
            "test_event",
            vec![opentelemetry::KeyValue::new("event_key", "event_value")],
        );

        assert!(span.is_recording());
        span.end();
        assert!(!span.is_recording());

        let _ = provider.force_flush();

        let exported = exported_spans.lock().unwrap();
        assert_eq!(exported.len(), 1);
        assert_eq!(exported[0].name, "test_span");
        assert_eq!(exported[0].attributes.len(), 1);
        assert_eq!(exported[0].events.len(), 1);
        assert_eq!(exported[0].events[0].name, "test_event");

        drop(tracer);
        assert!(provider.shutdown().is_ok());
    }
}
