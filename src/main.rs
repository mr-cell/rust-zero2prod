use rust_zero2prod::configuration::get_configuration;
use rust_zero2prod::startup::run;
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};
use std::net::TcpListener;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");

    let tracing_subscriber =
        get_tracing_subscriber("rust-zero2prod".into(), "info".into(), std::io::stdout);
    init_tracing_subscriber(tracing_subscriber);

    let db_connection_pool = PgPoolOptions::new()
        .connect_timeout(std::time::Duration::from_secs(2))
        .connect_with(configuration.database.with_db())
        .await
        .expect("Failed to connect to Postgres DB");
    migrate_db(&db_connection_pool).await;

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    let tcp_listener = TcpListener::bind(address).expect("Failed to bind the address.");
    run(tcp_listener, db_connection_pool)?.await
}

#[tracing::instrument(name = "Migrating database", skip(db_connection_pool))]
async fn migrate_db(db_connection_pool: &Pool<Postgres>) {
    sqlx::migrate!("./migrations")
        .run(db_connection_pool)
        .await
        .expect("Failed to migrate database.");
}
