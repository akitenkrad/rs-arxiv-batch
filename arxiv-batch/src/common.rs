use crate::utils::s;
use anyhow::Result;
use chrono::{DateTime, Datelike, Utc};
use fxhash::FxHashMap;
use keywords::rsc::{extract_keywords, load_keywords, Keyword, Language};
use rsrpp::parser::parse;
use rsrpp::parser::structs::{ParserConfig, Section};
use serde::{Deserialize, Serialize};

pub enum StatusCode {
    Success,
    Failure(String),
    PaperAlreadyExists,
}

#[derive(Clone, Debug)]
pub struct Author {
    pub page_id: String,
    pub ss_id: String,
    pub name: String,
    pub url: String,
    pub affiliations: Vec<String>,
    pub paper_count: u32,
    pub citation_count: u32,
    pub h_index: u32,
}

impl Default for Author {
    fn default() -> Self {
        Self {
            page_id: "".to_string(),
            ss_id: "".to_string(),
            name: "John Doe".to_string(),
            url: "".to_string(),
            affiliations: Vec::new(),
            paper_count: 0,
            citation_count: 0,
            h_index: 0,
        }
    }
}

impl Author {
    pub fn from_ss_author(ss_author: &ss_tools::structs::Author) -> Self {
        Self {
            page_id: "".to_string(),
            ss_id: match ss_author.author_id.clone() {
                Some(id) => id,
                None => "".to_string(),
            },
            name: ss_author.name.clone().unwrap(),
            url: ss_author.url.clone().unwrap_or(String::from("-")),
            affiliations: ss_author.affiliations.clone().unwrap_or_default(),
            paper_count: ss_author.paper_count.unwrap_or(0),
            citation_count: ss_author.citation_count.unwrap_or_default(),
            h_index: ss_author.hindex.unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Summary {
    pub is_survey: bool,
    pub overview: String,
    pub research_question: String,
    pub task_category: String,
    pub task_as_words: String,
    pub comparison_with_related_works: String,
    pub proposed_method: String,
    pub datasets: String,
    pub domain_as_words: String,
    pub experiments: String,
    pub analysis: String,
    pub contributions: String,
    pub future_works: String,
}

impl Summary {
    pub fn task_as_vec(&self) -> Vec<String> {
        if self.task_as_words.contains(",") {
            return self
                .task_as_words
                .split(",")
                .map(|s| s.to_string())
                .collect();
        } else if self.task_as_words.contains("、") {
            return self
                .task_as_words
                .split("、")
                .map(|s| s.to_string())
                .collect();
        } else {
            return vec![self.task_as_words.clone()];
        }
    }

    pub fn domain_as_vec(&self) -> Vec<String> {
        if self.domain_as_words.contains(",") {
            return self
                .domain_as_words
                .split(",")
                .map(|s| s.to_string())
                .collect();
        } else if self.domain_as_words.contains("、") {
            return self
                .domain_as_words
                .split("、")
                .map(|s| s.to_string())
                .collect();
        } else {
            return vec![self.domain_as_words.clone()];
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Paper {
    pub page_id: String,
    pub arxiv_id: String,
    pub ss_id: String,
    pub title: String,
    pub authors: Vec<Author>,
    pub abstract_text: String,
    pub publication_date: DateTime<Utc>,
    pub keywords: Vec<Keyword>,
    pub arxiv_primary_category: String,
    pub arxiv_categories: Vec<String>,
    pub url: String,
    pub doi: String,
    pub journal: String,
    pub publisher: String,
    pub bibtex: String,
    pub citation_count: u32,
    pub influential_citation_count: u32,
    pub reference_count: u32,
    pub citations: Vec<Paper>,
    pub references: Vec<Paper>,
    pub original_text_map: FxHashMap<String, Section>,
    pub original_text: Vec<Section>,
    pub summary: Summary,
}

impl Paper {
    pub fn reference(
        ss_id: &str,
        title: &str,
        abstract_text: &str,
        authors: Vec<Author>,
        publication_date: DateTime<Utc>,
    ) -> Self {
        Self {
            ss_id: ss_id.to_string(),
            title: title.to_string(),
            abstract_text: abstract_text.to_string(),
            authors,
            publication_date,
            ..Default::default()
        }
    }

    pub fn get_keywords(&mut self) -> Result<&mut Self> {
        let mut target_text = String::new();
        target_text.push_str(&self.title);
        target_text.push_str("\n\n");
        target_text.push_str(&self.abstract_text);

        if self.original_text_map.contains_key("Introduction") {
            let section = self.original_text_map.get("Introduction").unwrap();
            let paragraphs = section.contents.join("\n");
            target_text.push_str("\n\n");
            target_text.push_str(&paragraphs);
        }

        let keywords = load_keywords();
        self.keywords = extract_keywords(&target_text, keywords, Language::English);
        self.keywords = self
            .keywords
            .iter()
            .filter(|k| k.score > 5)
            .cloned()
            .collect();
        return Ok(self);
    }

    pub async fn get_original_text(
        &mut self,
        pdf: Option<String>,
        verbose: bool,
    ) -> Result<&mut Self> {
        let pdf = match pdf {
            Some(pdf) => pdf,
            None => {
                assert!(
                    self.url.len() > 0,
                    "Failed to get original text: URL is empty."
                );
                self.url.clone()
            }
        };

        let mut parser_config = ParserConfig::new();
        let pages = parse(&pdf, &mut parser_config, verbose).await?;
        let sections = Section::from_pages(&pages);

        self.original_text = sections.clone();
        self.original_text_map = FxHashMap::default();
        for section in sections {
            self.original_text_map
                .insert(section.title.clone(), section);
        }
        return Ok(self);
    }

    pub fn original_text2xml(&self) -> String {
        let mut sections = self.original_text_map.values().collect::<Vec<&Section>>();
        sections.sort_by(|a, b| a.index.cmp(&b.index));

        let mut xml = s("<paper>");
        // baseic information
        xml.push_str("<metadata>");
        xml.push_str(format!("<title>{}</title>", self.title).as_str());
        xml.push_str("<authors>");
        for author in self.authors.clone().iter() {
            xml.push_str(format!("<author>{}</author>", author.name).as_str());
        }
        xml.push_str("</authors>");
        xml.push_str("</metadata>");

        // contents
        xml.push_str("<contents>");
        for section in sections {
            xml.push_str("<section>");
            xml.push_str(format!("<title>{}</title>", section.title).as_str());
            for paragraph in section.contents.iter() {
                xml.push_str(format!("<paragraph>{}</paragraph>", paragraph).as_str());
            }
            xml.push_str("</section>");
        }
        xml.push_str("</contents>");
        xml.push_str("</paper");
        return xml;
    }

    pub fn references2xml(&self) -> String {
        let mut xml = s("<references>");
        for reference in &self.references {
            xml.push_str("<reference>");
            xml.push_str(format!("<title>{}<title>", reference.title).as_str());
            xml.push_str("<authors>");
            for author in reference.authors.clone().iter() {
                xml.push_str(format!("<author>{}</author>", author.name).as_str());
            }
            xml.push_str("</authors>");
            xml.push_str(format!("<year>{}</year>", reference.publication_date.year()).as_str());
            xml.push_str(format!("<abstract>{}</abstract>", reference.abstract_text).as_str());
            xml.push_str("</reference>");
        }
        xml.push_str("</references>");
        return xml;
    }

    pub fn citations2xml(&self) -> String {
        let mut xml = s("<citations>");
        for citation in &self.citations {
            xml.push_str("<citation>");
            xml.push_str(format!("<title>{}<title>", citation.title).as_str());
            xml.push_str("<authors>");
            for author in citation.authors.clone().iter() {
                xml.push_str(format!("<author>{}</author>", author.name).as_str());
            }
            xml.push_str("</authors>");
            xml.push_str(format!("<year>{}</year>", citation.publication_date.year()).as_str());
            xml.push_str(format!("<abstract>{}</abstract>", citation.abstract_text).as_str());
            xml.push_str("</citation>");
        }
        xml.push_str("</citations>");
        return xml;
    }
}
