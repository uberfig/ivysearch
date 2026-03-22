use std::sync::{Arc, atomic::AtomicU32};

use actix_web::{App, HttpServer, web::Data};
use chrono::Local;
use ivysearch::{
    configuration::{index_info::IndexInfo, root_sites::RootSites},
    crawling::crawler::{StackElem, crawl},
    indexing::index_store::{IndexSharable, IndexStore},
    website::routes::get_api_routes,
};
use tokio::sync::RwLock;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let sites = RootSites::get().await;

    let word_index = IndexStore::get().await;

    let word_index = IndexSharable::new(word_index);
    let index_info = IndexInfo::get().await;
    let stale = index_info.is_stale();

    let mut crawlstack: Vec<StackElem> = Vec::new();
    for site in &sites.sites {
        crawlstack.push(StackElem { depth: index_info.crawl_depth, site_depth: index_info.site_depth, page: site.clone() });
    }
    let crawlstack = Arc::new(RwLock::new(crawlstack));

    if false {
        word_index.init_pagerank().await;
        for _ in 0..20 {
            word_index.pagerank_iteration().await;
        }
        println!("finished pagerank, writing");
        word_index
            .write()
            .await
            .expect("failed to write word index");
        println!("finished writing");
    }

    if stale {
        let active_threads = Arc::new(AtomicU32::new(0));
        println!("index stale, begin indexing");
        for _ in 0..index_info.num_of_runners {
            active_threads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let active_threads = active_threads.clone();
                let word_index = word_index.clone();
                let crawlstack = crawlstack.clone();
                actix_rt::spawn(async move {
                    println!(
                        "running with active threads of {}",
                        active_threads.load(std::sync::atomic::Ordering::SeqCst)
                    );
                    crawl(
                        word_index.clone(),
                        crawlstack.clone(),
                    )
                    .await;
                    println!("finished crawling");

                    let remain =
                        active_threads.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) - 1;
                    println!("remain {}", remain);
                    if remain == 0 {
                        IndexInfo::set_indexed(Some(Local::now())).await;
                        println!("last runner finished");
                        word_index.init_pagerank().await;
                        for _ in 0..20 {
                            word_index.pagerank_iteration().await;
                        }
                        println!("finished pagerank, writing");
                        word_index
                            .write()
                            .await
                            .expect("failed to write word index");
                        println!("finished writing");
                    }
                });
        }
    }

    HttpServer::new(move || {
        App::new()
            .app_data(Data::new(word_index.clone()))
            .service(get_api_routes())
    })
    .bind(("127.0.0.1", index_info.port))?
    .run()
    .await
}
