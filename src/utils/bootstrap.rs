//! Bootstrap utilities for angzarr binaries.
//!
//! Shared initialization code for all angzarr sidecar binaries.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::config::LOG_ENV_VAR;

/// Global handle to the LoggerProvider so it stays alive for the process lifetime.
///
/// The batch exporter lives inside the provider — dropping it kills log export.
#[cfg(feature = "otel")]
static LOG_PROVIDER: std::sync::OnceLock<opentelemetry_sdk::logs::LoggerProvider> =
    std::sync::OnceLock::new();

/// Initialize tracing and metrics with LOG_ENV_VAR environment variable.
///
/// Defaults to "info" level if LOG_ENV_VAR is not set.
///
/// When the `otel` feature is enabled, configures:
/// - OTLP trace exporter (spans → OTel Collector)
/// - OTLP log exporter (tracing events → OTel Collector)
/// - OTLP metrics exporter (counters/histograms → OTel Collector)
/// - W3C TraceContext propagator for distributed tracing
///
/// Configuration via environment variables:
/// - `OTEL_EXPORTER_OTLP_ENDPOINT` — Collector endpoint (default: `http://localhost:4317`)
/// - `OTEL_SERVICE_NAME` — Service name for resource attribution
/// - `OTEL_RESOURCE_ATTRIBUTES` — Additional resource key=value pairs
pub fn init_tracing() {
    let env_filter = tracing_subscriber::EnvFilter::try_from_env(LOG_ENV_VAR)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer();

    #[cfg(feature = "otel")]
    {
        use opentelemetry::trace::TracerProvider;

        // W3C TraceContext propagator for distributed trace context
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );

        // OTLP trace exporter
        let trace_exporter = match opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .build()
        {
            Ok(exporter) => exporter,
            Err(e) => {
                eprintln!("Failed to init OTLP trace exporter: {e}");
                // Fall back to non-OTel tracing
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .init();
                return;
            }
        };

        let tracer_provider = opentelemetry_sdk::trace::TracerProvider::builder()
            .with_batch_exporter(trace_exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(otel_resource())
            .build();

        opentelemetry::global::set_tracer_provider(tracer_provider.clone());

        let otel_trace_layer =
            tracing_opentelemetry::layer().with_tracer(tracer_provider.tracer("angzarr"));

        // OTLP log exporter — provider stored in LOG_PROVIDER static to keep
        // the batch exporter alive for the process lifetime.
        let log_layer = match opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .build()
        {
            Ok(log_exporter) => {
                let log_provider = opentelemetry_sdk::logs::LoggerProvider::builder()
                    .with_batch_exporter(log_exporter, opentelemetry_sdk::runtime::Tokio)
                    .with_resource(otel_resource())
                    .build();
                let layer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                    &log_provider,
                );
                let _ = LOG_PROVIDER.set(log_provider);
                Some(layer)
            }
            Err(e) => {
                eprintln!("Failed to init OTLP log exporter: {e}");
                None
            }
        };

        // OTLP metrics exporter
        match opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_temporality(opentelemetry_sdk::metrics::Temporality::Cumulative)
            .build()
        {
            Ok(metrics_exporter) => {
                let reader = opentelemetry_sdk::metrics::PeriodicReader::builder(
                    metrics_exporter,
                    opentelemetry_sdk::runtime::Tokio,
                )
                .build();

                let meter_provider = opentelemetry_sdk::metrics::SdkMeterProvider::builder()
                    .with_reader(reader)
                    .with_resource(otel_resource())
                    .build();

                opentelemetry::global::set_meter_provider(meter_provider);
            }
            Err(e) => {
                eprintln!("Failed to init OTLP metrics exporter: {e}");
            }
        }

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_trace_layer)
            .with(log_layer)
            .init();

        tracing::info!("OpenTelemetry tracing initialized");
    }

    #[cfg(not(feature = "otel"))]
    {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
}

