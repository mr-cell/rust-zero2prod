use actix_web::HttpResponse;

#[tracing::instrument(name = "Health Check request")]
pub async fn health_check() -> HttpResponse {
    HttpResponse::Ok().finish()
}
