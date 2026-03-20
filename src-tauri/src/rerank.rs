//! Reranking — combines semantic similarity with keyword overlap and source diversity.

use std::collections::{HashMap, HashSet};

/// A search result with combined scoring.
#[derive(Clone)]
pub struct RankedResult {
    pub content: String,
    pub chunk_index: usize,
    pub document_id: String,
    pub document_title: String,
    pub semantic_distance: f64,
    pub final_score: f64,
}

/// Stop words to ignore during keyword extraction.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "do", "does", "did", "will", "would", "could",
    "should", "may", "might", "shall", "can", "need", "dare", "ought",
    "used", "to", "of", "in", "for", "on", "with", "at", "by", "from",
    "as", "into", "through", "during", "before", "after", "above", "below",
    "between", "out", "off", "over", "under", "again", "further", "then",
    "once", "here", "there", "when", "where", "why", "how", "all", "both",
    "each", "few", "more", "most", "other", "some", "such", "no", "nor",
    "not", "only", "own", "same", "so", "than", "too", "very", "just",
    "don", "now", "and", "but", "or", "if", "while", "that", "this",
    "what", "which", "who", "whom", "these", "those", "it", "its",
];

/// Extract meaningful keywords from a query string.
fn extract_keywords(text: &str) -> HashSet<String> {
    let stops: HashSet<&str> = STOP_WORDS.iter().copied().collect();
    text.split_whitespace()
        .map(|w| w.to_lowercase().trim_matches(|c: char| !c.is_alphanumeric()).to_string())
        .filter(|w| w.len() > 2 && !stops.contains(w.as_str()))
        .collect()
}

/// Compute keyword overlap score between query keywords and a text.
/// Returns a value between 0.0 and 1.0 with diminishing returns.
fn keyword_score(query_keywords: &HashSet<String>, text: &str) -> f64 {
    if query_keywords.is_empty() {
        return 0.0;
    }

    let text_lower = text.to_lowercase();
    let mut matches = 0u32;
    for kw in query_keywords {
        if text_lower.contains(kw.as_str()) {
            matches += 1;
        }
    }

    if matches == 0 {
        return 0.0;
    }

    // Diminishing returns: first match worth most, each subsequent worth less
    let ratio = matches as f64 / query_keywords.len() as f64;
    // Apply sqrt for diminishing returns curve
    ratio.sqrt()
}

/// Rerank library search results combining semantic similarity, keyword overlap,
/// and source diversity.
pub fn rerank_results(
    candidates: Vec<crate::db::LibraryChunkResult>,
    query: &str,
    budget: usize,
) -> Vec<RankedResult> {
    if candidates.is_empty() {
        return Vec::new();
    }

    let query_keywords = extract_keywords(query);

    // Score each candidate
    let mut scored: Vec<RankedResult> = candidates.iter().map(|c| {
        // Semantic score: convert distance to similarity (lower distance = higher similarity)
        // sqlite-vec returns L2 distance; normalize to 0-1 range
        let semantic_sim = 1.0 / (1.0 + c.distance);

        // Keyword overlap bonus (0-1, weighted at 20% of total)
        let kw_score = keyword_score(&query_keywords, &c.content);

        // Combined score (semantic 80%, keyword 20%)
        let combined = semantic_sim * 0.8 + kw_score * 0.2;

        RankedResult {
            content: c.content.clone(),
            chunk_index: c.chunk_index,
            document_id: c.document_id.clone(),
            document_title: c.document_title.clone(),
            semantic_distance: c.distance,
            final_score: combined,
        }
    }).collect();

    // Sort by combined score descending
    scored.sort_by(|a, b| b.final_score.partial_cmp(&a.final_score).unwrap_or(std::cmp::Ordering::Equal));

    // Source diversity: limit chunks per document, interleave sources
    let mut result = Vec::with_capacity(budget);
    let mut doc_counts: HashMap<String, usize> = HashMap::new();
    let max_per_doc = (budget / 2).max(2); // At most half the budget from one source

    for item in &scored {
        if result.len() >= budget {
            break;
        }
        let count = doc_counts.entry(item.document_id.clone()).or_insert(0);
        if *count >= max_per_doc {
            continue; // Skip — this document has enough representation
        }
        *count += 1;
        result.push(item.clone());
    }

    // If we didn't fill the budget (due to diversity limits), add remaining by score
    if result.len() < budget {
        let selected: HashSet<(String, usize)> = result.iter()
            .map(|r| (r.document_id.clone(), r.chunk_index))
            .collect();
        for item in &scored {
            if result.len() >= budget {
                break;
            }
            if !selected.contains(&(item.document_id.clone(), item.chunk_index)) {
                result.push(item.clone());
            }
        }
    }

    result
}

/// Rerank single-document results with keyword boosting (no diversity needed).
pub fn rerank_document_results(
    candidates: Vec<crate::db::ChunkSearchResult>,
    query: &str,
) -> Vec<crate::db::ChunkSearchResult> {
    if candidates.is_empty() {
        return candidates;
    }

    let query_keywords = extract_keywords(query);

    let mut scored: Vec<(f64, crate::db::ChunkSearchResult)> = candidates.into_iter().map(|c| {
        let semantic_sim = 1.0 / (1.0 + c.distance);
        let kw_score = keyword_score(&query_keywords, &c.content);
        let combined = semantic_sim * 0.8 + kw_score * 0.2;
        (combined, c)
    }).collect();

    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.into_iter().map(|(_, c)| c).collect()
}
