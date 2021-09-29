use std::net::TcpListener;

use actix_web::dev::Server;
use actix_web::{web, App, HttpServer};
use sqlx::PgPool;

use crate::email_client::EmailClient;
use crate::routes;
use tracing_actix_web::TracingLogger;

pub fn run(
    tcp_listener: TcpListener,
    connection_pool: PgPool,
    email_client: EmailClient,
) -> Result<Server, std::io::Error> {
    let connection_pool = web::Data::new(connection_pool);
    let email_client = web::Data::new(email_client);
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

    Ok(server)
}
