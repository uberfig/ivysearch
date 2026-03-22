use std::collections::VecDeque;

use crate::{
    indexing::index_store::{IndexSharable, SearchHit},
    parsing::keywords::split_string,
};
use actix_web::{
    HttpResponse, Responder, get,
    http::{StatusCode, header::{ContentLength, ContentType}},
    post,
    web::{self, Data},
};
use lazy_static::lazy_static;
use percent_encoding::{AsciiSet, CONTROLS};
use serde::{Deserialize, Serialize};
use tera::{Context, Tera};

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_template(
            "results.html",
            include_str!("../../webui/templates/results.html"),
        )
        .expect("Failed to add raw template");
        tera
    };
}

const FRAGMENT: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'<').add(b'>').add(b'`');

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Combined {
    parent: SearchHit,
    children: Vec<SearchHit>,
}

fn combine_results(meow: Vec<SearchHit>) -> Vec<Combined> {
    let mut meow = VecDeque::from(meow);
    let mut new_list: Vec<Combined> = Vec::new();
    while let Some(hit) = meow.pop_front() {
        if new_list.is_empty() {
            new_list.push(Combined {
                parent: hit,
                children: vec![],
            });
            continue;
        }
        let index = new_list.len() - 1;
        if let Some(curr) = new_list.get_mut(index) {
            if curr.parent.page.domain() == hit.page.domain() {
                curr.children.push(hit);
            } else {
                new_list.push(Combined {
                    parent: hit,
                    children: vec![],
                });
            }
        }
    }
    for combined in &mut new_list {
        combined
            .children
            .sort_by(|a, b| a.page_rank.total_cmp(&b.page_rank));
        combined.children.reverse();
        if let Some(child) = combined.children.get(0) {
            if child.page_rank > combined.parent.page_rank {
                let child = combined.children.remove(0);
                let parent = combined.parent.clone();
                combined.parent = child;
                combined.children.insert(0, parent);
                combined
                    .children
                    .sort_by(|a, b| a.page_rank.total_cmp(&b.page_rank));
                combined.children.reverse();
            }
        }
    }

    new_list
}

#[get("/search/{query}")]
async fn results(index: Data<IndexSharable>, path: web::Path<String>) -> impl Responder {
    let path: Vec<u8> = percent_encoding::percent_decode_str(&path).collect();
    let path: String = String::from_utf8_lossy(&path).to_lowercase();
    let mut words: Vec<String> = path
        .split_ascii_whitespace()
        .map(|x| x.to_string())
        .collect();
    println!("search: {}", serde_json::to_string(&words).unwrap());
    let site_search = match words.get(0).cloned() {
        Some(first) => {
            if first.ends_with(":") && first != ":" {
                words.remove(0);
                first.strip_suffix(":").map(|x| x.to_string())
            } else {
                None
            }
        }
        None => None,
    };
    let words: Vec<String> = split_string(&words.join(" "))
        .map(|x| x.to_string())
        .collect();
    let results = match &site_search {
        Some(site) => index.search_site(words, site).await,
        None => index.search(words).await,
    };

    let mut context = Context::new();
    context.insert("query", path.as_str());
    context.insert("results", &results);
    context.insert("site_mode", &site_search);
    context.insert("combined_results", &combine_results(results));
    let body = TEMPLATES
        .render("results.html", &context)
        .expect("failed to render");

    HttpResponse::build(StatusCode::OK)
        .content_type(ContentType::html())
        .insert_header(ContentLength(body.len()))
        .body(body)
}

#[derive(Deserialize)]
pub struct Query {
    pub query: String,
}

#[post("/search")]
async fn search_post(web::Form(form): web::Form<Query>) -> impl Responder {
    HttpResponse::SeeOther()
        .insert_header((
            "Location",
            format!(
                "/search/{}",
                percent_encoding::utf8_percent_encode(&form.query, FRAGMENT)
            ),
        ))
        .body("")
}
