use chrono::{DateTime, Utc};

pub fn levenshtein_dist(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    s1.chars().enumerate().for_each(|(i, c1)| {
        s2.chars().enumerate().for_each(|(j, c2)| {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = std::cmp::min(
                matrix[i][j + 1] + 1,
                std::cmp::min(matrix[i + 1][j] + 1, matrix[i][j] + cost),
            );
        });
    });

    return matrix[len1][len2];
}

pub fn levenshtein_dist_normalized(s1: &str, s2: &str) -> f64 {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();
    let dist = levenshtein_dist(s1, s2) as f64;
    let max_len = std::cmp::max(len1, len2) as f64;
    return dist / max_len;
}

pub fn levenshtein_similarity(s1: &str, s2: &str) -> f64 {
    return 1.0 / (1.0 + levenshtein_dist_normalized(s1, s2));
}

pub fn s(str: &str) -> String {
    str.to_string()
}

pub fn default_datetime() -> DateTime<Utc> {
    DateTime::parse_from_str("1970-01-01 00:00:00+0000", "%Y-%m-%d %H:%M:%S%z")
        .unwrap()
        .with_timezone(&Utc)
}

/// Convert a "%Y-%m-%d" style date string to a DateTime<Utc> object.
/// If the conversion fails, return the epoch time: "1970-01-01 00:00:00+0000".
pub fn datetime_from_str(date_str: &str) -> DateTime<Utc> {
    let mut date_str = date_str.to_string();
    date_str.push_str(" 00:00:00+0000");
    match DateTime::parse_from_str(&date_str, "%Y-%m-%d %H:%M:%S%z") {
        Ok(date) => date.with_timezone(&Utc),
        Err(e) => {
            eprintln!(
                "WARNING: Failed to parse date string: {} e: {}",
                date_str, e
            );
            default_datetime()
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn test_levenshtein_dist() {
        let s1 = "kitten";
        let s2 = "sitting";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 3);

        let s1 = "saturday";
        let s2 = "sunday";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 3);

        let s1 = "flaw";
        let s2 = "lawn";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 2);

        let s1 = "flaw";
        let s2 = "flawn";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 1);

        let s1 = "flaw";
        let s2 = "flaw";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 0);

        let s1 = "attention is all you need";
        let s2 = "attentoin is all you need";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 2);

        let s1 = "attention is all you need";
        let s2 = "attention is not all you need";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 4);

        let s1 = "attention is all you need";
        let s2 = "transformer is all you need";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 9);

        let s1 = "attention is all you need";
        let s2 = "true or marige? towards end-to-end factuality evaluation with llm-oasis";
        println!(
            "'{}' vs '{}' -> DIST: {} NORMALIZED_DIST: {:.2} SIMILARITY: {:.2}",
            s1,
            s2,
            levenshtein_dist(s1, s2),
            levenshtein_dist_normalized(s1, s2),
            levenshtein_similarity(s1, s2)
        );
        assert_eq!(levenshtein_dist(s1, s2), 58);
    }

    #[test]
    fn test_levenshtein_simu() {
        let s1 = "attention is all you need";
        let s2 = "attention is all you need";
        let score = levenshtein_similarity(s1, s2);
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Attention Is All You Need In Speech Separation";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Channel Attention Is All You Need for Video Frame Interpolation";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Attention is all you need: utilizing attention in AI-enabled drug discovery";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 =
        "Attention is all you need: An interpretable transformer-based asset allocation approach";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 =
        "Cross-Attention is All You Need: Adapting Pretrained Transformers for Machine Translation";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Is Space-Time Attention All You Need for Video Understanding?";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Attention Is All You Need For Blind Room Volume Estimation";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Graph Structure from Point Clouds: Geometric Attention is All You Need";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);

        let s1 = "attention is all you need";
        let s2 = "Master GAN: Multiple Attention is all you Need: A Multiple Attention Guided Super Resolution Network for Dems";
        let score = levenshtein_similarity(s1, s2.to_lowercase().as_str());
        println!("|{}|{:.3}|", s2, score);
    }

    #[test]
    fn test_datetime_from_str() {
        let date_str = "2024-12-29";
        let date = datetime_from_str(date_str);

        assert_eq!(date.year(), 2024);
        assert_eq!(date.month(), 12);
        assert_eq!(date.day(), 29);
    }
}
