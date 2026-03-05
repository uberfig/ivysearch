use actix_web::{
    HttpResponse, Responder, get,
    http::{StatusCode, header::ContentType},
};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .body(include_str!("../../webui/index.html"))
}
