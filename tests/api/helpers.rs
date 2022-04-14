use once_cell::sync::Lazy;
use rust_zero2prod::configuration::{get_configuration, TracingSettings};
use rust_zero2prod::telemetry::{get_tracing_subscriber, init_tracing_subscriber};
use secrecy::Secret;
use sqlx::postgres::PgArguments;
use sqlx::{Arguments, PgPool};
use std::collections::HashMap;
use testcontainers::clients::Cli;
use testcontainers::core::Port;
use testcontainers::images::postgres::Postgres;
use testcontainers::{clients, images, Container, Docker, RunArgs};
use wiremock::MockServer;

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

pub struct ConfirmationLinks {
    pub html: reqwest::Url,
    pub plain: reqwest::Url,
}

pub struct EmailBody {
    pub html: String,
    pub plain: String,
}

#[derive(sqlx::FromRow)]
pub struct SubscriptionDetails {
    pub email: String,
    pub name: String,
    pub status: String,
}

pub struct TestApp<'d> {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub email_server: MockServer,
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

    pub async fn post_newsletters(&self, body: serde_json::Value) -> reqwest::Response {
        reqwest::Client::new()
            .post(format!("{}/newsletters", &self.address))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .expect("Failed to send the request.")
    }

    pub async fn get_saved_subscription(&self, email: &str) -> SubscriptionDetails {
        let mut args = PgArguments::default();
        args.add(email);
        sqlx::query_as_with::<_, SubscriptionDetails, PgArguments>(
            "SELECT email, name, status FROM subscriptions WHERE email = $1",
            args,
        )
        .fetch_one(&self.db_pool)
        .await
        .expect("Failed to fetch saved subscriptions")
    }

    pub fn get_email_body(&self, email_request: &wiremock::Request) -> EmailBody {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let html_body = jsonpath_lib::selector(&body)("$.content[?(@.type == 'text/html')].value")
            .unwrap()[0]
            .as_str()
            .unwrap();
        let text_body = jsonpath_lib::selector(&body)("$.content[?(@.type == 'text/plain')].value")
            .unwrap()[0]
            .as_str()
            .unwrap();

        EmailBody {
            html: html_body.to_string(),
            plain: text_body.to_string(),
        }
    }

    pub fn get_confirmation_links(&self, email_request: &wiremock::Request) -> ConfirmationLinks {
        let get_link = |s: &str| -> reqwest::Url {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();

            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();

            confirmation_link
        };

        let email_body = self.get_email_body(email_request);

        let html_link = get_link(email_body.html.as_str());
        let text_link = get_link(email_body.plain.as_str());

        ConfirmationLinks {
            html: html_link,
            plain: text_link,
        }
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

    let email_server = MockServer::start().await;

    let configuration = {
        let mut c = get_configuration().expect("Failed to read configuration");
        c.database.port = db_port;
        c.database.username = Secret::new(db_username.into());
        c.database.password = Secret::new(db_password.into());
        c.application.port = 0;
        c.email_client.base_url = email_server.uri();
        c
    };

    let app = rust_zero2prod::startup::Application::build(&configuration)
        .await
        .expect("Could not start the application.");
    let db_pool = rust_zero2prod::startup::create_db_connection_pool(&configuration.database).await;
    let port = app.get_port();
    let address = format!("http://127.0.0.1:{}", port);
    let _ = tokio::spawn(app.run_until_stopped());

    Box::new(TestApp {
        address,
        port,
        db_pool,
        email_server,
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
