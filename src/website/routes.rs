use crate::website::{
    index::index,
    results::{results, search_post},
};

pub fn get_api_routes() -> actix_web::Scope {
    actix_web::web::scope("")
        .service(results)
        .service(search_post)
        .service(index)
}
