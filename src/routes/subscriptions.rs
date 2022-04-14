use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::routes::errors::{ApiError, StoreTokenError};
use crate::startup::ApplicationBaseUrl;
use actix_web::{web, HttpResponse};
use anyhow::Context;
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use std::convert::{TryFrom, TryInto};
use tera::Tera;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct SubscribeFormData {
    email: String,
    name: String,
}

impl TryFrom<SubscribeFormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: SubscribeFormData) -> Result<Self, Self::Error> {
        let name = SubscriberName::parse(value.name)?;
        let email = SubscriberEmail::parse(value.email)?;
        Ok(NewSubscriber { name, email })
    }
}

#[tracing::instrument(
    name = "Adding a new subscriber",
    skip(form, db_pool, email_client, base_url, templates),
    fields(
        subscriber_email = %form.email,
        subscriber_name = %form.name
    )
)]
pub async fn subscribe(
    form: web::Form<SubscribeFormData>,
    db_pool: web::Data<PgPool>,
    email_client: web::Data<EmailClient>,
    base_url: web::Data<ApplicationBaseUrl>,
    templates: web::Data<Tera>,
) -> Result<HttpResponse, ApiError> {
    let new_subscriber = form.0.try_into().map_err(ApiError::ValidationError)?;
    let mut transaction = db_pool
        .begin()
        .await
        .context("Failed to acquire database connection from the pool.")?;
    let subscriber_id = insert_subscriber(&mut transaction, &new_subscriber)
        .await
        .context("Failed to insert new subscriber to the database.")?;
    let subscription_token = generate_subscription_token();
    store_token(&mut transaction, &subscription_token, &subscriber_id)
        .await
        .context("Failed to store subscription token.")?;
    transaction
        .commit()
        .await
        .context("Failed to commit SQL transaction to store a new subscriber.")?;

    send_confirmation_email(
        &email_client,
        new_subscriber,
        subscription_token.as_str(),
        &base_url.0,
        &templates,
    )
    .await
    .context("Failed to send a confirmation email.")?;

    Ok(HttpResponse::Ok().finish())
}

fn generate_subscription_token() -> String {
    let mut rng = thread_rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric))
        .map(char::from)
        .take(25)
        .collect()
}

#[tracing::instrument(
    name = "Saving new subscriber details in the database",
    skip(new_subscriber, transaction)
)]
async fn insert_subscriber(
    transaction: &mut Transaction<'_, Postgres>,
    new_subscriber: &NewSubscriber,
) -> Result<Uuid, sqlx::Error> {
    let subscriber_id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO subscriptions(id, email, name, subscribed_at, status)
        VALUES ($1, $2, $3, $4, $5)
        "#,
        subscriber_id,
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
        Utc::now(),
        "PENDING"
    )
    .execute(transaction)
    .await?;

    Ok(subscriber_id)
}

#[tracing::instrument(name = "Saving subscription token in the database", skip(transaction))]
async fn store_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscription_token: &str,
    subscriber_id: &Uuid,
) -> Result<(), StoreTokenError> {
    sqlx::query!(
        r#"
        INSERT INTO subscription_tokens(subscription_token, subscriber_id)
        VALUES ($1, $2)
        "#,
        subscription_token,
        subscriber_id
    )
    .execute(transaction)
    .await
    .map_err(StoreTokenError)?;

    Ok(())
}

#[tracing::instrument(
    name = "Send a confirmation email to a new subscriber",
    skip(email_client, new_subscriber, base_url, templates),
    fields(
        subscriber_email = %new_subscriber.email,
        subscriber_name = %new_subscriber.name
    )
)]
async fn send_confirmation_email(
    email_client: &EmailClient,
    new_subscriber: NewSubscriber,
    subscription_token: &str,
    base_url: &str,
    templates: &Tera,
) -> Result<(), anyhow::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );

    let mut context = tera::Context::new();
    context.insert("confirmation_link", &confirmation_link);
    let html_body = templates.render("subscriptions/confirm_subscription_email.html", &context)?;
    let plain_body = templates.render("subscriptions/confirm_subscription_email.txt", &context)?;

    email_client
        .send_email(&new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
        .with_context(|| {
            format!(
                "Sending confirmation email failed for email address: {}.",
                new_subscriber.email.as_ref()
            )
        })
}
