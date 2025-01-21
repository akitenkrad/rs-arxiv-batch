//! This module collects the metadata of the papers from the arXiv API.
use crate::common::{Author, Paper};
use crate::utils::{datetime_from_str, default_datetime, levenshtein_similarity};
use anyhow::{Ok, Result};
use arxiv_tools as ar;
use chrono::{DateTime, Utc};
use ss_tools as ss;

#[derive(Clone, Debug)]
pub struct Collector {
    max_retry_count: u64,
    wait_time: u64,
}

impl Default for Collector {
    fn default() -> Self {
        Collector {
            max_retry_count: 10,
            wait_time: 15,
        }
    }
}

impl Collector {
    pub fn new(max_retry_count: u64, wait_time: u64) -> Self {
        Collector {
            max_retry_count,
            wait_time,
        }
    }

    fn build_default_arxiv(target_date: Option<DateTime<Utc>>) -> ar::ArXiv {
        let category_conditions = ar::QueryParams::or(vec![
            ar::QueryParams::subject_category(ar::Category::CsAi),
            ar::QueryParams::subject_category(ar::Category::CsLg),
            ar::QueryParams::subject_category(ar::Category::CsCl),
            ar::QueryParams::subject_category(ar::Category::CsCv),
        ]);

        let args = if let Some(target_date) = target_date {
            let from = target_date.clone().format("%Y%m%d0000").to_string();
            let to = target_date.clone().format("%Y%m%d2359").to_string();
            let args = ar::QueryParams::and(vec![
                ar::QueryParams::group(vec![category_conditions]),
                ar::QueryParams::SubmittedDate(from, to),
            ]);
            args
        } else {
            category_conditions
        };

        let mut arxiv = ar::ArXiv::from_args(args);
        arxiv.max_results(500);

        return arxiv;
    }

    pub async fn collect_papers_from_arxiv(
        &self,
        target_date: DateTime<Utc>,
    ) -> Result<Vec<Paper>> {
        let mut arxiv = Self::build_default_arxiv(Some(target_date));
        let response = arxiv.query().await;
        let papers: Vec<Paper> = response
            .iter()
            .map(|entry| {
                let mut paper = Paper::default();
                paper.arxiv_id = entry.id.clone();
                paper.title = entry.title.clone().replace("\n", " ");
                paper.abstract_text = entry.abstract_text.clone();
                paper.arxiv_primary_category = entry.primary_category.clone();
                paper.arxiv_categories = entry.categories.clone();
                paper.url = entry.pdf_url.clone();
                paper.doi = entry.doi.clone();
                paper.journal = "arXiv".to_string();
                paper.publisher = "arXiv".to_string();
                paper
            })
            .collect();

        return Ok(papers);
    }

    pub async fn update_from_arxiv(&self, paper: &mut Paper, overwrite: bool) -> Result<()> {
        let title = paper.title.clone();
        let args = ar::QueryParams::title(&title);
        let mut arxiv = ar::ArXiv::from_args(args);
        arxiv.max_results(1000);
        arxiv.sort_by(ar::SortBy::Relevance);
        arxiv.sort_order(ar::SortOrder::Descending);
        let response = arxiv.query().await;

        // Find the most similar paper
        let scores = response
            .iter()
            .enumerate()
            .map(|(idx, arxiv_paper)| {
                let score = levenshtein_similarity(
                    &title.to_lowercase(),
                    &arxiv_paper.title.to_lowercase(),
                );
                (score, idx)
            })
            .collect::<Vec<(f64, usize)>>();
        let (score, idx) = scores
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();

        assert!(
            *score >= 0.9,
            "No similar paper found: most similar paper: {} vs {} ({:.3})",
            title,
            response.get(*idx).unwrap().title.clone(),
            score
        );

        // Update the paper
        let arxiv_paper = response.get(*idx).unwrap();
        paper.arxiv_id = arxiv_paper.id.clone();
        paper.title = arxiv_paper.title.clone().replace("\n", " ");
        if paper.abstract_text.is_empty() || overwrite {
            paper.abstract_text = arxiv_paper.abstract_text.clone();
        }
        if paper.arxiv_primary_category.is_empty() || overwrite {
            paper.arxiv_primary_category = arxiv_paper.primary_category.clone();
        }
        if paper.arxiv_categories.is_empty() || overwrite {
            paper.arxiv_categories = arxiv_paper.categories.clone();
        }
        if paper.url.is_empty() || overwrite {
            paper.url = arxiv_paper.pdf_url.clone();
        }
        if paper.doi.is_empty() || overwrite {
            paper.doi = arxiv_paper.doi.clone();
        }
        if paper.journal.is_empty() || overwrite {
            paper.journal = "arXiv".to_string();
        }
        if paper.publisher.is_empty() || overwrite {
            paper.publisher = "arXiv".to_string();
        }

        return Ok(());
    }

