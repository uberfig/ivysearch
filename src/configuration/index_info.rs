use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

const FILE_PATH: &str = "index_info.toml";

#[derive(Debug, Serialize, Deserialize)]
pub struct IndexInfo {
    pub last_indexed: Option<DateTime<Local>>,
    pub crawl_depth: usize,
    /// how deep do we go in a single site
    pub site_depth: usize,
    /// after how many days should we treat the index as stale
    pub index_stale_days: Option<usize>,
    pub num_of_runners: usize,
    pub port: u16,
    /// enable/disable crawling if out of date on run
    pub run_crawler: bool,
}

impl IndexInfo {
    pub async fn get() -> IndexInfo {
        match fs::read_to_string(FILE_PATH).await {
            Ok(contents) => toml::from_str(contents.as_str()).expect("invalid config on disk"),
            Err(_) => {
                let new = Self::new();
                let mut file = File::create(FILE_PATH).await.expect("failed to init file");
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
            last_indexed: None,
            crawl_depth: 1,
            site_depth: 4,
            index_stale_days: Some(7),
            num_of_runners: 4,
            port: 8080,
            run_crawler: true,
        }
    }
    pub async fn set_indexed(indexed: Option<DateTime<Local>>) {
        let mut info = Self::get().await;
        info.last_indexed = indexed;
        info.write().await.expect("failed to write");
    }
    async fn write(&self) -> std::io::Result<()> {
        let mut file = File::create(FILE_PATH).await?;
        file.write_all(toml::to_string_pretty(&self).unwrap().as_bytes())
            .await?;
        file.flush().await?;
        Ok(())
    }
    pub fn is_stale(&self) -> bool {
        match self.last_indexed {
            Some(indexed) => {
                if let Some(stale) = self.index_stale_days {
                    let now = Utc::now();
                    let diff = now.signed_duration_since(indexed);
                    if diff.num_days() >= stale.try_into().unwrap() {
                        return true;
                    }
                }
            }
            None => return true,
        }
        return false;
    }
}
