use crate::common::{Author, Paper};
use anyhow::Result;
use dotenvy::dotenv;
use fxhash::FxHashMap;
use indicatif::ProgressBar;
use notion_tools::structs::query_filter::{
    FilterItem, QueryFilter, RichTextFilterItem, StatusFilterItem,
};
use notion_tools::Notion;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperCache {
    pub title: String,
    pub ss_id: String,
    pub page_id: String,
    #[serde(skip_serializing_if = "String::is_empty", default = "String::new")]
    pub failed_reason: String,
}

impl PaperCache {
    pub fn from_paper(paper: &Paper, failed_reason: Option<String>) -> PaperCache {
        PaperCache {
            title: paper.title.clone(),
            ss_id: paper.ss_id.clone(),
            page_id: paper.page_id.clone(),
            failed_reason: match failed_reason {
                Some(reason) => reason,
                None => String::new(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorCache {
    pub name: String,
    pub ss_id: String,
    pub page_id: String,
}

impl AuthorCache {
    pub fn from_author(author: &Author) -> AuthorCache {
        AuthorCache {
            name: author.name.clone(),
            ss_id: author.ss_id.clone(),
            page_id: author.page_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(skip_serializing, default = "PathBuf::default")]
    pub path: PathBuf,
    pub papers: Vec<PaperCache>,
    pub failed_papers: Vec<PaperCache>,
    pub authors: Vec<AuthorCache>,
    pub author_map: FxHashMap<String, String>,
}

impl Cache {
    pub fn new() -> Cache {
        dotenv().ok();
        let cache_dir = std::env::var("CACHE_DIR").unwrap_or(String::from(".cache"));
        let path = Path::new(&cache_dir).join("cache.json");
        Cache {
            path,
            papers: Vec::new(),
            failed_papers: Vec::new(),
            authors: Vec::new(),
            author_map: FxHashMap::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Path::new(&self.path);
        let parent = path.parent().unwrap();
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }

        // backup
        let org_path = path.with_extension("org.json");
        std::fs::copy(path, org_path)?;

        // save
        std::fs::write(path, serde_json::to_string(&self)?)?;
        return Ok(());
    }

    pub async fn build() -> Result<Cache> {
        let mut cache = Cache::new();
        let mut notion = Notion::new();

        // load papers
        notion.database(std::env::var("NOTION_PAPER_DATABASE_ID").unwrap());
        let mut filter = QueryFilter::new();
        filter.args(FilterItem::status(
            String::from("Status"),
            StatusFilterItem::is_not_empty(),
        ));

        let mut has_more = true;
        let pb = ProgressBar::new(1);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:10.green/blue}]: {msg}")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        pb.inc(1);
        pb.set_message("Loading papers...");
        while has_more {
            let response = match notion.query_database(filter.clone()).await {
                Ok(response) => response,
                Err(e) => {
                    pb.println(format!(
                        "Failed to load papers from database: {}",
                        e.to_string()
                    ));
                    continue;
                }
            };
            has_more = response.has_more.unwrap_or(false);
            filter.start_cursor = response.next_cursor.unwrap_or(String::new());
            cache
                .papers
                .extend(response.results.iter().map(|x| PaperCache {
                    title: x.properties.get("Title").unwrap().get_value(),
                    ss_id: x.properties.get("SS ID").unwrap().get_value(),
                    page_id: x.id.clone(),
                    failed_reason: String::new(),
                }));
            pb.set_message(format!(
                "Loading papers... {} papers loaded",
                cache.papers.len()
            ));
        }
        pb.finish_and_clear();

        // load authors
        notion.database(std::env::var("NOTION_AUTHOR_DATABASE_ID").unwrap());
        let mut filter = QueryFilter::new();
        filter.args(FilterItem::rich_text(
            String::from("Name"),
            RichTextFilterItem::is_not_empty(),
        ));
        let mut has_more = true;
        let pb = ProgressBar::new(1);
        pb.set_style(
            indicatif::ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:20.green/blue}]: {msg}")
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        pb.inc(1);
        pb.set_message("Loading authors...");
        while has_more {
            let response = match notion.query_database(filter.clone()).await {
                Ok(response) => response,
                Err(e) => {
                    pb.println(format!(
                        "Failed to load authors from database: {}",
                        e.to_string()
                    ));
                    continue;
                }
            };
            has_more = response.has_more.unwrap_or(false);
            filter.start_cursor = response.next_cursor.unwrap_or(String::new());
            cache
                .authors
                .extend(response.results.iter().map(|x| AuthorCache {
                    name: x.properties.get("Name").unwrap().get_value(),
                    ss_id: x.properties.get("SS ID").unwrap().get_value(),
                    page_id: x.id.clone(),
                }));
            pb.set_message(format!(
                "Loading authors... {} authors loaded",
                cache.authors.len()
            ));
        }
        pb.finish_and_clear();

        // construct author map
        cache.author_map = cache
            .authors
            .iter()
            .map(|x| {
                let ssid = x.ss_id.clone();
                let page_id = x.page_id.clone();
                (ssid, page_id)
            })
            .collect();

        // save cache
        cache.save()?;

        return Ok(cache);
    }

    pub fn load() -> Result<Cache> {
        let cache = Cache::new();
        let path = Path::new(&cache.path);
        if path.exists() {
            let mut cache = serde_json::from_str::<Cache>(&std::fs::read_to_string(path)?)?;
            cache.path = path.to_path_buf();
            return Ok(cache);
        } else {
            return Ok(Cache::new());
        }
    }

    pub fn is_exist_paper(&self, title: &str) -> bool {
        return self
            .papers
            .iter()
            .any(|x| x.title.to_lowercase() == title.to_lowercase());
    }

    pub fn is_exist_author(&self, ss_id: &str) -> bool {
        return self.author_map.contains_key(ss_id);
    }

    pub fn get_author_id(&self, ss_id: &str) -> Option<String> {
        return self.author_map.get(ss_id).cloned();
    }

    pub fn add_paper(&mut self, paper: PaperCache) {
        self.papers.push(paper);
    }

    pub fn add_author(&mut self, author: AuthorCache) {
        self.authors.push(author.clone());
        self.author_map
            .insert(author.ss_id.clone(), author.page_id.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache() {
        let cache = match Cache::load() {
            Ok(cache) => cache,
            Err(e) => {
                println!("Failed to load cache: {}", e.to_string());
                Cache::new()
            }
        };
        assert!(cache.papers.len() > 0);
        println!("{:?}", cache.path);
        println!("{}", cache.papers.len());
        println!("{}", cache.authors.len());
    }
}
