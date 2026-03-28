use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use url::Url;


#[derive(Debug, Serialize, Deserialize)]
pub struct GraphStore {
    /// the links pointing to this page
    pub incoming: HashMap<Url, Vec<Url>>,
    /// the links that this page points to
    pub outgoing: HashMap<Url, Vec<Url>>,
    pub page_rank: HashMap<Url, f32>,
}

impl GraphStore {
    pub fn new() -> Self {
        Self {
            incoming: HashMap::new(),
            outgoing: HashMap::new(),
            page_rank: HashMap::new(),
        }
    }
    pub fn set_page_links(&mut self, page: Url, outgoing: Vec<Url>) {
        if let Some(old_outgoing) = self.outgoing.get(&page) {
            // page has been changed, remove all existing
            for link in old_outgoing {
                if let Some(links) = self.incoming.get_mut(link) {
                    let pos = links.iter().position(|n| n == link);
                    if let Some(pos) = pos {
                        links.remove(pos);
                    }
                }
            }
        }
        else {
            // not in yet, make sure we init
            self.incoming.insert(page.clone(), vec![]);
        }
        for out in &outgoing {
            if let None = self.outgoing.get(out) {
                // this outgoing link hasn't been init yet
                self.outgoing.insert(out.clone(), vec![]);
            }
            match self.incoming.get_mut(out) {
                Some(incoming) => incoming.push(page.clone()),
                None => {
                    self.incoming.insert(out.clone(), vec![page.clone()]);
                },
            }
        }
        self.page_rank.insert(page.clone(), 0.1);
        self.outgoing.insert(page, outgoing);
    }
    pub fn init_pagerank(&mut self) {
        let start_val: f32 = 1.0 / self.page_rank.len() as f32;
        println!("initing pagerank with start val {}", start_val);
        for (_, rank) in &mut self.page_rank {
            *rank = start_val;
        }
    }
    pub fn pagerank_iteration(&mut self) {
        let mut new_ranks = HashMap::with_capacity(self.page_rank.len());
        const DAMPENING: f32 = 0.85;
        let n = self.page_rank.len() as f32;

        for page in self.page_rank.keys() {
            let mut rank = 0.0;
            for incoming in self.incoming.get(page).expect(&format!("{} incoming links missing, graph incorrect", page.as_str())) {
                rank += self.page_rank.get(incoming).unwrap() / self.outgoing.get(incoming).expect("outgoing links missing").len() as f32;
            }
            rank = (1.0 - DAMPENING) / n + DAMPENING * rank;
            new_ranks.insert(page.clone(), rank);
        }
        self.page_rank = new_ranks;
    }
}
