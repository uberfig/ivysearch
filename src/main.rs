use std::sync::{Arc, atomic::AtomicU32};

use actix_web::{App, HttpServer, web::Data};
use chrono::Local;
use ivysearch::{
    configuration::{index_info::IndexInfo, root_sites::RootSites},
    crawling::crawler::crawl_recursive,
    indexing::index_store::{IndexSharable, IndexStore},
    website::routes::get_api_routes,
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let sites = RootSites::get().await;

    let word_index = IndexStore::get().await;

    let word_index = IndexSharable::new(word_index);
    let index_info = IndexInfo::get().await;
    let stale = index_info.is_stale();

    let mut remain = sites.sites.clone();
    let mut divvied = Vec::new();

    if false {
        word_index.init_pagerank().await;
        for _ in 0..4 {
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
            if remain.len() > 2 {
                let group_split = remain.len() / index_info.num_of_runners;
                if group_split >= 1 {
                    let val = remain.split_off(group_split);
                    divvied.push(val);
                } else {
                    let val = remain.split_off(remain.len() / 2);
                    divvied.push(val);
                }
            } else {
                divvied.push(remain);
                break;
            }
        }
        for _ in 0..index_info.num_of_runners {
            if let Some(sites) = divvied.pop() {
                active_threads.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let active_threads = active_threads.clone();
                let word_index = word_index.clone();
                actix_rt::spawn(async move {
                    println!(
                        "running with active threads of {}",
                        active_threads.load(std::sync::atomic::Ordering::SeqCst)
                    );
                    crawl_recursive(
                        sites,
                        word_index.clone(),
                        index_info.crawl_depth,
                        index_info.site_depth,
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
                        for _ in 0..4 {
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
