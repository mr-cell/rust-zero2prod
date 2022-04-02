use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[actix_rt::test]
async fn confirmations_without_token_are_rejected_with_a_400() {
    // given
    let app = spawn_app().await;

    // when
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();

    // then
    assert_eq!(response.status().as_u16(), 400);
}

#[actix_rt::test]
async fn the_link_returned_by_subscribe_returns_200_when_called() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(response.status().as_u16(), 200);

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = &app.get_confirmation_links(email_request);

    // when
    let response = reqwest::get(confirmation_links.html.clone()).await.unwrap();

    // then
    assert_eq!(response.status().as_u16(), 200);
}

#[actix_rt::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = &app.get_confirmation_links(email_request);

    // when
    reqwest::get(confirmation_links.html.clone())
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    // then
    let saved = &app.get_saved_subscription("ursula_le_guin@gmail.com").await;
    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "CONFIRMED");
}
