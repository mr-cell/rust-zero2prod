use once_cell::sync::Lazy;
use rust_zero2prod::configuration::{get_configuration, DatabaseSettings, TracingSettings};
use rust_zero2prod::email_client::EmailClient;
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use sqlx::PgPool;
use std::collections::HashMap;
use std::net::TcpListener;
use std::time::Duration;
use testcontainers::clients::Cli;
use testcontainers::core::Port;
use testcontainers::images::postgres::Postgres;
use testcontainers::{clients, images, Container, Docker, RunArgs};

static DOCKER: Lazy<Cli> = Lazy::new(|| clients::Cli::default());

static TRACING: Lazy<()> = Lazy::new(|| {
    let tracing_settings = TracingSettings {
        service_name: "test".into(),
        log_level: "debug".into(),
        host: "localhost".into(),
        port: 6831,
    };

    if std::env::var("TEST_LOG").is_ok() {
        let tracing_subscriber = get_tracing_subscriber(&tracing_settings, std::io::stdout);
        init_tracing_subscriber(tracing_subscriber);
    } else {
        let tracing_subscriber = get_tracing_subscriber(&tracing_settings, std::io::sink);
        init_tracing_subscriber(tracing_subscriber);
    };
});

pub struct TestApp<'d> {
    pub address: String,
    pub db_pool: PgPool,
    _db_container: Container<'d, Cli, Postgres>,
}

pub async fn spawn_app<'d>() -> Box<TestApp<'d>> {
    Lazy::force(&TRACING);

    let configuration = get_configuration().expect("Failed to read configuration");
    let (db_container, db_port) = configure_db_container(&configuration.database);

    let localhost = "127.0.0.1";
    let tcp_listener =
        TcpListener::bind(format!("{}:0", localhost)).expect("Failed to bind random port.");
    let port = tcp_listener.local_addr().unwrap().port();
    let address = format!("http://{}:{}", localhost, port);

    let connection_pool = configure_database(&configuration.database, db_port).await;

    let sender_email = configuration
        .email_client
        .sender()
        .expect("Invalid sender email address");
    let email_client = EmailClient::new(
        configuration.email_client.base_url,
        sender_email,
        configuration.email_client.authorization_token,
        Duration::from_millis(configuration.email_client.timeout_millis),
    );

    let server = rust_zero2prod::startup::run(tcp_listener, connection_pool.clone(), email_client)
        .expect("Failed to bind the address.");
    let _ = tokio::spawn(server);

    Box::new(TestApp {
        address,
        db_pool: connection_pool,
        _db_container: db_container,
    })
}

fn configure_db_container<'d>(
    db_configuration: &DatabaseSettings,
) -> (Container<'d, Cli, Postgres>, u16) {
    let docker = Lazy::force(&DOCKER);
    let env_vars: HashMap<String, String> = [
        ("POSTGRES_USER", db_configuration.username.as_str()),
        ("POSTGRES_PASSWORD", db_configuration.password.as_str()),
        ("POSTGRES_DB", db_configuration.database_name.as_str()),
    ]
    .iter()
    .cloned()
    .map(|tuple| (tuple.0.to_string(), tuple.1.to_string()))
    .collect();
    let postgres_image = images::postgres::Postgres::default()
        .with_version(13)
        .with_env_vars(env_vars);
    let db_port = free_local_port().expect("Could not obtain free network port");

    let db_container = docker.run_with_args(
        postgres_image,
        RunArgs::default().with_mapped_port(Port {
            local: db_port,
            internal: 5432,
        }),
    );

    (db_container, db_port)
}

fn free_local_port() -> Option<u16> {
    let socket = std::net::SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, 0);
    std::net::TcpListener::bind(socket)
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .ok()
}

async fn configure_database(config: &DatabaseSettings, db_port: u16) -> PgPool {
    let connection_pool = PgPool::connect_with(config.with_db().port(db_port))
        .await
        .expect("Failed to connect to Postgres DB.");
    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database.");

    connection_pool
}
