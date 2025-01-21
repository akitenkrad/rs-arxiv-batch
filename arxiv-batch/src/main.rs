pub mod ai;
pub mod cache;
pub mod collector;
pub mod common;
pub mod reporter;
pub mod utils;

use crate::common::StatusCode;
use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Args, Parser, Subcommand};
use dotenvy::dotenv;
use indicatif::ProgressBar;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// CLI SETTISNGS ---------------------------------------------------------------
/// Command-line interface
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Sets a custom config file: "config.toml"
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Post a new paper to Notion
    #[command(name = "post-a-new-paper")]
    PostANewPaper(PostANewPaperArgs),
    /// Post specific date's arXiv papers to Notion
    #[command(name = "post-arxiv-papers")]
    PostArxivPapers(PostArxivPapersArgs),
    #[command(name = "build-cache")]
    BuildCache,
}

#[derive(Debug, Args)]
struct PostANewPaperArgs {
    /// Title of the paper
    #[arg(long)]
    title: String,
    /// Path to the PDF file or URL
    #[arg(long)]
    pdf: Option<String>,
    /// Maximum number of retry attempts
    #[arg(long, default_value_t = 15)]
    max_retry_count: u64,
    /// Wait time in seconds between retry attempts
    #[arg(long, default_value_t = 30)]
    wait_time: u64,
    /// OpenAI model ID: "gpt-4o-mini"
    #[arg(long, default_value_t = String::from("gpt-4o-mini"))]
    model_id: String,
    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
}

#[derive(Debug, Args)]
struct PostArxivPapersArgs {
    /// Date to post papers: "YYYY-MM-DD"
    #[arg(long)]
    date: String,
    /// Maximum number of retry attempts
    #[arg(long, default_value_t = 15)]
    max_retry_count: u64,
    /// Wait time in seconds between retry attempts
    #[arg(long, default_value_t = 30)]
    wait_time: u64,
    /// OpenAI model ID: "gpt-4o-mini"
    #[arg(long, default_value_t = String::from("gpt-4o-mini"))]
    model_id: String,
    /// Verbose mode
    #[arg(short, long)]
    verbose: bool,
}

// CONFIGURATION SETTINGS -----------------------------------------------------

/// Configuration settings
#[derive(Serialize, Deserialize, Debug, Default)]
struct Config {
    /// Semantic Scholar API key
    #[serde(rename = "SEMANTIC_SCHOLAR_API_KEY", default = "String::new")]
    semantic_scholar_api_key: String,
    /// Notion API key
    #[serde(rename = "NOTION_API_KEY", default = "String::new")]
    notion_api_key: String,
    /// Notion database ID for papers
    #[serde(rename = "NOTION_PAPER_DATABASE_ID", default = "String::new")]
    notion_paper_database_id: String,
    /// Notion database ID for authors
    #[serde(rename = "NOTION_AUTHOR_DATABASE_ID", default = "String::new")]
    notion_author_database_id: String,
    /// OpenAI API key
    #[serde(rename = "OPENAI_API_KEY", default = "String::new")]
    openai_api_key: String,
    #[serde(rename = "CACHE_DIR", default = "String::new")]
    cache_dir: String,
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self> {
        let config = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&config)?;
        Ok(config)
    }

    /// Set environment variables
    pub fn set_env(&self) {
        std::env::set_var("SEMANTIC_SCHOLAR_API_KEY", &self.semantic_scholar_api_key);
        std::env::set_var("NOTION_API_KEY", &self.notion_api_key);
        std::env::set_var("NOTION_PAPER_DATABASE_ID", &self.notion_paper_database_id);
        std::env::set_var("NOTION_AUTHOR_DATABASE_ID", &self.notion_author_database_id);
        std::env::set_var("OPENAI_API_KEY", &self.openai_api_key);
        std::env::set_var("CACHE_DIR", &self.cache_dir);
    }
}

