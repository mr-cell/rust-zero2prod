use crate::domain::SubscriberEmail;
use reqwest::Client;
use std::time::Duration;

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: String,
}

impl EmailClient {
    pub fn new(
        base_url: String,
        sender: SubscriberEmail,
        authorization_token: String,
        timeout: Duration,
    ) -> Self {
        Self {
            http_client: Client::builder().timeout(timeout).build().unwrap(),
            base_url,
            sender,
            authorization_token,
        }
    }

    pub async fn send_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/v3/mail/send", self.base_url);
        let request_body = SendEmailRequestBody {
            personalizations: vec![EmailPersonalization {
                to: vec![EmailAddress {
                    email: self.sender.as_ref(),
                }],
            }],
            from: EmailAddress {
                email: recipient.as_ref(),
            },
            subject,
            content: vec![
                EmailContent {
                    content_type: "text/plain",
                    value: text_content,
                },
                EmailContent {
                    content_type: "text/html",
                    value: html_content,
                },
            ],
        };

        self.http_client
            .post(url)
            .header(
                "Authorization",
                format!("Bearer {}", self.authorization_token),
            )
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        Ok(())
    }
}

#[derive(serde::Serialize)]
struct SendEmailRequestBody<'a> {
    personalizations: Vec<EmailPersonalization<'a>>,
    from: EmailAddress<'a>,
    subject: &'a str,
    content: Vec<EmailContent<'a>>,
}

#[derive(serde::Serialize)]
struct EmailPersonalization<'a> {
    to: Vec<EmailAddress<'a>>,
}

#[derive(serde::Serialize)]
struct EmailAddress<'a> {
    email: &'a str,
}

#[derive(serde::Serialize)]
struct EmailContent<'a> {
    #[serde(rename = "type")]
    content_type: &'a str,
    value: &'a str,
}

#[cfg(test)]
mod tests {
    use crate::domain::SubscriberEmail;
    use crate::email_client::EmailClient;
    use claim::{assert_err, assert_ok};
    use fake::faker::internet::en::SafeEmail;
    use fake::faker::lorem::en::Paragraph;
    use fake::faker::lorem::en::Sentence;
    use fake::{Fake, Faker};
    use jsonpath_lib::selector;
    use std::time::Duration;
    use wiremock::matchers::{any, header, header_exists, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn sent_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(180)))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_sends_expected_request() {
        let mock_server = MockServer::start().await;
        let email_client = email_client(mock_server.uri());

        Mock::given(header_exists("authorization"))
            .and(header("Content-Type", "application/json"))
            .and(path("/v3/mail/send"))
            .and(method("POST"))
            .and(SendEmailRequestMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_client
            .send_email(email(), &subject(), &content(), &content())
            .await;

        // asserts
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn email_client(base_url: String) -> EmailClient {
        EmailClient::new(base_url, email(), Faker.fake(), Duration::from_millis(200))
    }

    struct SendEmailRequestMatcher;

    impl wiremock::Match for SendEmailRequestMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);

            if let Ok(body) = result {
                let has_subscriber_email = match selector(&body)("$.personalizations.*.to.*.email")
                {
                    Ok(res) => !res.is_empty(),
                    Err(_) => false,
                };
                let has_sender_email = match selector(&body)("$.from.email") {
                    Ok(res) => !res.is_empty(),
                    Err(_) => false,
                };
                let has_subject = match selector(&body)("$.subject") {
                    Ok(res) => !res.is_empty(),
                    Err(_) => false,
                };
                let has_text_content =
                    match selector(&body)("$.content[?(@.type == 'text/plain')].value") {
                        Ok(res) => !res.is_empty(),
                        Err(_) => false,
                    };
                let has_html_content =
                    match selector(&body)("$.content[?(@.type == 'text/html')].value") {
                        Ok(res) => !res.is_empty(),
                        Err(_) => false,
                    };

                has_subscriber_email
                    && has_sender_email
                    && has_subject
                    && has_text_content
                    && has_html_content
            } else {
                false
            }
        }
    }
}
