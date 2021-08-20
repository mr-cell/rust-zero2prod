use rust_zero2prod::configuration::get_configuration;
use rust_zero2prod::startup::run;
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use sqlx::PgPool;
use std::net::TcpListener;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let tracing_subscriber =
        get_tracing_subscriber("rust-zero2prod".into(), "info".into(), std::io::stdout);
    init_tracing_subscriber(tracing_subscriber);

    let db_connection_pool = PgPool::connect(&configuration.database.connection_string())
        .await
        .expect("Failed to connect to Postgres DB");

    let address = format!("127.0.0.1:{}", configuration.application_port);
    let tcp_listener = TcpListener::bind(address).expect("Failed to bind the address.");
    run(tcp_listener, db_connection_pool)?.await
}
