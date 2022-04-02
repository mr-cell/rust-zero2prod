use crate::helpers::spawn_app;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[actix_rt::test]
async fn post_subscriptions_returns_200_for_valid_form_data() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // when
    let response = app.post_subscriptions(body.into()).await;

    // then
    assert_eq!(200, response.status().as_u16());
}

#[actix_rt::test]
async fn subscribe_persists_the_new_subscriber() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // when
    app.post_subscriptions(body.into()).await;

    // then
    let saved = &app.get_saved_subscription("ursula_le_guin@gmail.com").await;

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "PENDING")
}

#[actix_rt::test]
async fn post_subscriptions_returns_400_when_fields_have_invalid_values() {
    // given
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin@gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=invalid_email", "invalid email"),
    ];

    for (body, description) in test_cases {
        // when
        let response = app.post_subscriptions(body.into()).await;

        // then
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not return 400 OK when the payload was {}",
            description
        );
    }
}

#[actix_rt::test]
async fn post_subscriptions_returns_400_for_missing_form_data() {
    // given
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=le%20guin", "missing the e-mail"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and e-mail"),
    ];

    for (invalid_body, error_message) in test_cases {
        // when
        let response = app.post_subscriptions(invalid_body.into()).await;

        // then
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}",
            error_message
        );
    }
}

#[actix_rt::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // when
    app.post_subscriptions(body.into()).await;

    // then
    // not needed - expect set on mock invocation
}

#[actix_rt::test]
async fn subscribe_fails_when_sending_confirmation_email_fails() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // when
    let response = app.post_subscriptions(body.into()).await;

    // then
    assert_eq!(response.status().as_u16(), 500);
}

#[actix_rt::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    // given
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    // when
    app.post_subscriptions(body.into()).await;

    // then
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_links = &app.get_confirmation_links(email_request);
    assert_eq!(confirmation_links.html, confirmation_links.plain);
}