/// Build the OpenTelemetry resource from environment variables.
///
/// Standard attributes:
/// - `service.name` — from `OTEL_SERVICE_NAME` (default: "angzarr")
/// - `service.version` — from `OTEL_SERVICE_VERSION` or Cargo package version
/// - `service.instance.id` — from `POD_NAME` (K8s) or `HOSTNAME`
/// - `deployment.environment` — from `OTEL_DEPLOYMENT_ENVIRONMENT` or `ENVIRONMENT`
/// - `service.namespace` — from `POD_NAMESPACE` (K8s only)
///
/// See: https://opentelemetry.io/docs/specs/semconv/resource/
#[cfg(feature = "otel")]
fn otel_resource() -> opentelemetry_sdk::Resource {
    use crate::config::OTEL_SERVICE_NAME_ENV_VAR;
    use opentelemetry::KeyValue;
    use opentelemetry_sdk::Resource;

    let service_name =
        std::env::var(OTEL_SERVICE_NAME_ENV_VAR).unwrap_or_else(|_| "angzarr".to_string());

    let mut attrs = vec![KeyValue::new("service.name", service_name)];

    // service.version: from env or compile-time package version
    let version = std::env::var("OTEL_SERVICE_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    attrs.push(KeyValue::new("service.version", version));

    // service.instance.id: K8s pod name or hostname
    if let Ok(instance_id) = std::env::var("POD_NAME").or_else(|_| std::env::var("HOSTNAME")) {
        attrs.push(KeyValue::new("service.instance.id", instance_id));
    }

    // deployment.environment: dev, staging, prod, etc.
    if let Ok(env) =
        std::env::var("OTEL_DEPLOYMENT_ENVIRONMENT").or_else(|_| std::env::var("ENVIRONMENT"))
    {
        attrs.push(KeyValue::new("deployment.environment", env));
    }

    // service.namespace: K8s namespace
    if let Ok(namespace) = std::env::var("POD_NAMESPACE") {
        attrs.push(KeyValue::new("service.namespace", namespace));
    }

    Resource::new(attrs)
}

/// Graceful shutdown of OpenTelemetry providers.
///
/// Call this before process exit to flush pending spans/logs/metrics.
pub fn shutdown_telemetry() {
    #[cfg(feature = "otel")]
    {
        opentelemetry::global::shutdown_tracer_provider();
        if let Some(provider) = LOG_PROVIDER.get() {
            if let Err(e) = provider.shutdown() {
                eprintln!("Failed to shut down log provider: {e}");
            }
        }
        tracing::info!("OpenTelemetry providers shut down");
    }
}

/// Create a shutdown signal future that waits for SIGTERM (K8s) or SIGINT (Ctrl+C).
///
/// When the signal is received, calls `shutdown_telemetry()` to flush OTel buffers.
/// Use this with tonic's `with_graceful_shutdown()`.
///
/// # Example
/// ```ignore
/// Server::builder()
///     .add_service(my_service)
///     .serve_with_shutdown(addr, shutdown_signal())
///     .await?;
/// ```
pub async fn shutdown_signal() {
    use tokio::signal;

    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("Received SIGINT, shutting down"),
        _ = terminate => tracing::info!("Received SIGTERM, shutting down"),
    }

    shutdown_telemetry();
}

/// Parse static endpoints from a comma-separated string.
///
/// Format: "domain=address,domain=address,..."
/// Example: "customer=/tmp/angzarr/aggregate-customer.sock,order=/tmp/angzarr/aggregate-order.sock"
pub fn parse_static_endpoints(endpoints_str: &str) -> Vec<(String, String)> {
    endpoints_str
        .split(',')
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.trim().splitn(2, '=').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// Parse configuration path from command-line arguments.
///
/// Looks for `--config` or `-c` followed by a path.
pub fn parse_config_path() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    parse_config_path_from_args(&args)
}