// MAIN FUNCTION --------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Load configuration settings
    if let Some(config) = cli.config.as_ref() {
        let config = match Config::load(config) {
            Ok(config) => config,
            Err(e) => {
                eprintln!("WARNING: Failed to load config: {}", e);
                return;
            }
        };
        config.set_env();
    } else {
        dotenv().ok();
    }

    match &cli.command {
        Some(Commands::PostANewPaper(args)) => {
            post_a_new_paper(
                args.title.clone(),
                args.pdf.clone(),
                args.max_retry_count,
                args.wait_time,
                args.model_id.clone(),
                args.verbose,
            )
            .await;
        }
        Some(Commands::PostArxivPapers(args)) => {
            let mut date = args.date.clone();
            date.push_str(" 00:00:00+0000");
            let date: DateTime<Utc> = DateTime::parse_from_str(&date, "%Y-%m-%d %H:%M:%S%z")
                .unwrap()
                .into();
            post_arxiv_papers(
                date,
                args.max_retry_count,
                args.wait_time,
                args.model_id.clone(),
                args.verbose,
            )
            .await;
        }
        Some(Commands::BuildCache) => {
            let result = cache::Cache::build().await;
            match result {
                Ok(cache) => {
                    println!("Finished building cache.");
                    match cache.save() {
                        Ok(_) => {
                            println!("Finished saving cache: {:?}", cache.path);
                        }
                        Err(e) => {
                            eprintln!("WARNING: Failed to save cache: {}", e);
                        }
                    }
                }
                Err(e) => {
                    eprintln!("WARNING: Failed to build cache: {}", e);
                }
            }
        }
        None => {
            eprintln!("WARNING: No subcommand specified.");
        }
    }
}

// SUBCOMMANDS ----------------------------------------------------------------

async fn post_a_new_paper(
    title: String,
    pdf: Option<String>,
    max_retry_count: u64,
    wait_time: u64,
    model_id: String,
    verbose: bool,
) {
    let time = std::time::Instant::now();

    // Load cache
    let mut cache = match cache::Cache::load() {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("WARNING: Failed to load cache: {}", e);
            return;
        }
    };

    let mut paper = common::Paper::default();
    paper.title = title;

    // Collect paper metadata
    let collector = collector::Collector::new(max_retry_count, wait_time);
    let reporter = reporter::Reporter::new();
    let ai = ai::AI::new(&model_id);

    match collector.update_from_ss(&mut paper, true).await {
        Ok(_) => {
            if verbose {
                println!(
                    "Finished collecting paper metadata from Semantic Scholar: {:.2}s",
                    time.elapsed().as_secs_f32()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "WARNING: Failed to collect paper metadata from Semantic Scholar: {}",
                e
            );
        }
    }
    match collector.update_from_arxiv(&mut paper, true).await {
        Ok(_) => {
            if verbose {
                println!(
                    "Finished collecting paper metadata from arXiv: {:.2}s",
                    time.elapsed().as_secs_f32()
                );
            }
        }
        Err(e) => {
            eprintln!(
                "WARNING: Failed to collect paper metadata from arXiv: {}",
                e
            );
        }
    }

    // Check if the paper already exists
    if cache.is_exist_paper(&paper.title) {
        if verbose {
            println!(
                "The paper already exists in the database: {}s",
                time.elapsed().as_secs_f32()
            );
        }
        return;
    }

    // Get original text
    match paper.get_original_text(pdf, verbose).await {
        Ok(_) => {
            if verbose {
                println!(
                    "Finished getting original text: {:.2}s",
                    time.elapsed().as_secs_f32()
                );
            }
        }
        Err(e) => {
            eprintln!("WARNING: Failed to get original text: {}", e);
            return;
        }
    }

    // Get keywords
    match paper.get_keywords() {
        Ok(_) => {
            if verbose {
                println!(
                    "Finished getting keywords: {:.2}s",
                    time.elapsed().as_secs_f32()
                );
            }
        }
        Err(e) => {
            eprintln!("WARNING: Failed to get keywords: {}", e);
            return;
        }
    }

    // Summarize the paper
    match ai.summarize(&mut paper).await {
        Ok(_) => {
            if verbose {
                println!(
                    "Finished summarizing the paper: {:.2}s",
                    time.elapsed().as_secs_f32()
                );
            }
        }
        Err(e) => {
            eprintln!("WARNING: Failed to summarize the paper: {}", e);
        }
    }

    // add authors
    match reporter.add_authors(&mut paper.authors, &mut cache).await {
        Ok(code) => match code {
            StatusCode::Success => {
                if verbose {
                    println!(
                        "Finished adding authors to database: {:.2}s",
                        time.elapsed().as_secs_f32()
                    );
                }
            }
            StatusCode::PaperAlreadyExists => {
                if verbose {
                    println!("The author already exists in the database.");
                }
            }
            StatusCode::Failure(e) => {
                eprintln!("WARNING: Failed to add authors to database: {}", e);
            }
        },
        Err(e) => {
            eprintln!("WARNING: Failed to report the paper to Notion: {}", e);
        }
    }

    // Post the paper to Notion
    match reporter.add_a_paper(&mut paper, &mut cache).await {
        Ok(code) => match code {
            StatusCode::Success => {
                if verbose {
                    println!(
                        "Finished reporting the paper to Notion: {:.2}s",
                        time.elapsed().as_secs_f32(),
                    );
                }
            }
            StatusCode::PaperAlreadyExists => {
                if verbose {
                    println!(
                        "The paper already exists in the database: {:.2}s",
                        time.elapsed().as_secs_f32()
                    );
                }
            }
            StatusCode::Failure(e) => {
                eprintln!(" WARNING: Failed to report the paper to Notion: {}", e);
            }
        },
        Err(e) => {
            eprintln!("WARNING: Failed to report the paper to Notion: {}", e);
        }
    }

    if verbose {
        println!(
            "Finished - Total time: {:.2}s",
            time.elapsed().as_secs_f32()
        );
    }

    cache.save().unwrap();
}

