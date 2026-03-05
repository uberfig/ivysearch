use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

const STOP_WORDS: &'static [&str] = &[
    "a", "is", "to", "it", "its", "it's", "if", "too", "this", "i", "by", "the", "an", "and",
    "are", "be", "of", "because",
];

const PUNCTUATION: &'static [char] = &[
    '\\', '\"', '/', ',', '!', '.', '*', ':', '(', ')', '-', '#', '•', '|', '<', '>', '?', '!',
    '#'
];

#[derive(Debug, Serialize, Deserialize)]
pub struct KeywordSet {
    pub keywords: HashMap<String, usize>,
}
impl KeywordSet {
    pub fn add_occurance(&mut self, word: String) {
        let word = word.to_lowercase();
        if word.is_empty() || STOP_WORDS.contains(&word.as_str()) {
            return;
        }
        match self.keywords.get_mut(&word) {
            Some(amount) => *amount += 1,
            None => {
                self.keywords.insert(word, 1);
            }
        }
    }
    pub fn add_link(&mut self, url: &Url) {
        let link = format!(
            "{} {} {}",
            url.domain().unwrap_or(""),
            url.path(),
            url.fragment().unwrap_or("")
        );
        for word in split_string(&link) {
            self.add_occurance(word.to_string());
        }
    }
}

pub fn split_string(input: &str) -> impl Iterator<Item = &str> {
    input
        .trim()
        .split_ascii_whitespace()
        .map(|x| x.split(|y| PUNCTUATION.contains(&y)))
        .flatten()
}

pub fn get_keywords(children: scraper::ElementRef<'_>) -> KeywordSet {
    let mut keywords = KeywordSet {
        keywords: HashMap::new(),
    };
    keyword_recursive(children, &mut keywords);
    keywords
}

fn combine_svg_text(parent: scraper::ElementRef<'_>) -> String {
    let mut builder: Vec<String> = Vec::new();
    for element in parent.child_elements() {
        if element.value().name() == "text" || element.value().name() == "tspan" {
            let text = element.text().collect::<Vec<_>>().join("");
            if !text.is_empty() {
                builder.push(text.replace(" ", "-"));
            }
        }
        builder.push("|".to_string());
        builder.push(combine_svg_text(element));
    }
    // for element in children {
    //     match element {
    //         Node::Text(text) => builder.push(text.clone()),
    //         Node::Element(element) => {
    //             builder.push(" ".to_string());
    //             builder.push(combine_svg_text(&element.children))
    //         },
    //         Node::Comment(_) => {}
    //     }
    // }
    builder
        .join("")
        .replace("’", "'")
        .replace("||", "-")
        .replace("|", "")
        .replace("-", " ")
}

fn insert_keywords(text: &str, keywords: &mut KeywordSet) {
    for word in split_string(text) {
        keywords.add_occurance(word.to_string());
    }
}

fn keyword_recursive(parent: scraper::ElementRef<'_>, keywords: &mut KeywordSet) {
    'outer: for element in parent.child_elements() {
        // println!("in element: {}", element.value().name());
        // println!("element: {}", element.value().name());
        if element.value().name() == "script"
            || element.value().name() == "noscript"
            || element.value().name() == "style"
        {
            // println!("in style");
            continue 'outer;
        }
        if element.value().name() == "head" {
            keyword_recursive(element, keywords);
            continue 'outer;
        }
        if element.value().name() == "svg" {
            // println!("in svg");
            let combined = combine_svg_text(element);
            // println!("combined svg: {}", combined);
            insert_keywords(&combined, keywords);
            continue 'outer;
        }
        if element.value().name() == "img" {
            if let Some(alt) = element.value().attr("alt") {
                insert_keywords(alt, keywords);
            }
           continue 'outer;
        }
        let text = element.text().collect::<Vec<_>>().join("");
        // let cleaned = split_string(&text).collect::<Vec<_>>().join(" ");
        // println!("inside: {} adding text: {}", element.value().name(), cleaned);
        insert_keywords(&text, keywords);

        keyword_recursive(element, keywords);
    }
}