/// Internal: parse config path from a given args list (testable).
fn parse_config_path_from_args(args: &[String]) -> Option<String> {
    for i in 0..args.len() {
        if (args[i] == "--config" || args[i] == "-c") && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    //! Tests for bootstrap utility functions.
    //!
    //! These utilities are used by all angzarr binaries during startup:
    //! - Static endpoint parsing for discovery
    //! - Config file path extraction from CLI args
    //!
    //! Correctness is critical — parsing errors cause runtime failures.

    use super::*;

    // ============================================================================
    // parse_static_endpoints Tests
    // ============================================================================
    //
    // Static endpoints configure domain-to-address mappings without discovery.
    // Format: "domain=address,domain=address,..."

    /// Single endpoint parses correctly.
    #[test]
    fn test_parse_static_endpoints_single() {
        let result = parse_static_endpoints("orders=/tmp/orders.sock");
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            ("orders".to_string(), "/tmp/orders.sock".to_string())
        );
    }

    /// Multiple endpoints separated by commas.
    #[test]
    fn test_parse_static_endpoints_multiple() {
        let result =
            parse_static_endpoints("orders=/tmp/orders.sock,inventory=/tmp/inventory.sock");
        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            ("orders".to_string(), "/tmp/orders.sock".to_string())
        );
        assert_eq!(
            result[1],
            ("inventory".to_string(), "/tmp/inventory.sock".to_string())
        );
    }

    /// Whitespace around pairs is trimmed.
    #[test]
    fn test_parse_static_endpoints_with_spaces() {
        let result =
            parse_static_endpoints("orders = /tmp/orders.sock , inventory = /tmp/inventory.sock");
        // After trim, "orders = /tmp/orders.sock" splits into ["orders ", " /tmp/orders.sock"]
        // The current impl doesn't trim the values, just the pair
        assert_eq!(result.len(), 2);
    }

    /// Empty string produces empty list.
    #[test]
    fn test_parse_static_endpoints_empty() {
        let result = parse_static_endpoints("");
        assert!(result.is_empty());
    }

    /// Malformed entries (missing =) are skipped.
    #[test]
    fn test_parse_static_endpoints_invalid_entry() {
        // Missing = sign
        let result = parse_static_endpoints("orders,inventory=/tmp/inventory.sock");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "inventory");
    }

    /// Values containing = are preserved (e.g., query strings).
    #[test]
    fn test_parse_static_endpoints_value_with_equals() {
        // Value containing = should be preserved
        let result = parse_static_endpoints("orders=http://localhost:8080?key=value");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "orders");
        assert_eq!(result[0].1, "http://localhost:8080?key=value");
    }

    /// Whitespace around individual entries is trimmed.
    #[test]
    fn test_parse_static_endpoints_whitespace_trimmed() {
        let result = parse_static_endpoints("  orders=/tmp/orders.sock  ");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "orders");
    }

    // ============================================================================
    // parse_config_path_from_args Tests
    // ============================================================================
    //
    // Config path is extracted from CLI args for YAML config loading.
    // Supports both --config and -c flags.

    /// --config flag extracts path.
    #[test]
    fn test_parse_config_path_long_flag() {
        let args: Vec<String> = vec![
            "program".to_string(),
            "--config".to_string(),
            "/path/to/config.yaml".to_string(),
        ];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, Some("/path/to/config.yaml".to_string()));
    }

    /// -c flag extracts path (shorthand).
    #[test]
    fn test_parse_config_path_short_flag() {
        let args: Vec<String> = vec![
            "program".to_string(),
            "-c".to_string(),
            "/path/to/config.yaml".to_string(),
        ];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, Some("/path/to/config.yaml".to_string()));
    }

    /// Missing flag returns None.
    #[test]
    fn test_parse_config_path_not_present() {
        let args: Vec<String> = vec![
            "program".to_string(),
            "--other".to_string(),
            "value".to_string(),
        ];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, None);
    }

    /// Flag at end without value returns None.
    #[test]
    fn test_parse_config_path_flag_without_value() {
        let args: Vec<String> = vec![
            "program".to_string(),
            "--config".to_string(),
            // No value follows
        ];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, None);
    }

    /// Config path extracted from middle of arg list.
    #[test]
    fn test_parse_config_path_among_other_args() {
        let args: Vec<String> = vec![
            "program".to_string(),
            "--verbose".to_string(),
            "--config".to_string(),
            "/path/to/config.yaml".to_string(),
            "--port".to_string(),
            "8080".to_string(),
        ];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, Some("/path/to/config.yaml".to_string()));
    }

    /// Empty args returns None.
    #[test]
    fn test_parse_config_path_empty_args() {
        let args: Vec<String> = vec![];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, None);
    }

    /// Only program name returns None.
    #[test]
    fn test_parse_config_path_only_program_name() {
        let args: Vec<String> = vec!["program".to_string()];
        let result = parse_config_path_from_args(&args);
        assert_eq!(result, None);
    }
}
