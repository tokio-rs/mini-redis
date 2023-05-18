//! mini-redis server.
//!
//! This file is the entry point for the server implemented in the library. It
//! performs command line parsing and passes the arguments on to
//! `mini_redis::server`.
//!
//! The `clap` crate is used for parsing arguments.

use mini_redis::{server, DEFAULT_PORT};

use clap::Parser;
use tokio::net::TcpListener;
use tokio::signal;
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer,
};

#[cfg(feature = "otel")]
// To be able to set the XrayPropagator
use opentelemetry::global;
#[cfg(feature = "otel")]
// To configure certain options such as sampling rate
use opentelemetry::sdk::trace as sdktrace;
#[cfg(feature = "otel")]
// For passing along the same XrayId across services
use opentelemetry_aws::trace::XrayPropagator;

#[tokio::main]
pub async fn main() -> mini_redis::Result<()> {
    set_up_logging()?;

    let cli = Cli::parse();
    let port = cli.port.unwrap_or(DEFAULT_PORT);

    // Bind a TCP listener
    let listener = TcpListener::bind(&format!("127.0.0.1:{}", port)).await?;

    server::run(listener, signal::ctrl_c()).await;

    Ok(())
}

#[derive(Parser, Debug)]
#[clap(name = "mini-redis-server", version, author, about = "A Redis server")]
struct Cli {
    #[clap(long)]
    port: Option<u16>,
}

fn set_up_logging() -> mini_redis::Result<()> {
    // layers which we apply the `EnvFilter` to (applying it to the
    // `ConsoleLayer` will break tokio-console).
    let filtered_layers = fmt::Layer::default();

    #[cfg(feature = "otel")]
    let filtered_layers = {
        // ... set up all the opentelemetry stuff ...
        // Set the global propagator to X-Ray propagator
        // Note: If you need to pass the x-amzn-trace-id across services in the same trace,
        // you will need this line. However, this requires additional code not pictured here.
        // For a full example using hyper, see:
        // https://github.com/open-telemetry/opentelemetry-rust/blob/main/examples/aws-xray/src/server.rs#L14-L26
        global::set_text_map_propagator(XrayPropagator::default());

        let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(opentelemetry_otlp::new_exporter().tonic())
        .with_trace_config(
                sdktrace::config()
                .with_sampler(sdktrace::Sampler::AlwaysOn)
                // Needed in order to convert the trace IDs into an Xray-compatible format
                .with_id_generator(sdktrace::XrayIdGenerator::default()),
        )
        .install_simple()
        .expect("Unable to initialize OtlpPipeline");

        // Create a tracing layer with the configured tracer
        let opentelemetry = tracing_opentelemetry::layer().with_tracer(tracer);
        // combine the `fmt` and `opentelemetry` layers so we
        // can apply the `EnvFilter` to both of them
        filtered_layers.and_then(opentelemetry)
    };

    // Parse an `EnvFilter` configuration from the `RUST_LOG`
    // environment variable.
    let filter = EnvFilter::from_default_env();
    let filtered_layers = filtered_layers.with_filter(filter);

    // Use the tracing subscriber `Registry`, or any other subscriber
    // that impls `LookupSpan`
    let registry = tracing_subscriber::registry().with(filtered_layers);

    // Add a `tokio-console` layer if enabled.
    #[cfg(feature = "console")]
    let registry = registry.with(console_subscriber::spawn());

    // set the subscriber as the default
    registry.try_init()?;

    Ok(())
}
