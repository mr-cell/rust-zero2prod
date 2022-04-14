use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[actix_rt::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    // given
    let app = spawn_app().await;
    create_unconfirmed_subscriber("le guin", "ursula_le_guin@gmail.com", &app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    // when
    let newsletter_request_body = serde_json::json!({
       "title": "newsletter title",
        "content": {
            "text": "newsletter content in text",
            "html": "newsletter content in html",
        }
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[actix_rt::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    // given
    let app = spawn_app().await;
    create_confirmed_subscriber("le guin", "ursula_le_guin@gmail.com", &app).await;

    Mock::given(path("/v3/mail/send"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    // when
    let newsletter_request_body = serde_json::json!({
       "title": "newsletter title",
        "content": {
            "text": "newsletter content in text",
            "html": "newsletter content in html",
        }
    });

    let response = app.post_newsletters(newsletter_request_body).await;

    // then
    let email_requests = &app.email_server.received_requests().await.unwrap();
    let last_email_request = email_requests.last().unwrap();
    let email_body = &app.get_email_body(last_email_request);

    assert_eq!(response.status().as_u16(), 200);
    assert!(email_body
        .html
        .as_str()
        .contains("newsletter content in html"));
    assert!(email_body
        .plain
        .as_str()
        .contains("newsletter content in text"));
}

#[actix_rt::test]
async fn newsletters_return_400_for_invalid_data() {
    // given
    let app = spawn_app().await;
    let test_cases = vec![
        (
            serde_json::json!({
                "content": {
                    "text": "Newsletter in plain text",
                    "html": "<p>Newsletter in HTML format</p>"
                }
            }),
            "missing title",
        ),
        (
            serde_json::json!({
                "title": "Newsletter title"
            }),
            "missing content",
        ),
    ];

    for (invalid_body, error_message) in test_cases {
        // when
        let response = app.post_newsletters(invalid_body).await;

        // then
        assert_eq!(response.status().as_u16(), 400, "{}", error_message);
    }
}

async fn create_unconfirmed_subscriber(
    name: &str,
    email: &str,
    app: &TestApp<'_>,
) -> ConfirmationLinks {
    let body = format!(
        "name={}&email={}",
        url_escape::encode_fragment(name),
        url_escape::encode_fragment(email)
    );

    let _mock_guard = Mock::given(path("/v3/mail/send"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    // when
    app.post_subscriptions(body)
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();

    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(name: &str, email: &str, app: &TestApp<'_>) {
    let confirmation_links = create_unconfirmed_subscriber(name, email, app).await;

    reqwest::get(confirmation_links.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
