use actix_web::{HttpResponse, Responder};

#[tracing::instrument(name = "Health Check request")]
pub async fn health_check() -> impl Responder {
    HttpResponse::Ok()
}
