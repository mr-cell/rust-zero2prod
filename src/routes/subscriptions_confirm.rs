use crate::domain::SubscriptionToken;
use actix_web::{web, HttpResponse, Responder};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct Parameters {
    subscription_token: String,
}

#[allow(clippy::async_yields_async)]
#[tracing::instrument(
    name = "Confirm a pending subscriber",
    skip(db_pool, _parameters),
    fields(
        subscription_token = %_parameters.subscription_token
    )
)]
pub async fn confirm(
    db_pool: web::Data<PgPool>,
    _parameters: web::Query<Parameters>,
) -> impl Responder {
    let token = match SubscriptionToken::parse(_parameters.subscription_token.clone()) {
        Ok(token) => token,
        Err(error) => {
            tracing::error!("Received subscription token is not valid: {}", error);
            return HttpResponse::BadRequest();
        }
    };

    let id = match find_subscriber_id(&db_pool, &token).await {
        Ok(subscriber_id) => subscriber_id,
        Err(error) => {
            tracing::error!(
                "Finding subscriber id for subscription token failed: {:?}",
                error
            );
            return HttpResponse::InternalServerError();
        }
    };

    match id {
        None => HttpResponse::Unauthorized(),
        Some(subscriber_id) => match mark_subscriber_as_confirmed(&db_pool, &subscriber_id).await {
            Ok(()) => HttpResponse::Ok(),
            Err(error) => {
                tracing::error!("Marking subscriber status as confirmed failed: {:?}", error);
                HttpResponse::InternalServerError()
            }
        },
    }
}

#[tracing::instrument(name = "Find subscriber id from subscription token", skip(db_pool))]
async fn find_subscriber_id(
    db_pool: &PgPool,
    subscription_token: &SubscriptionToken,
) -> Result<Option<Uuid>, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        SELECT subscriber_id FROM subscription_tokens WHERE subscription_token = $1
        "#,
        subscription_token.as_ref(),
    )
    .fetch_optional(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(result.map(|r| r.subscriber_id))
}

#[tracing::instrument(name = "Mark subscriber as confirmed", skip(db_pool))]
async fn mark_subscriber_as_confirmed(
    db_pool: &PgPool,
    subscriber_id: &Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"
        UPDATE subscriptions SET status=$1 WHERE id=$2
        "#,
        "CONFIRMED",
        subscriber_id
    )
    .execute(db_pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to execute query: {:?}", e);
        e
    })?;

    Ok(())
}
