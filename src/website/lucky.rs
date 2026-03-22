use actix_web::{HttpResponse, Responder, get};

use crate::configuration::root_sites::RootSites;

#[get("/lucky")]
async fn lucky() -> impl Responder {
    let lucky = RootSites::get().await.get_random();
    HttpResponse::SeeOther()
        .insert_header((
            "Location",
            lucky.as_str(),
        ))
        .body("")
}