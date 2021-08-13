use rust_zero2prod::run;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let tcp_listener =
        std::net::TcpListener::bind("127.0.0.1:8000").expect("Failed to bind the address.");
    run(tcp_listener)?.await
}
