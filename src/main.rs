use rust_zero2prod::configuration::get_configuration;
use rust_zero2prod::startup::Application;
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};

#[cfg(not(tarpaulin_include))]
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let tracing_subscriber = get_tracing_subscriber(&configuration.tracing, std::io::stdout);
    init_tracing_subscriber(tracing_subscriber);

    let app = Application::build(&configuration)
        .await
        .expect("Failed to initialize the application.");
    app.run_until_stopped().await
}
