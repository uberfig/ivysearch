use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use url::Url;

use crate::indexing::graph_store::GraphStore;

const FILE_PATH: &str = "word_frequency_index.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexedPage {
    pub frequency: usize,
    pub page: usize,
}

impl PartialEq for IndexedPage {
    fn eq(&self, other: &Self) -> bool {
        self.frequency == other.frequency && self.page == other.page
    }
}

impl PartialOrd for IndexedPage {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self.frequency.partial_cmp(&other.frequency) {
            Some(core::cmp::Ordering::Equal) => {}
            ord => return ord,
        }
        self.page.partial_cmp(&other.page)
    }
}
#[derive(Debug, Clone)]
pub struct IndexSharable {
    store: Arc<RwLock<IndexStore>>,
}
impl IndexSharable {
    pub fn new(store: IndexStore) -> Self {
        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
    pub async fn search(&self, words: Vec<String>) -> Vec<SearchHit> {
        let lock = self.store.read().await;
        lock.search(words)
    }
    /// insert word occurances for a given page, inits metadata if necessary
    pub async fn insert(
        &self,
        words: impl Iterator<Item = (String, usize)>,
        url: Url,
        last_indexed: DateTime<Local>,
        hash: &str,
    ) {
        let mut lock = self.store.write().await;
        for (word, frequency) in words {
            lock.insert(word, url.clone(), frequency, last_indexed, hash);
        }
    }
    pub async fn remove_page_content(&self, url: &Url) {
        let mut lock = self.store.write().await;
        lock.remove_page_content(url);
    }
    pub async fn write(&self) -> std::io::Result<()> {
        let lock = self.store.read().await;
        lock.write().await
    }
    pub async fn get_page_id(&self, page: &Url) -> Option<usize> {
        let lock = self.store.read().await;
        lock.page_ids.get(page).copied()
    }
    pub async fn get_page_hash(&self, id: usize) -> String {
        let lock = self.store.read().await;
        lock.pages[id].hash.clone()
    }
    pub async fn get_outgoing(&self, page: &Url) -> Vec<Url> {
        let lock = self.store.read().await;
        lock.graph
            .outgoing
            .get(&page)
            .cloned()
            .unwrap_or(Vec::new())
    }
    pub async fn set_page_links(&self, page: Url, outgoing: Vec<Url>) {
        let mut lock = self.store.write().await;
        lock.graph.set_page_links(page, outgoing);
    }
    pub async fn search_site(&self, words: Vec<String>, domain: &str) -> Vec<SearchHit> {
        self.search(words)
            .await
            .into_iter()
            .filter(|x| x.page.domain() == Some(domain))
            .collect()
    }
    pub async fn init_pagerank(&self) {
        let mut lock = self.store.write().await;
        lock.graph.init_pagerank();
    }
    pub async fn pagerank_iteration(&self) {
        let mut lock = self.store.write().await;
        lock.graph.pagerank_iteration();
    }
    pub async fn get_page_date(&self, url: &Url) -> Option<DateTime<Local>> {
        let lock = self.store.read().await;
        if let Some(id) = lock.page_ids.get(url) {
            return Some(lock.pages[*id].last_indexed);
        }
        None
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageInfo {
    pub url: Url,
    pub hash: String,
    pub last_indexed: DateTime<Local>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexStore {
    pub pages: Vec<PageInfo>,
    pub page_ids: HashMap<Url, usize>,
    pub graph: GraphStore,
    pub words: HashMap<String, Vec<IndexedPage>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchHit {
    pub page: Url,
    /// how many of the searched words does this hit
    pub individual_hits: usize,
    /// how many times in total does it hit the searched words
    pub total_hits: usize,
    pub page_rank: f64,
    pub missing_keywords: Vec<String>,
}

impl IndexStore {
    pub async fn get() -> IndexStore {
        match fs::read_to_string(FILE_PATH).await {
            Ok(contents) => {
                serde_json::from_str(contents.as_str()).expect("invalid config on disk")
            }
            Err(_) => {
                let new = Self::new();
                let mut file = File::create(FILE_PATH).await.expect("failed to init file");
                file.write_all(serde_json::to_string_pretty(&new).unwrap().as_bytes())
                    .await
                    .expect("failed to write to new file");
                file.flush().await.expect("failed to flush");
                new
            }
        }
    }
    pub fn new() -> Self {
        Self {
            words: HashMap::new(),
            pages: Vec::new(),
            page_ids: HashMap::new(),
            graph: GraphStore::new(),
        }
    }
    pub async fn write(&self) -> std::io::Result<()> {
        let mut file = File::create(FILE_PATH).await?;
        file.write_all(serde_json::to_string_pretty(&self).unwrap().as_bytes())
            .await?;
        file.flush().await?;
        Ok(())
    }
    /// removes all instances of page from words but leaves the metadata
    pub fn remove_page_content(&mut self, url: &Url) {
        let Some(id) = self.page_ids.get(url) else {
            return;
        };
        for (_, pages) in &mut self.words {
            let pos = pages.iter().position(|x| x.page == *id);
            if let Some(pos) = pos {
                pages.remove(pos);
            }
        }
    }
    /// adds a url to metadata if doesn't exist, returns id if it does
    fn insert_url(&mut self, url: Url, last_indexed: DateTime<Local>, hash: &str) -> usize {
        match self.page_ids.get(&url) {
            Some(id) => *id,
            None => {
                let id = self.pages.len();
                self.pages.push(PageInfo {
                    url: url.clone(),
                    hash: hash.to_string(),
                    last_indexed,
                });
                self.page_ids.insert(url, id);
                id
            }
        }
    }
    /// insert a word occurance for a given page, inits metadata if necessary
    pub fn insert(
        &mut self,
        word: String,
        url: Url,
        frequency: usize,
        last_indexed: DateTime<Local>,
        hash: &str,
    ) {
        let page = self.insert_url(url, last_indexed, hash);
        match self.words.get_mut(&word) {
            Some(word_pages) => {
                let pos = word_pages
                    .binary_search_by(|probe| probe.frequency.cmp(&frequency))
                    .unwrap_or_else(|e| e);
                word_pages.insert(pos, IndexedPage { frequency, page });
            }
            None => {
                self.words
                    .insert(word, vec![IndexedPage { frequency, page }]);
            }
        }
    }
    pub fn search(&self, words: Vec<String>) -> Vec<SearchHit> {
        let mut results: HashMap<Url, SearchHit> = HashMap::new();
        let words_clone = words.clone();
        for word in words {
            let hits = self.words.get(&word);
            if let Some(hits) = hits {
                for page in hits {
                    match results.get_mut(&self.pages[page.page].url) {
                        Some(result) => {
                            result.individual_hits += 1;
                            result.total_hits += page.frequency;
                            result.missing_keywords = result
                                .missing_keywords
                                .clone()
                                .into_iter()
                                .filter(|x| x != &word)
                                .collect();
                        }
                        None => {
                            results.insert(
                                self.pages[page.page].url.clone(),
                                SearchHit {
                                    page: self.pages[page.page].url.clone(),
                                    individual_hits: 1,
                                    total_hits: page.frequency,
                                    page_rank: self
                                        .graph
                                        .page_rank
                                        .get(&self.pages[page.page].url)
                                        .unwrap_or(&Some(0.1))
                                        .unwrap_or(0.1),
                                    missing_keywords: words_clone
                                        .clone()
                                        .into_iter()
                                        .filter(|x| x != &word)
                                        .collect(),
                                },
                            );
                        }
                    }
                }
            }
        }
        let mut results: Vec<SearchHit> = results.into_values().collect();
        results.sort_unstable_by_key(|val| -1 * val.total_hits as isize);
        // results.sort_by(|a, b| {
        //     if a.page.domain() == b.page.domain() {
        //         (-1.0 * a.page_rank).total_cmp(&(-1.0 * b.page_rank))
        //     } else {
        //         (-1.0 * a.total_hits as f32).total_cmp(&(-1.0 * b.total_hits as f32))
        //     }
        // });
        results.sort_by_key(|val| -1 * val.individual_hits as isize);
        results
    }
}
