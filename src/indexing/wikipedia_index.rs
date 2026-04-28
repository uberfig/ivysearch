use std::collections::HashMap;
use std::io::BufRead;
use std::sync::Arc;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;
use url::Url;

use crate::indexing::index_store::{IndexSharable, IndexStore, IndexedPage, SearchHit};
use crate::parsing::keywords::KeywordSet;

const WIKI_INDEX_FILE_PATH: &str = "wiki_titles_frequency_index.json";
const WIKI_TITLE_DUMP_PATH: &str = "wikipedia/titles/simplewiki-20260325-all-titles-in-ns-0";

pub fn to_wiki_slug(title: &str) -> String {
    title.replace(" ", "_")
}

pub fn to_url(title: &str) -> Url {
    Url::parse(&format!(
        "https://en.wikipedia.org/wiki/{}",
        to_wiki_slug(title)
    ))
    .expect("invalid wikipedia url")
}

#[derive(Debug, Clone)]
pub struct WikiSharable {
    pub store: WikiShareWrapper,
}

impl WikiSharable {
    pub async fn get() -> Self {
        match fs::read_to_string(WIKI_INDEX_FILE_PATH).await {
            Ok(contents) => {
                let store =
                    serde_json::from_str(contents.as_str()).expect("invalid config on disk");
                Self {
                    store: WikiShareWrapper::new(store),
                }
            }
            Err(_) => Self::new().await,
        }
    }
    pub async fn new() -> Self {
        let new = WikiStore::new();
        let mut file = File::create(WIKI_INDEX_FILE_PATH)
            .await
            .expect("failed to init file");
        file.write_all(serde_json::to_string(&new).unwrap().as_bytes())
            .await
            .expect("failed to write to new file");
        file.flush().await.expect("failed to flush");

        let new_self = Self {
            store: WikiShareWrapper::new(new),
        };

        let cloned = new_self.clone();

        actix_rt::spawn(async move {
            if let Ok(file) = std::fs::File::open(WIKI_TITLE_DUMP_PATH) {
                println!("beginning wiki parse");
                let reader = std::io::BufReader::new(file);
                // let mut lines = reader.lines();
                for line in reader.lines() {
                    if let Ok(line) = line {
                        cloned.insert_title_line(&line).await;
                    }
                }
                // while let Ok(Some(line)) = lines.next_line().await {

                // }
                cloned
                    .store
                    .write()
                    .await
                    .expect("failed to write updated wikipedia index");
                println!("finished wiki parse");
            }
        });

        new_self
    }

    async fn insert_title_line(&self, title: &str) {
        let set = KeywordSet::from_line(title);
        self.store
            .insert(set.keywords.into_iter(), title.to_string())
            .await;
    }
}

type WikiTitle = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct WikiStore {
    pub pages: Vec<WikiTitle>,
    pub page_ids: HashMap<WikiTitle, usize>,
    pub words: HashMap<String, Vec<IndexedPage>>,
}

impl WikiStore {
    pub fn new() -> Self {
        Self {
            words: HashMap::new(),
            pages: Vec::new(),
            page_ids: HashMap::new(),
        }
    }
    /// adds a url to metadata if doesn't exist, returns id if it does
    fn insert_title(&mut self, url: WikiTitle) -> usize {
        match self.page_ids.get(&url) {
            Some(id) => *id,
            None => {
                let id = self.pages.len();
                self.pages.push(url.clone());
                self.page_ids.insert(url, id);
                id
            }
        }
    }
    /// insert a word occurance for a given page, inits metadata if necessary
    pub fn insert(&mut self, word: String, url: WikiTitle, frequency: usize) {
        let page = self.insert_title(url);
        match self.words.get_mut(&word) {
            Some(word_pages) => {
                word_pages.push(IndexedPage { frequency, page });
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
                    match results.get_mut(&to_url(&self.pages[page.page])) {
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
                                to_url(&self.pages[page.page]),
                                SearchHit {
                                    page: to_url(&self.pages[page.page]),
                                    individual_hits: 1,
                                    total_hits: page.frequency,
                                    page_rank: 0.1,
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
        results.sort_by_key(|val| -1 * val.individual_hits as isize);
        results
    }
    pub async fn write(&self) -> std::io::Result<()> {
        let mut file = File::create(WIKI_INDEX_FILE_PATH).await?;
        file.write_all(serde_json::to_string_pretty(&self).unwrap().as_bytes())
            .await?;
        file.flush().await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WikiShareWrapper {
    store: Arc<RwLock<WikiStore>>,
}
impl WikiShareWrapper {
    pub fn new(store: WikiStore) -> Self {
        Self {
            store: Arc::new(RwLock::new(store)),
        }
    }
    pub async fn search(&self, words: Vec<String>) -> Vec<SearchHit> {
        let lock = self.store.read().await;
        lock.search(words)
    }
    /// insert word occurances for a given page, inits metadata if necessary
    pub async fn insert(&self, words: impl Iterator<Item = (String, usize)>, url: WikiTitle) {
        let mut lock = self.store.write().await;
        for (word, frequency) in words {
            lock.insert(word, url.clone(), frequency);
        }
    }
    pub async fn write(&self) -> std::io::Result<()> {
        let lock = self.store.read().await;
        lock.write().await
    }
}
