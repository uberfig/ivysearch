use rand::seq::IndexedRandom;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use url::Url;

const FILE_PATH: &str = "root_sites.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct RootSites {
    pub sites: Vec<Url>,
    /// list of sites to index but do not probe further
    pub no_depth: Vec<Url>,
    pub blacklist: Vec<Url>,
    pub exclude_prefix: Vec<Url>,
}

impl RootSites {
    pub async fn get() -> RootSites {
        match fs::read_to_string(FILE_PATH).await {
            Ok(contents) => toml::from_str(contents.as_str()).expect("invalid config on disk"),
            Err(_) => {
                let new = Self::new();
                let mut file = fs::File::create(FILE_PATH)
                    .await
                    .expect("failed to init file");
                file.write_all(toml::to_string_pretty(&new).unwrap().as_bytes())
                    .await
                    .expect("failed to write to new file");
                file.flush().await.expect("failed to flush");
                new
            }
        }
    }
    fn new() -> Self {
        Self {
            sites: vec![],
            blacklist: vec![],
            no_depth: vec![],
            exclude_prefix: vec![],
        }
    }
    pub fn get_random(mut self) -> Url {
        let mut combined = self.sites;
        combined.append(&mut self.no_depth);
        combined.choose(&mut rand::rng()).expect("no root sites").clone()
    }
}
