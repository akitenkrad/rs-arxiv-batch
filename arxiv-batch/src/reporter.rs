use crate::cache::{AuthorCache, Cache, PaperCache};
use crate::common::{Author, Paper, StatusCode};
use crate::utils::s;
use anyhow::Result;
use chrono::Datelike;
use fxhash::FxHashMap;
use indicatif::ProgressBar;
use notion_tools::structs::block::*;
use notion_tools::structs::common::*;
use notion_tools::structs::page::{Page, PageProperty};
use notion_tools::structs::query_filter::{FilterItem, QueryFilter, RichTextFilterItem};
use notion_tools::Notion;
use tokio::time::sleep;

pub struct Reporter {}

impl Reporter {
    pub fn new() -> Reporter {
        Reporter {}
    }

    fn get_pbar(&self, total: u64) -> ProgressBar {
        let pbar = ProgressBar::new(total);
        pbar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:60.blue/green}] {pos}/{len} ({eta})",
                )
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        return pbar;
    }

    pub async fn get_an_author_notion_id(&self, ss_id: &str) -> Option<String> {
        let mut notion = Notion::new();
        notion.database(std::env::var("NOTION_AUTHOR_DATABASE_ID").unwrap());
        let mut filter = QueryFilter::new();
        filter.args(FilterItem::rich_text(
            String::from("SS ID"),
            RichTextFilterItem::equals(String::from(ss_id)),
        ));
        let response = notion.query_database(filter).await;
        match response {
            Ok(response) => {
                if response.results.len() > 0 {
                    let page = response.results.first().unwrap();
                    return Some(page.id.clone());
                } else {
                    return None;
                }
            }
            Err(_) => {
                return None;
            }
        }
    }

    pub async fn add_authors(
        &self,
        authors: &mut Vec<Author>,
        cache: &mut Cache,
    ) -> Result<StatusCode> {
        let mut notion = Notion::new();
        notion.database(std::env::var("NOTION_AUTHOR_DATABASE_ID").unwrap());

        let pbar = self.get_pbar(authors.len() as u64);
        pbar.set_style(
            indicatif::ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:60.green/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("█▓▒░"),
        );
        pbar.set_message("Adding authors to database...");
        for author in authors {
            // check if the author already exists
            if cache.is_exist_author(&author.ss_id) {
                pbar.inc(1);
                pbar.set_message("Author already exists");
                sleep(std::time::Duration::from_millis(10)).await;
                continue;
            } else {
                pbar.set_message("Adding author to database...");
            }

            // create notion page
            let mut properties: FxHashMap<String, PageProperty> = FxHashMap::default();
            properties.insert(
                s("SS ID"),
                PageProperty::title(RichText::from_str(author.ss_id.clone())),
            );
            properties.insert(
                s("Name"),
                PageProperty::rich_text(vec![RichText::from_str(author.name.clone())]),
            );
            properties.insert(
                s("Affiliations"),
                PageProperty::multi_select(
                    author
                        .affiliations
                        .iter()
                        .map(|x| x.clone())
                        .collect::<Vec<String>>(),
                ),
            );
            properties.insert(
                s("Citation Count"),
                PageProperty::number(author.citation_count as f64),
            );
            properties.insert(
                s("Paper Count"),
                PageProperty::number(author.paper_count as f64),
            );
            properties.insert(s("h-Index"), PageProperty::number(author.h_index as f64));
            properties.insert(s("URL"), PageProperty::url(author.url.clone()));

            let mut page = Page::from_properties(properties);
            page.parent.type_name = ParentType::Database;
            page.parent.database_id = Some(notion.database_id.clone());

            let response = notion.create_a_page(&page).await;
            match response {
                Ok(page) => {
                    author.page_id = page.id.clone();

                    // update cache
                    let author_cache = AuthorCache {
                        name: author.name.clone(),
                        ss_id: author.ss_id.clone(),
                        page_id: author.page_id.clone(),
                    };
                    cache.add_author(author_cache);
                    cache.save()?;
                }
                Err(e) => {
                    return Ok(StatusCode::Failure(format!(
                        "Failed to add author to database: {}",
                        e.to_string()
                    )))
                }
            }
            pbar.inc(1);
        }
        pbar.finish_and_clear();

        return Ok(StatusCode::Success);
    }

    pub async fn update_page_content(&self, paper: &Paper, page_id: String) -> StatusCode {
        let mut blocks: Vec<Block> = Vec::new();
        blocks.push(Block::heading_1(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("Summary")],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("1. Overview")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.overview.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("2. Research Question")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.research_question.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("3. Task")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.task_category.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("4. Comparison with Related Works")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(
                paper.summary.comparison_with_related_works.clone(),
            )],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("5. Methodology")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.proposed_method.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("6. Datasets")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.datasets.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("7. Experiments")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.experiments.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("8. Analysis")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.analysis.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("9. Contributions")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.contributions.clone())],
        ));

        blocks.push(Block::heading_2(
            ParentType::Page,
            page_id.clone(),
            vec![String::from("10. Future Works")],
        ));
        blocks.push(Block::paragraph(
            ParentType::Page,
            page_id.clone(),
            vec![String::from(paper.summary.future_works.clone())],
        ));

        let mut notion = Notion::new();
        notion.database(std::env::var("NOTION_PAPER_DATABASE_ID").unwrap());
        match notion.append_block_children(page_id.clone(), blocks).await {
            Ok(_) => {
                return StatusCode::Success;
            }
            Err(e) => {
                return StatusCode::Failure(format!(
                    "Failed to update page content: {}",
                    e.to_string()
                ));
            }
        }
    }

    pub async fn add_a_paper(&self, paper: &mut Paper, cache: &mut Cache) -> Result<StatusCode> {
        // check if the paper already exists
        if cache.is_exist_paper(&paper.title) {
            return Ok(StatusCode::PaperAlreadyExists);
        }

        // get notion properties
        let mut properties: FxHashMap<String, PageProperty> = FxHashMap::default();

        if paper.authors.len() == 0 {
            println!("DEBUG: Authors are empty: {}", paper.title);
            println!("DEBUG: {:?}", paper);
        }

        properties.insert(
            s("Name"),
            PageProperty::title(RichText::from_str(format!(
                "{} ({}, {})",
                paper.title.clone(),
                paper.authors.first().unwrap().name.clone(),
                paper.publication_date.year()
            ))),
        );
        properties.insert(
            s("arXiv ID"),
            PageProperty::rich_text(vec![RichText::from_str(paper.arxiv_id.clone())]),
        );
        properties.insert(
            s("SS ID"),
            PageProperty::rich_text(vec![RichText::from_str(paper.ss_id.clone())]),
        );
        properties.insert(
            s("Title"),
            PageProperty::rich_text(vec![RichText::from_str(paper.title.clone())]),
        );
        properties.insert(
            s("Year"),
            PageProperty::number(paper.publication_date.year() as f64),
        );
        properties.insert(
            s("Abstract"),
            PageProperty::rich_text(vec![RichText::from_str(paper.abstract_text.clone())]),
        );
        properties.insert(
            s("PrimaryCategory"),
            PageProperty::select(paper.arxiv_primary_category.clone()),
        );
        properties.insert(s("Journal"), PageProperty::select(paper.journal.clone()));
        properties.insert(
            s("Publisher"),
            PageProperty::select(paper.publisher.clone()),
        );
        properties.insert(s("URL"), PageProperty::url(paper.url.clone()));
        properties.insert(
            s("DOI"),
            PageProperty::rich_text(vec![RichText::from_str(paper.doi.clone())]),
        );
        properties.insert(s("Status"), PageProperty::status(s("Ready")));
        properties.insert(
            s("Citation Count"),
            PageProperty::number(paper.citation_count as f64),
        );
        properties.insert(
            s("Reference Count"),
            PageProperty::number(paper.reference_count as f64),
        );
        properties.insert(
            s("Influential Citation Count"),
            PageProperty::number(paper.influential_citation_count as f64),
        );
        if paper.keywords.len() > 0 {
            properties.insert(
                s("Keywords"),
                PageProperty::multi_select(
                    paper
                        .keywords
                        .iter()
                        .map(|x| x.alias.clone())
                        .collect::<Vec<String>>(),
                ),
            );
        }
        properties.insert(
            s("Domain"),
            PageProperty::multi_select(paper.summary.domain_as_vec()),
        );
        if paper.summary.task_as_vec().len() > 0 {
            properties.insert(
                s("Task"),
                PageProperty::multi_select(paper.summary.task_as_vec()),
            );
        }
        properties.insert(
            s("Research Question"),
            PageProperty::rich_text(vec![RichText::from_str(
                paper.summary.research_question.clone(),
            )]),
        );
        properties.insert(
            s("Methodology"),
            PageProperty::rich_text(vec![RichText::from_str(
                paper.summary.proposed_method.clone(),
            )]),
        );
        properties.insert(
            s("Results"),
            PageProperty::rich_text(vec![RichText::from_str(paper.summary.experiments.clone())]),
        );
        properties.insert(s("Status"), PageProperty::status(s("Ready")));

        let mut author_ids = paper
            .authors
            .iter()
            .map(|x| {
                cache
                    .author_map
                    .get(&x.ss_id)
                    .unwrap_or(&String::new())
                    .to_string()
            })
            .filter(|x| x.len() > 0)
            .collect::<Vec<String>>();
        if author_ids.len() > 100 {
            author_ids = author_ids.drain(0..100).collect();
        }
        properties.insert(s("Author IDs"), PageProperty::relation(author_ids.clone()));
        if let Some(first_author) = paper.authors.first() {
            let author_id = cache.get_author_id(&first_author.ss_id).unwrap();
            if author_id.len() > 0 {
                properties.insert(
                    s("First Author ID"),
                    PageProperty::relation(vec![author_id]),
                );
            }
        }

        // create notion page
        let mut notion = Notion::new();
        notion.database(std::env::var("NOTION_PAPER_DATABASE_ID").unwrap());
        let mut page = Page::from_properties(properties);
        page.parent.type_name = ParentType::Database;
        page.parent.database_id = Some(notion.database_id.clone());
        let response = notion.create_a_page(&page).await;
        match response {
            Ok(page) => {
                paper.page_id = page.id.clone();
                let result = self.update_page_content(paper, paper.page_id.clone()).await;
                match result {
                    StatusCode::Success => {
                        // update cache
                        let paper_cache = PaperCache::from_paper(&paper, None);
                        cache.add_paper(paper_cache);
                        cache.save()?;
                        return Ok(StatusCode::Success);
                    }
                    StatusCode::Failure(e) => {
                        return Ok(StatusCode::Failure(format!(
                            "Failed to update page content: {}",
                            e
                        )))
                    }
                    _ => return Ok(StatusCode::Failure("Unknown error".to_string())),
                }
            }
            Err(e) => {
                return Ok(StatusCode::Failure(format!(
                    "Failed to add paper to database: {}",
                    e.to_string()
                )))
            }
        }
    }
}
