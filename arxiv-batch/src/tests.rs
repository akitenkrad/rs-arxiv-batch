use super::ai::*;
use super::collector::*;
use super::common::*;
use std::sync::Once;

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        dotenvy::dotenv().ok();
    });
}

#[tokio::test]
async fn test_update_from_arxiv() {
    initialize();
    let mut paper = Paper::default();
    paper.title = "Attention is all you need".to_string();

    let collector = Collector::default();
    let result = collector.update_from_arxiv(&mut paper, true).await;
    match result {
        Ok(_) => {
            println!("Paper: {:?}", paper);
            assert_eq!(paper.arxiv_id, "http://arxiv.org/abs/1706.03762v7");
            assert_eq!(paper.title.to_lowercase(), "attention is all you need");
        }
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_update_from_ss() {
    initialize();
    let mut paper = Paper::default();
    paper.title = "Attention Is All You Need".to_string();

    let collector = Collector::new(5, 15);
    let result = collector.update_from_ss(&mut paper, true).await;
    match result {
        Ok(_) => {
            println!("Paper: {:?}", paper);
            assert!(paper.citation_count > 0);
            assert_eq!(paper.title.to_lowercase(), "attention is all you need");
        }
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_paper2xml() {
    initialize();
    let mut paper = Paper::default();
    paper.title = "Attention Is All You Need".to_string();

    let collector = Collector::new(5, 15);
    let _ = collector.update_from_arxiv(&mut paper, true).await;
    let _ = collector.update_from_ss(&mut paper, false).await;

    match paper.get_original_text(None, true).await {
        Ok(_) => {}
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }

    let text_xml = paper.original_text2xml();
    println!("{}", text_xml);
    assert!(text_xml.len() > 0);

    let reference_xml = paper.references2xml();
    println!("{}", reference_xml);
    assert!(reference_xml.len() > 0);
}

#[tokio::test]
async fn test_get_keywords() {
    initialize();
    let mut paper = Paper::default();
    paper.title = "Attention Is All You Need".to_string();

    let collector = Collector::new(5, 15);
    let _ = collector.update_from_arxiv(&mut paper, true).await;
    let _ = collector.update_from_ss(&mut paper, false).await;

    match paper.get_original_text(None, true).await {
        Ok(_) => {}
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }

    paper.get_keywords().unwrap();

    println!("{:?}", paper.keywords);
}

#[tokio::test]
async fn test_summarize() {
    initialize();
    let mut paper = Paper::default();
    paper.title = "Attention Is All You Need".to_string();

    let collector = Collector::new(5, 15);
    let _ = collector.update_from_arxiv(&mut paper, true).await;
    let _ = collector.update_from_ss(&mut paper, false).await;

    match paper.get_original_text(None, true).await {
        Ok(_) => {}
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }

    let ai = AI::new("gpt-4o-mini");
    let result = ai.summarize(&mut paper).await;
    match result {
        Ok(_) => {
            assert!(paper.summary.overview.len() > 0);
            assert!(paper.summary.research_question.len() > 0);
            println!("{:?}", paper.summary);
        }
        Err(e) => {
            assert!(false, "Error: {:?}", e);
        }
    }
}
