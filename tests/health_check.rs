use std::net::TcpListener;

fn spawn_app() -> String {
    let tcp_listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port.");
    let port = tcp_listener.local_addr().unwrap().port();
    let server = rust_zero2prod::run(tcp_listener).expect("Failed to bind the address.");
    let _ = tokio::spawn(server);

    format!("http://127.0.0.1:{}", port)
}

#[actix_rt::test]
async fn health_check_works() {
    let address = spawn_app();

    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health", &address))
        .send()
        .await
        .expect("Failed to execute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}

