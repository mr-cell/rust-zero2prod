use actix_web::{App, HttpResponse, HttpServer, Responder, web};
use actix_web::dev::Server;
use std::net::TcpListener;

async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}

pub fn run(tcp_listener: TcpListener) -> Result<Server, std::io::Error> {
    let server = HttpServer::new(|| {
        App::new()
            .route("/health", web::get().to(health_check))
    })
        .listen(tcp_listener)?
        .run();

    Ok(server)
}