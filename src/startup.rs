use std::net::TcpListener;
use std::time::Duration;

use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Pool, Postgres};

use crate::configuration::{DatabaseSettings, EmailClientSettings, Settings};
use crate::email_client::EmailClient;
use crate::routes;
use tracing_actix_web::TracingLogger;

pub struct Application {
    server: Server,
    port: u16,
}

impl Application {
    pub async fn build(configuration: &Settings) -> Result<Self, std::io::Error> {
        let db_connection_pool = create_db_connection_pool(&configuration.database).await;
        migrate_db(&db_connection_pool).await;

        let email_client = create_email_client(&configuration.email_client);
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );
        let tcp_listener = TcpListener::bind(address).expect("Failed to bind the address.");

        Application::initialize(tcp_listener, db_connection_pool, email_client)
    }

    fn initialize(
        tcp_listener: TcpListener,
        connection_pool: PgPool,
        email_client: EmailClient,
    ) -> Result<Self, std::io::Error> {
        let connection_pool = web::Data::new(connection_pool);
        let email_client = web::Data::new(email_client);

        let port = tcp_listener.local_addr().unwrap().port();

        let server = HttpServer::new(move || {
            App::new()
                .wrap(TracingLogger::default())
                .route("/health", web::get().to(routes::health_check))
                .route("/subscriptions", web::post().to(routes::subscribe))
                .app_data(connection_pool.clone())
                .app_data(email_client.clone())
        })
        .listen(tcp_listener)?
        .run();

        Ok(Self { server, port })
    }

    pub fn get_port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}

pub fn create_email_client(config: &EmailClientSettings) -> EmailClient {
    let sender_email = config.sender().expect("Invalid sender email address");
    EmailClient::new(
        config.base_url.clone(),
        sender_email,
        config.authorization_token.clone(),
        Duration::from_millis(config.timeout_millis),
    )
}

pub async fn create_db_connection_pool(config: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .connect_timeout(Duration::from_secs(config.connection_timeout.into()))
        .connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres DB.")
}

#[tracing::instrument(name = "Migrating database", skip(db_connection_pool))]
pub async fn migrate_db(db_connection_pool: &Pool<Postgres>) {
    sqlx::migrate!("./migrations")
        .run(db_connection_pool)
        .await
        .expect("Failed to migrate database.");
}
