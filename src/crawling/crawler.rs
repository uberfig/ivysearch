use chrono::Local;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use texting_robots::Robot;
use tokio::sync::RwLock;
use url::Url;

use crate::{
    configuration::root_sites::RootSites,
    indexing::index_store::IndexSharable,
    parsing::{keywords::get_keywords, links::get_links},
};

const KNOWN_WRONG_FORMAT: &'static [&str] = &[
    ".pdf", ".jpg", ".png", ".gif", ".rss", ".xml", ".css", ".js",
    ".webp", ".svg", ".zip", ".7z", ".gz", ".tar", ".csv", ".txt",
    ".mp3", ".mp4", ".wav", ".ogg", ".fcstd", ".3mf", ".jpeg", ".gif"
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
    if is_wrong(page.as_str()) {
        println!("not html");
        return Err(());
    }
    let indexed_time = Local::now();

    if let Some(last_indexed) = word_index.get_page_date(&page).await {
        if indexed_time.signed_duration_since(last_indexed).as_seconds_f32() < (60.0 * 60.0 * -1.0) { // less than 2 hours old
            println!("page indexed very recently");
            return Ok((page.clone(), word_index.get_outgoing(&page).await));
        }
    }

    let client = reqwest::Client::new();
    let response = client
        .get(page.as_str())
        .header("User-Agent", "ivysearch")
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
        println!(
            "possible duplicate site resolved: {} from link: {}",
            resolved_url.as_str(),
            page.as_str()
        );
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
            println!("links {}", serde_json::to_string(&word_index.get_outgoing(&page).await).unwrap());
            return Ok((resolved_url, word_index.get_outgoing(&page).await));
        }
        //remove content and begin reindexing
        word_index.remove_page_content(&page).await;
    }

    // println!("parsing");

    let parsed: scraper::Html = scraper::Html::parse_document(&body);
    // dbg!(&parsed.errors);
    if parsed.tree.nodes().count() == 0 || parsed.errors.len() > 45 {
        println!("body err on page {}", page);
        // dbg!(parsed.errors);
        return Err(());
    }

    // println!("parsing finished");

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
    word_index.set_page_links(page.clone(), links.clone()).await;

    println!("finished crawl of {}", page.as_str());

    Ok((resolved_url, links))
}

pub struct VisitedSites {
    pub visited: Arc<RwLock<HashSet<String>>>,
}

// pub async fn crawl_recursive(
//     pages: Vec<Url>,
//     word_index: IndexSharable,
//     depth: usize,
//     site_depth: usize,
//     stack: Arc<RwLock<Vec<StackElem>>>,
//     // todo make this take in a visited sites shared with all threads
//     // todo add in sleeps
// ) {
//     let mut visited = HashSet::new();
//     for page in pages {
//         crawl(page, word_index.clone(), depth, site_depth, &mut visited, stack).await;
//     }
// }

type Depth = usize;
type SiteDepth = usize;
#[derive(Debug, Serialize, Deserialize)]
pub struct StackElem {
    pub depth: usize,
    pub site_depth: usize,
    pub page: Url,
    // parent: Option<Url>,
}

fn has_domain(domain: Option<&str>, list: &Vec<Url>) -> bool {
    for element in list {
        if let (Some(banned_domain), Some(check_domain)) = (element.domain(), domain) {
            if check_domain.ends_with(banned_domain) {
                return true;
            }
        }
    }
    false
}

async fn get_robots(domain: &str) -> Option<Robot> {
    let robots_url = format!("https://{}/robots.txt", domain);
    let client = reqwest::Client::new();
    let response = client
        .get(robots_url)
        .header("User-Agent", "ivysearch")
        .send()
        .await;
    let response = match response {
        Ok(resp) => resp,
        Err(_err) => {
            println!("robot response err for {}", domain);
            return None;
        }
    };
    let body = match response.text().await {
        Ok(bod) => bod,
        Err(_err) => {
            println!("robot body resp err for {}", domain);
            return None;
        }
    };
    let r = Robot::new("ivysearch", body.as_bytes());
    if let Ok(res) = r {
        return Some(res);
    }
    None
}

async fn pop_next(stack: &Arc<RwLock<Vec<StackElem>>>) -> Option<StackElem> {
    stack.write().await.pop()
}

async fn insert_new(stack: &Arc<RwLock<Vec<StackElem>>>, element: StackElem) {
    let mut stack = stack.write().await;
    stack.push(element);
    // println!("stack {}", serde_json::to_string(&*stack).unwrap());
}

pub async fn crawl(
    word_index: IndexSharable,
    stack: Arc<RwLock<Vec<StackElem>>>,
) {
    let mut visited = HashSet::new();
    let blacklist = RootSites::get().await.blacklist;
    let mut domain_robots: HashMap<String, Option<Robot>> = HashMap::new();
    // let mut stack: Vec<StackElem> = vec![StackElem {
    //     depth: start_depth,
    //     site_depth: start_site_depth,
    //     page: start_page,
    // }];
    while let Some(mut elem) = pop_next(&stack).await {
        // println!("preparing to crawl: {}", serde_json::to_string_pretty(&elem).unwrap());
        if elem.depth <= 0 || elem.site_depth <= 0 {
            continue;
        }
        elem.page.set_fragment(None);
        let _ = elem.page.set_scheme("https");
        println!("crawling: {}", elem.page.as_str());

        let Ok((resolved, pages)) =
            crawl_html_page(elem.page.clone(), word_index.clone(), &mut visited).await
        else {
            // println!("crawling error");
            continue;
        };
        visited.insert(elem.page.as_str().to_string());
        // println!("crawled: {} and got links: {}", elem.page.as_str(), serde_json::to_string_pretty(&pages).unwrap());

        'new_page_loop: for new_page in pages {
            
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
            if depth <=0 || site_depth <= 0 {
                // println!("meow");
                continue;
            }
            if let Some(domain) = new_page.domain() {
                match domain_robots.get(domain) {
                    Some(Some(robot_rules)) => {
                        if !robot_rules.allowed(new_page.as_str()) {
                            println!("robots disallowed for {}", new_page);
                            continue 'new_page_loop;
                        }
                    }
                    Some(None) => {}
                    None => {
                        let robot = get_robots(domain).await;
                        if let Some(robot) = &robot {
                            if !robot.allowed(new_page.as_str()) {
                                println!("robots disallowed for {}", new_page);
                                continue 'new_page_loop;
                            }
                        }
                        domain_robots.insert(domain.to_string(), robot);
                    }
                }
            }
            if has_domain(new_page.domain(), &blacklist) {
                println!("skipping banned domain");
                continue;
            }
            insert_new(&stack, StackElem {
                depth,
                site_depth,
                page: new_page,
            }).await;
        }
    }
}
