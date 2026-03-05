use std::{collections::HashSet, sync::Arc};
use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use url::Url;

use crate::{
    indexing::index_store::IndexSharable,
    parsing::{keywords::get_keywords, links::get_links},
};

const KNOWN_WRONG_FORMAT: &'static [&str] = &[
    ".pdf", ".jpg", ".png", ".gif", ".rss", ".xml", ".css", ".js", 
];

fn is_wrong(input: &str) -> bool {
    for format in KNOWN_WRONG_FORMAT {
        if input.ends_with(format) {
            return true;
        }
    }
    return false;
}

fn sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input);
    format!("{:x}", hasher.finalize())
}

/// returns the resolved url and the links if successful
pub async fn crawl_html_page(
    page: Url,
    word_index: IndexSharable,
    visited: &mut HashSet<String>,
) -> Result<(Url, Vec<Url>), ()> {
    println!("crawling");
    if is_wrong(page.as_str()){
        println!("not html");
        return Err(());
    }
    let indexed_time = Local::now();
    let client = reqwest::Client::new();
    let response = client
        .get(page.as_str())
        // .header("User-Agent", "ivysearch")
        .send()
        .await;
    let response = match response {
        Ok(resp) => resp,
        Err(_err) => {
            println!("response err");
            // dbg!(err);
            return Err(());
        }
    };
    let mut resolved_url = response.url().to_owned();
    resolved_url.set_fragment(None);
    let _ = resolved_url.set_scheme("https");
    if resolved_url != page {
        println!("resolved url: {}", resolved_url.as_str());
    }
    if visited.contains(resolved_url.as_str()) && resolved_url != page {
        println!("possible duplicate site resolved: {} from link: {}", resolved_url.as_str(), page.as_str());
        return Err(());
    }
    visited.insert(resolved_url.to_string());
    let body = match response.text().await {
        Ok(bod) => bod,
        Err(_err) => {
            println!("body resp err");
            // dbg!(err);
            return Err(());
        }
    };
    let hash = sha256(&body);

    if let Some(id) = word_index.get_page_id(&page).await {
        if word_index.get_page_hash(id).await == hash {
            println!("page unchanged");
            return Ok((resolved_url, word_index.get_outgoing(&page).await));
        }
        //remove content and begin reindexing
        word_index.remove_page_content(&page).await;
    }

    println!("parsing");

    let parsed: scraper::Html = scraper::Html::parse_document(&body);
    // dbg!(&parsed.errors);
    if parsed.tree.nodes().count() == 0 || parsed.errors.len() > 45 {
        println!("body err on page {}", page);
        // dbg!(parsed.errors);
        return Err(());
    }

    let mut keywords = get_keywords(parsed.root_element());
    keywords.add_link(&page);
    // println!("inserting keywords");
    word_index
        .insert(
            keywords.keywords.into_iter(),
            page.clone(),
            indexed_time,
            &hash,
        )
        .await;
    let mut links = get_links(parsed.root_element(), resolved_url.domain().unwrap_or(""));
    for link in &mut links {
        link.set_fragment(None);
        let _ = link.set_scheme("https");
    }
    links.dedup();
    word_index.set_page_links(page, links.clone()).await;

    Ok((resolved_url, links))
}

pub struct VisitedSites {
    pub visited: Arc<RwLock<HashSet<String>>>,
}

pub async fn crawl_recursive(
    pages: Vec<Url>,
    word_index: IndexSharable,
    depth: usize,
    site_depth: usize,
    // todo make this take in a visited sites shared with all threads
    // todo add in sleeps
) {
    let mut visited = HashSet::new();
    for page in pages {
        crawl(page, word_index.clone(), depth, site_depth, &mut visited).await;
    }
}

type Depth = usize;
type SiteDepth = usize;
#[derive(Debug, Serialize, Deserialize)]
struct StackElem {
    depth: usize,
    site_depth: usize,
    page: Url,
    // parent: Option<Url>,
}

async fn crawl(
    start_page: Url,
    word_index: IndexSharable,
    start_depth: usize,
    start_site_depth: usize,
    visited: &mut HashSet<String>,
) {
    let mut stack: Vec<StackElem> = vec![ StackElem{ depth: start_depth, site_depth: start_site_depth, page: start_page }];
    while let Some(mut elem) = stack.pop() {
        // println!("preparing to crawl: {}", serde_json::to_string_pretty(&elem).unwrap());
        if elem.depth <= 0 || elem.site_depth <= 0 {
            continue;
        }
        elem.page.set_fragment(None);
        let _ = elem.page.set_scheme("https");
        println!("crawling: {}", elem.page.as_str());
        
        let Ok((resolved, pages)) =
            crawl_html_page(elem.page.clone(), word_index.clone(), visited).await
        else {
            println!("crawling error");
            continue;
        };
        visited.insert(elem.page.as_str().to_string());
        // println!("crawled: {} and got links: {}", elem.page.as_str(), serde_json::to_string_pretty(&pages).unwrap());


        for new_page in pages {
            // println!("child page: {}", new_page.as_str());
            if visited.contains(new_page.as_str()) {
                // println!("already visited {}", new_page.as_str());
                continue;
            }
            
            let mut depth: Depth = elem.depth;
            let mut site_depth: SiteDepth = elem.site_depth;
            if new_page.domain() == elem.page.domain() || new_page.domain() == resolved.domain() {
                // println!("same domain");
                site_depth -= 1;
            } else {
                depth -= 1;
            }
            // println!("pushing: {}", new_page.as_str());
            stack.push(StackElem {depth, site_depth, page: new_page } );
            // println!("stack after push: {}", serde_json::to_string_pretty(&stack).unwrap());
        }
    }
}
