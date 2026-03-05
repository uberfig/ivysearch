# IvySearch

this is a basic toy search engine written in rust

ivysearch uses a combination of keyword analyisis and pagerank to find and order matching pages

the index results are stored in memory and written to a file named word_frequency_index.json. While ideally this would actually use some sort of database for properly storing index results this is just meant to be a toy and done from scratch so it just doesn't scale very well

this does not implement robots.txt properly nor does it try to avoid spamming sites so if you use it ***do not** use it on sites that you do not have permission to scrape 

# configuring 

to configure ivysearch, you need both a root_sites.toml and a index_info.toml

## root_sites.toml

all sites to begin indexing from, try to make sure you have permission or that their robots.txt says its all good

```toml
sites = [
    "https://ivytime.gay/",
    "...",
]
```

## index_info.toml

do not make `crawl_depth` greater than one unless you are certain that  your system can handle it and that folks will not be angry with you or you properly implement robots.txt logic

```toml
crawl_depth = 1
site_depth = 4
index_stale_days = 7
num_of_runners = 4

```