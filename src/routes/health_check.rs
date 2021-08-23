use actix_web::{HttpResponse, Responder};

#[allow(clippy::async_yields_async)]
#[tracing::instrument(name = "Health Check request")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}
