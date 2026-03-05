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
        let pages = self.page_rank.clone();
        for page in pages.keys() {
            let mut rank: f32 = 0.0; 
            for incoming_link in self.incoming.get(page).expect(&format!("{} incoming links missing, graph incorrect", page.as_str())) {
                let divider = self.outgoing.get(incoming_link).expect("outgoing links missing").len();
                if divider != 0 {
                    rank += pages.get(incoming_link).expect("page rank missing graph incorrect") / divider as f32;
                }                
            }
            *self.page_rank.get_mut(page).expect("page missing") = rank;
        }
    }
}
