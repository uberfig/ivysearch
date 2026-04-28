use actix_web::{
    HttpResponse, Responder, get,
    http::{StatusCode, header::ContentLength, header::ContentType},
};

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .insert_header(ContentLength(include_str!("../../webui/index.html").len()))
        .body(include_str!("../../webui/index.html"))
}