    pub async fn update_from_ss(&self, paper: &mut Paper, overwrite: bool) -> Result<()> {
        // Build the query
        let title = paper.title.clone();
        let mut ss = ss::SemanticScholar::new();
        let max_retry_count = self.max_retry_count;
        let wait_time = self.wait_time;
        let mut query_params = ss::QueryParams::default();
        query_params.query_text(&paper.title);
        query_params.fields(vec![
            ss::structs::PaperField::PaperId,
            ss::structs::PaperField::Title,
            ss::structs::PaperField::Abstract,
            ss::structs::PaperField::Authors(vec![
                ss::structs::AuthorField::AuthorId,
                ss::structs::AuthorField::Name,
                ss::structs::AuthorField::Url,
                ss::structs::AuthorField::Affiliations,
            ]),
            ss::structs::PaperField::Venue,
            ss::structs::PaperField::PaperId,
            ss::structs::PaperField::Url,
            ss::structs::PaperField::ReferenceCount,
            ss::structs::PaperField::CitationCount,
            ss::structs::PaperField::InfluentialCitationCount,
            ss::structs::PaperField::PublicationDate,
            ss::structs::PaperField::Citations(vec![
                ss::structs::PaperField::PaperId,
                ss::structs::PaperField::Title,
                ss::structs::PaperField::Abstract,
                ss::structs::PaperField::PublicationDate,
            ]),
            ss::structs::PaperField::References(vec![
                ss::structs::PaperField::PaperId,
                ss::structs::PaperField::Title,
                ss::structs::PaperField::Abstract,
                ss::structs::PaperField::PublicationDate,
            ]),
        ]);

        // Execute the query
        let response = ss
            .query_papers_by_title(query_params, max_retry_count, wait_time)
            .await?;

        // Find the most similar paper
        let scores = response
            .iter()
            .enumerate()
            .map(|(idx, ss_paper)| {
                let score = levenshtein_similarity(
                    &title.to_lowercase(),
                    &ss_paper.title.clone().unwrap().to_lowercase(),
                );
                (score, idx)
            })
            .collect::<Vec<(f64, usize)>>();
        let (score, idx) = scores
            .iter()
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .unwrap();
        assert!(
            *score >= 0.9,
            "No similar paper found: most similar paper: {} vs {} ({:.3})",
            title,
            response.get(*idx).unwrap().title.clone().unwrap(),
            score
        );

        // Update the paper
        let ss_paper = response.get(*idx).unwrap();
        paper.ss_id = ss_paper.paper_id.clone().unwrap();
        paper.title = ss_paper.title.clone().unwrap().replace("\n", " ");
        if paper.abstract_text.is_empty() || overwrite {
            paper.abstract_text = ss_paper.abstract_text.clone().unwrap();
        }
        if paper.url.is_empty() || overwrite {
            paper.url = ss_paper.url.clone().unwrap();
        }
        if paper.journal.is_empty() || overwrite {
            paper.journal = ss_paper.venue.clone().unwrap();
        }
        paper.publication_date = if let Some(publication_date) = ss_paper.publication_date.clone() {
            datetime_from_str(&publication_date)
        } else {
            default_datetime()
        };
        paper.reference_count = ss_paper.reference_count.unwrap();
        paper.citation_count = ss_paper.citation_count.unwrap();
        paper.influential_citation_count = ss_paper.influential_citation_count.unwrap();
        paper.authors = ss_paper
            .authors
            .clone()
            .unwrap()
            .iter()
            .map(|a| Author::from_ss_author(a))
            .collect::<Vec<Author>>();
        if let Some(publication_date) = ss_paper.publication_date.clone() {
            if publication_date.is_empty() || overwrite {
                paper.publication_date = datetime_from_str(&publication_date);
            }
        }
        paper.citations = ss_paper
            .citations
            .clone()
            .unwrap()
            .iter()
            .map(|c| {
                Paper::reference(
                    c.paper_id.clone().unwrap_or_default().as_str(),
                    c.title
                        .clone()
                        .unwrap_or_default()
                        .replace("\n", " ")
                        .as_str(),
                    c.abstract_text.clone().unwrap_or_default().as_str(),
                    c.authors
                        .clone()
                        .unwrap_or_default()
                        .iter()
                        .map(|a| Author::from_ss_author(a))
                        .collect(),
                    if let Some(publication_date) = c.publication_date.clone() {
                        datetime_from_str(&publication_date)
                    } else {
                        default_datetime()
                    },
                )
            })
            .collect();
        paper.references = ss_paper
            .references
            .clone()
            .unwrap()
            .iter()
            .map(|r| {
                Paper::reference(
                    r.paper_id.clone().unwrap_or_default().as_str(),
                    r.title
                        .clone()
                        .unwrap_or_default()
                        .replace("\n", " ")
                        .as_str(),
                    r.abstract_text.clone().unwrap_or_default().as_str(),
                    r.authors
                        .clone()
                        .unwrap_or_default()
                        .iter()
                        .map(|a| Author::from_ss_author(a))
                        .collect(),
                    if let Some(publication_date) = r.publication_date.clone() {
                        datetime_from_str(&publication_date)
                    } else {
                        default_datetime()
                    },
                )
            })
            .collect();

        return Ok(());
    }
}
