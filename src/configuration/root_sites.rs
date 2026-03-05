use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use url::Url;

const FILE_PATH: &str = "root_sites.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct RootSites {
    pub sites: Vec<Url>,
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
        Self { sites: vec![] }
    }
}
