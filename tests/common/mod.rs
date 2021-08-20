use once_cell::sync::Lazy;
use rust_zero2prod::configuration::{get_configuration, DatabaseSettings};
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use sqlx::{Connection, Executor, PgConnection, PgPool};
use std::net::TcpListener;
use uuid::Uuid;

static TRACING: Lazy<()> = Lazy::new(|| {
    let tracing_subscriber_name = "test".into();
    let default_filter_level = "debug".into();

    if std::env::var("TEST_LOG").is_ok() {
        let tracing_subscriber = get_tracing_subscriber(
            tracing_subscriber_name,
            default_filter_level,
            std::io::stdout,
        );
        init_tracing_subscriber(tracing_subscriber);
    } else {
        let tracing_subscriber =
            get_tracing_subscriber(tracing_subscriber_name, default_filter_level, std::io::sink);
        init_tracing_subscriber(tracing_subscriber);
    };
});

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
}

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);
    let localhost = "127.0.0.1";
    let tcp_listener =
        TcpListener::bind(format!("{}:0", localhost)).expect("Failed to bind random port.");
    let port = tcp_listener.local_addr().unwrap().port();
    let address = format!("http://{}:{}", localhost, port);

    let configuration = get_configuration().expect("Failed to read configuration");
    let connection_pool = configure_database(&configuration.database).await;
    let server = rust_zero2prod::startup::run(tcp_listener, connection_pool.clone())
        .expect("Failed to bind the address.");
    let _ = tokio::spawn(server);

    TestApp {
        address,
        db_pool: connection_pool,
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let random_database_name = Uuid::new_v4().to_string();
    let mut connection = PgConnection::connect(&config.connection_string_without_db())
        .await
        .expect("Failed to connect to Postgres DB.");
    connection
        .execute(&*format!(r#"CREATE DATABASE "{}";"#, random_database_name))
        .await
        .expect("Failed to create new database.");

    let connection_pool = PgPool::connect(&*format!(
        "{}/{}",
        config.connection_string_without_db(),
        random_database_name
    ))
    .await
    .expect("Failed to connect to Postgres DB.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database.");

    connection_pool
}