async fn post_arxiv_papers(
    date: DateTime<Utc>,
    max_retry_count: u64,
    wait_time: u64,
    model_id: String,
    verbose: bool,
) {
    let time = std::time::Instant::now();
    let mut cache = match cache::Cache::load() {
        Ok(cache) => cache,
        Err(e) => {
            eprintln!("WARNING: Failed to load cache: {}", e);
            return;
        }
    };

    // Collect arXiv papers
    let collector = collector::Collector::new(max_retry_count, wait_time);
    let mut papers = match collector.collect_papers_from_arxiv(date).await {
        Ok(papers) => papers,
        Err(e) => {
            eprintln!("WARNING: Failed to collect arXiv papers: {}", e);
            return;
        }
    };

    if verbose {
        println!(
            "Finished collecting arXiv papers: {:.2}s",
            time.elapsed().as_secs_f32()
        );
    }

    let ai = ai::AI::new(&model_id);
    let reporter = reporter::Reporter::new();

    let bar = ProgressBar::new(papers.len() as u64);
    bar.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:10.green/blue}] {pos:>3}/{len:3}: {msg}")
            .unwrap()
            .progress_chars("=> "),
    );
    bar.set_message("Processing papers");
    for paper in papers.iter_mut() {
        let time = std::time::Instant::now();
        bar.println(format!(
            "Start processing a paper: {}",
            &paper.title.clone()
        ));
        bar.set_message(format!(
            "Start processing a paper: {:.2}s)",
            time.elapsed().as_secs_f32()
        ));
        if cache.is_exist_paper(&paper.title) {
            bar.println(format!(
                "The paper already exists in the database: {:.2}s: {}",
                time.elapsed().as_secs_f32(),
                paper.title.clone()
            ));
            bar.inc(1);
            continue;
        }
        // Collect paper metadata
        match collector.update_from_ss(paper, false).await {
            Ok(_) => {
                bar.set_message(format!(
                    "Finished getting metadata from SS: ({:.2}s)",
                    time.elapsed().as_secs_f32()
                ));
            }
            Err(e) => {
                eprintln!(
                    "WARNING: Failed to collect paper metadata from Semantic Scholar: {}",
                    e
                );
                bar.inc(1);
                cache.failed_papers.push(cache::PaperCache::from_paper(
                    &paper,
                    Some(String::from("Failed to get metadata from SS")),
                ));
                continue;
            }
        }

        // Get original text
        match paper.get_original_text(None, verbose).await {
            Ok(_) => {
                bar.set_message(format!(
                    "Finished getting original text: ({:.2}s)",
                    time.elapsed().as_secs_f32()
                ));
            }
            Err(e) => {
                eprintln!("WARNING: Failed to get original text: {}", e);
                bar.inc(1);
                cache.failed_papers.push(cache::PaperCache::from_paper(
                    &paper,
                    Some(String::from("Failed to get original text")),
                ));
                continue;
            }
        }

        if paper.original_text.len() < 4 {
            eprintln!("WARNING: The paper is too short: {}", paper.title);
            bar.inc(1);
            cache.failed_papers.push(cache::PaperCache::from_paper(
                &paper,
                Some(String::from("The paper is too short")),
            ));
            continue;
        }

        // Get keywords
        match paper.get_keywords() {
            Ok(_) => {
                bar.set_message(format!(
                    "Finished getting keywords ({:.2}s)",
                    time.elapsed().as_secs_f32()
                ));
            }
            Err(e) => {
                eprintln!("WARNING: Failed to get keywords: {}", e);
                bar.inc(1);
                cache.failed_papers.push(cache::PaperCache::from_paper(
                    &paper,
                    Some(String::from("Failed to get keywords")),
                ));
                continue;
            }
        }

        // Summarize the paper
        match ai.summarize(paper).await {
            Ok(_) => {
                bar.set_message(format!(
                    "Finished summarizing the paper: ({:.2}s)",
                    time.elapsed().as_secs_f32()
                ));
            }
            Err(e) => {
                eprintln!("WARNING: Failed to summarize the paper: {}", e);
                bar.inc(1);
                cache.failed_papers.push(cache::PaperCache::from_paper(
                    &paper,
                    Some(String::from("Failed to summarize the paper")),
                ));
                continue;
            }
        }

        // add authors
        let mut error_to_update_authors = false;
        bar.suspend(|| async {
            match reporter.add_authors(&mut paper.authors, &mut cache).await {
                Ok(code) => match code {
                    StatusCode::Success => {
                        bar.set_message(format!(
                            "Finished adding authors to database: ({:.2}s)",
                            time.elapsed().as_secs_f32()
                        ));
                    }
                    StatusCode::PaperAlreadyExists => {}
                    StatusCode::Failure(e) => {
                        eprintln!("WARNING: Failed to add authors to database: {}", e);
                    }
                },
                Err(e) => {
                    eprintln!("WARNING: Failed to report the paper to Notion: {}", e);
                    bar.inc(1);
                    cache.failed_papers.push(cache::PaperCache::from_paper(
                        &paper,
                        Some(String::from("Failed to add authors")),
                    ));
                    error_to_update_authors = true;
                }
            }
        })
        .await;
        if error_to_update_authors {
            continue;
        }

        // Post the paper to Notion
        match reporter.add_a_paper(paper, &mut cache).await {
            Ok(status) => match status {
                StatusCode::Success => {
                    bar.set_message(format!(
                        "Finished reporting the paper to Notion: ({:.2}s)",
                        time.elapsed().as_secs_f32()
                    ));
                }
                StatusCode::PaperAlreadyExists => {
                    bar.set_message(format!(
                        "The paper already exists in the database: ({:.2}s)",
                        time.elapsed().as_secs_f32()
                    ));
                }
                StatusCode::Failure(e) => {
                    eprintln!("WARNING: Failed to report the paper to Notion: {}", e);
                    bar.inc(1);
                    cache.failed_papers.push(cache::PaperCache::from_paper(
                        &paper,
                        Some(String::from("Failed to report the paper")),
                    ));
                    continue;
                }
            },
            Err(e) => {
                eprintln!("WARNING: Failed to report the paper to Notion: {}", e);
                bar.inc(1);
                cache.failed_papers.push(cache::PaperCache::from_paper(
                    &paper,
                    Some(String::from("Failed to report the paper")),
                ));
                continue;
            }
        }

        if verbose {
            println!(
                "Finished - Total time: {:.2}s: {}",
                time.elapsed().as_secs_f32(),
                paper.title
            );
        }
        bar.inc(1);
    }
    bar.finish();
    cache.save().unwrap();
}

#[cfg(test)]
mod tests;
