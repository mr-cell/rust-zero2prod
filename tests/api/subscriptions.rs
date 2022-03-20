use crate::helpers::spawn_app;
use jsonpath_lib::selector;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[derive(sqlx::FromRow)]
struct SubscriptionDetails {
    email: String,
    name: String,
    status: String,
}

#[actix_rt::test]
async fn post_subscriptions_returns_200_for_valid_form_data() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    let response = app.post_subscriptions(body.into()).await;

    assert_eq!(200, response.status().as_u16());
}

#[actix_rt::test]
async fn subscribe_persists_the_new_subscriber() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let saved =
        sqlx::query_as::<_, SubscriptionDetails>("SELECT email, name, status FROM subscriptions")
            .fetch_one(&app.db_pool)
            .await
            .expect("Failed to fetch saved subscriptions");

    assert_eq!(saved.email, "ursula_le_guin@gmail.com");
    assert_eq!(saved.name, "le guin");
    assert_eq!(saved.status, "PENDING")
}

#[actix_rt::test]
async fn post_subscriptions_returns_400_when_fields_have_invalid_values() {
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=&email=ursula_le_guin@gmail.com", "empty name"),
        ("name=Ursula&email=", "empty email"),
        ("name=Ursula&email=invalid_email", "invalid email"),
    ];

    for (body, description) in test_cases {
        let response = app.post_subscriptions(body.into()).await;

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
    let app = spawn_app().await;
    let test_cases = vec![
        ("name=le%20guin", "missing the e-mail"),
        ("email=ursula_le_guin%40gmail.com", "missing the name"),
        ("", "missing both name and e-mail"),
    ];

    for (invalid_body, error_message) in test_cases {
        let response = app.post_subscriptions(invalid_body.into()).await;

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
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;
}

#[actix_rt::test]
async fn subscribe_sends_a_confirmation_email_with_a_link() {
    let app = spawn_app().await;
    let body = "name=le%20guin&email=ursula_le_guin%40gmail.com";

    Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;

    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

    let get_link = |s: &str| -> String {
        let links: Vec<_> = linkify::LinkFinder::new()
            .links(s)
            .filter(|l| *l.kind() == linkify::LinkKind::Url)
            .collect();
        assert_eq!(links.len(), 1);

        links[0].as_str().to_owned()
    };

    let html_link = get_link(
        selector(&body)("$.content[?(@.type == 'text/html')].value").unwrap()[0]
            .as_str()
            .unwrap(),
    );
    let text_link = get_link(
        selector(&body)("$.content[?(@.type == 'text/plain')].value").unwrap()[0]
            .as_str()
            .unwrap(),
    );

    assert_eq!(html_link, text_link);
}
