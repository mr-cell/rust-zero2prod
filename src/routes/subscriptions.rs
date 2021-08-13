use actix_web::{web, HttpResponse, Responder};

#[derive(serde::Deserialize)]
pub struct SubscribeFormData {
    email: String,
    name: String,
}

pub async fn subscribe(_form: web::Form<SubscribeFormData>) -> impl Responder {
    HttpResponse::Ok()
}
