use url::Url;

pub fn get_links(parent: scraper::ElementRef<'_>, domain: &str) -> Vec<Url> {
    let mut links = Vec::new();
    if parent.value().name() == "a" {
        if let Some(link) = parent.value().attr("href") {
            let link = link.trim().to_lowercase();
            if let Ok(link) = Url::parse(&link) {
                links.push(link);
            } else if let Ok(link) = Url::parse(&format!("http://{}{}", domain, link)) {
                links.push(link);
            } else {
                println!("invalid link: {}", link);
            }
        }
    }
    for element in parent.child_elements() {
        links.append(&mut get_links(element, domain))
    }
    links.dedup();
    links
}
