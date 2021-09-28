use crate::configuration::TracingSettings;
use tracing::subscriber::set_global_default;
use tracing::Subscriber;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::{EnvFilter, Registry};

pub fn get_tracing_subscriber(
    tracing_config: &TracingSettings,
    sink: impl MakeWriter + Send + Sync + 'static,
) -> impl Subscriber + Send + Sync {
    opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&tracing_config.log_level));
    let formatting_layer =
        BunyanFormattingLayer::new(String::from(&tracing_config.service_name), sink);

    let tracer = opentelemetry_jaeger::new_pipeline()
        .with_service_name(&tracing_config.service_name)
        .with_agent_endpoint(format!("{}:{}", &tracing_config.host, tracing_config.port))
        .install_batch(opentelemetry::runtime::Tokio)
        .expect("Error initializing Jaeger exporter.");
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    Registry::default()
        .with(env_filter)
        .with(JsonStorageLayer)
        .with(telemetry)
        .with(formatting_layer)
}

pub fn init_tracing_subscriber(tracing_subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to set logger");
    set_global_default(tracing_subscriber).expect("Failed to set tracing subscriber");
}
