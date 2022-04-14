use crate::domain::{SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::routes::errors::ApiError;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use sqlx::PgPool;
use tera::Tera;

#[derive(serde::Deserialize)]
pub struct BodyData {
    title: String,
    content: Content,
}

#[derive(serde::Deserialize)]
pub struct Content {
    text: String,
    html: String,
}

pub struct ConfirmedSubscriber {
    name: SubscriberName,
    email: SubscriberEmail,
}

#[tracing::instrument(
    name = "Distributing the newsletter",
    skip(newsletter, db_pool, email_client, templates),
    fields(
        newsletter_title = %newsletter.title
    ),
)]
pub async fn distribute_newsletter(
    newsletter: web::Json<BodyData>,
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    templates: web::Data<Tera>,
) -> Result<HttpResponse, ApiError> {
    let confirmed_subscribers = get_confirmed_subscribers(&db_pool).await?;
    for subscriber in confirmed_subscribers {
        match subscriber {
            Ok(subscriber) => send_newsletter(&newsletter, &subscriber, &email_client, &templates)
                .await
                .with_context(|| format!("Failed to send newsletter to {}", subscriber.email))?,
            Err(error) => {
                tracing::warn!(error.cause_chain = ?error, "Skipping a confirmed subscriber Their stored contact details are invalid")
            }
        }
    }

    Ok(HttpResponse::Ok().finish())
}

#[allow(clippy::unnecessary_unwrap)]
#[tracing::instrument(name = "Getting confirmed subscribers", skip(db_pool))]
async fn get_confirmed_subscribers(
    db_pool: &PgPool,
) -> Result<Vec<Result<ConfirmedSubscriber, anyhow::Error>>, anyhow::Error> {
    let confirmed_subscribers = sqlx::query!(
        r#"
        SELECT name, email
        FROM subscriptions
        WHERE status = 'CONFIRMED'
        "#,
    )
    .fetch_all(db_pool)
    .await?
    .into_iter()
    .map(|r| {
        let subscriber_email = SubscriberEmail::parse(r.email).map_err(|e| anyhow::anyhow!(e));
        let subscriber_name = SubscriberName::parse(r.name).map_err(|e| anyhow::anyhow!(e));

        if subscriber_name.is_ok() && subscriber_email.is_ok() {
            Ok(ConfirmedSubscriber {
                name: subscriber_name.unwrap(),
                email: subscriber_email.unwrap(),
            })
        } else if subscriber_name.is_err() {
            Err(subscriber_name.unwrap_err())
        } else {
            Err(subscriber_email.unwrap_err())
        }
    })
    .collect();

    Ok(confirmed_subscribers)
}

#[tracing::instrument(
    name = "Sending newsletter to confirmed subscriber",
    skip(newsletter, subscriber, email_client, templates),
    fields(
        subscriber_email = %subscriber.email
    )
)]
async fn send_newsletter(
    newsletter: &BodyData,
    subscriber: &ConfirmedSubscriber,
    email_client: &EmailClient,
    templates: &Tera,
) -> Result<(), anyhow::Error> {
    let mut context = tera::Context::new();
    context.insert("subscriber_name", subscriber.name.as_ref());
    context.insert("html_newsletter", newsletter.content.html.as_str());
    context.insert("text_newsletter", newsletter.content.text.as_str());

    let html_body = templates.render("newsletters/distribute_newsletter.html", &context)?;
    let text_body = templates.render("newsletters/distribute_newsletter.txt", &context)?;

    email_client
        .send_email(
            &subscriber.email,
            newsletter.title.as_str(),
            html_body.as_str(),
            text_body.as_str(),
        )
        .await
        .with_context(|| {
            format!(
                "Sending newsletter email failed for email address: {}",
                subscriber.email.as_ref()
            )
        })
}
