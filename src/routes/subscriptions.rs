use crate::domain::{NewSubscriber, SubscriberEmail, SubscriberName};
use crate::email_client::EmailClient;
use crate::startup::ApplicationBaseUrl;
use actix_web::{web, HttpResponse, Responder};
use chrono::Utc;
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use sqlx::{PgPool, Postgres, Transaction};
use std::convert::{TryFrom, TryInto};
use tera::{Context, Tera};
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

#[allow(clippy::async_yields_async)]
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
) -> impl Responder {
    let new_subscriber = match form.0.try_into() {
        Ok(new_subscriber) => new_subscriber,
        Err(message) => {
            tracing::error!("Subscribing new user failed: {}", message);
            return HttpResponse::BadRequest();
        }
    };

    let mut transaction = match db_pool.begin().await {
        Ok(transaction) => transaction,
        Err(error) => {
            tracing::error!("Creating new DB transaction failed: {:?}", error);
            return HttpResponse::InternalServerError();
        }
    };

    let subscriber_id = match insert_subscriber(&mut transaction, &new_subscriber).await {
        Ok(subscriber_id) => subscriber_id,
        Err(error) => {
            tracing::error!("Inserting new subscriber into database failed: {:?}", error);
            return HttpResponse::InternalServerError();
        }
    };
    let subscription_token = generate_subscription_token();

    if let Err(error) =
        insert_subscription_token(&mut transaction, &subscription_token, &subscriber_id).await
    {
        tracing::error!(
            "Inserting subscription token into database failed: {:?}",
            error
        );
        return HttpResponse::InternalServerError();
    }

    if let Err(error) = transaction.commit().await {
        tracing::error!("Commiting DB transaction failed: {:?}", error);
        return HttpResponse::InternalServerError();
    }

    if let Err(error) = send_confirmation_email(
        &email_client,
        new_subscriber,
        subscription_token.as_str(),
        &base_url.0,
        &templates,
    )
    .await
    {
        tracing::error!("Sending confirmation email failed: {}", error);
        return HttpResponse::InternalServerError();
    }

    HttpResponse::Ok()
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
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(subscriber_id)
}

#[tracing::instrument(name = "Saving subscription token in the database", skip(transaction))]
async fn insert_subscription_token(
    transaction: &mut Transaction<'_, Postgres>,
    subscription_token: &str,
    subscriber_id: &Uuid,
) -> Result<(), sqlx::Error> {
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
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

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
) -> Result<(), reqwest::Error> {
    let confirmation_link = format!(
        "{}/subscriptions/confirm?subscription_token={}",
        base_url, subscription_token
    );

    let mut context = Context::new();
    context.insert("confirmation_link", &confirmation_link);
    let html_body = templates
        .render("subscriptions/confirm_subscription_email.html", &context)
        .unwrap();
    let plain_body = templates
        .render("subscriptions/confirm_subscription_email.txt", &context)
        .unwrap();

    email_client
        .send_email(new_subscriber.email, "Welcome!", &html_body, &plain_body)
        .await
}
