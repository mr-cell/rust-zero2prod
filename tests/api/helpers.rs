use once_cell::sync::Lazy;
use rust_zero2prod::configuration::{get_configuration, TracingSettings};
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use secrecy::Secret;
use sqlx::PgPool;
use std::collections::HashMap;
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

struct DbContainerSettings {
    username: String,
    password: String,
    db_name: String,
}

pub struct TestApp<'d> {
    pub address: String,
    pub db_pool: PgPool,
    _db_container: Container<'d, Cli, Postgres>,
}

impl TestApp<'_> {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request")
    }
}

pub async fn spawn_app<'d>() -> Box<TestApp<'d>> {
    Lazy::force(&TRACING);

    let db_username = "postgres";
    let db_password = "password";

    let db_container_settings = DbContainerSettings {
        username: db_username.into(),
        password: db_password.into(),
        db_name: "newsletter".into(),
    };
    let (db_container, db_port) = configure_db_container(&db_container_settings);

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.port = db_port;
        c.database.username = Secret::new(db_username.into());
        c.database.password = Secret::new(db_password.into());
        c.application.port = 0;
        c
    };

    let app = rust_zero2prod::startup::Application::build(&configuration)
        .await
        .expect("Could not start the application.");
    let db_pool = rust_zero2prod::startup::create_db_connection_pool(&configuration.database).await;
    let address = format!("http://127.0.0.1:{}", app.get_port());
    let _ = tokio::spawn(app.run_until_stopped());

    Box::new(TestApp {
        address,
        db_pool,
        _db_container: db_container,
    })
}

fn configure_db_container<'d>(
    db_configuration: &DbContainerSettings,
) -> (Container<'d, Cli, Postgres>, u16) {
    let docker = Lazy::force(&DOCKER);
    let env_vars: HashMap<String, String> = [
        ("POSTGRES_USER", db_configuration.username.as_str()),
        ("POSTGRES_PASSWORD", db_configuration.password.as_str()),
        ("POSTGRES_DB", db_configuration.db_name.as_str()),
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
