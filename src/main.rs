use rust_zero2prod::configuration::get_configuration;
use rust_zero2prod::startup::run;
use std::net::TcpListener;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let configuration = get_configuration().expect("Failed to read configuration");
    println!("configuration: {}", configuration.database.password);
    let address = format!("127.0.0.1:{}", configuration.application_port);
    let tcp_listener = TcpListener::bind(address).expect("Failed to bind the address.");
    run(tcp_listener)?.await
}
