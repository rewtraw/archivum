use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use std::path::Path;

pub struct EmbeddingEngine {
    model: TextEmbedding,
}

#[derive(Debug, Clone)]
pub struct Chunk {
    pub index: usize,
    pub offset: usize,
    pub text: String,
}

impl EmbeddingEngine {
    pub fn new(cache_dir: &Path) -> Result<Self, String> {
        let options = InitOptions::new(EmbeddingModel::BGESmallENV15)
            .with_cache_dir(cache_dir.to_path_buf())
            .with_show_download_progress(true);

        let model = TextEmbedding::try_new(options)
            .map_err(|e| format!("Failed to load embedding model: {}", e))?;

        Ok(Self { model })
    }

    pub fn embed_chunks(&self, chunks: &[Chunk]) -> Result<Vec<Vec<f32>>, String> {
        if chunks.is_empty() {
            return Ok(Vec::new());
        }
        let texts: Vec<&str> = chunks.iter().map(|c| c.text.as_str()).collect();
        self.model
            .embed(texts, Some(64))
            .map_err(|e| format!("Embedding failed: {}", e))
    }

    pub fn embed_query(&self, query: &str) -> Result<Vec<f32>, String> {
        let results = self
            .model
            .embed(vec![query], None)
            .map_err(|e| format!("Query embedding failed: {}", e))?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| "No embedding returned".to_string())
    }

    pub fn dimensions(&self) -> usize {
        384
    }
}

/// Split markdown into overlapping chunks, preserving heading context.
pub fn chunk_markdown(text: &str) -> Vec<Chunk> {
    const TARGET_SIZE: usize = 2000; // ~500 tokens
    const OVERLAP: usize = 200;
    const MIN_SIZE: usize = 50;

    let paragraphs: Vec<&str> = text.split("\n\n").collect();
    let mut chunks = Vec::new();
    let mut current_text = String::new();
    let mut current_offset = 0;
    let mut chunk_start_offset = 0;
    let mut current_heading = String::new();

    for para in &paragraphs {
        let trimmed = para.trim();
        if trimmed.is_empty() {
            current_offset += para.len() + 2; // +2 for \n\n
            continue;
        }

        // Track headings for context
        if trimmed.starts_with('#') {
            let heading_line = trimmed.lines().next().unwrap_or("");
            current_heading = heading_line.to_string();
        }

        let would_be = current_text.len() + trimmed.len() + 1;

        if would_be > TARGET_SIZE && !current_text.is_empty() {
            // Flush current chunk
            let mut chunk_text = current_text.trim().to_string();

            // Prepend heading context if the chunk doesn't start with one
            if !chunk_text.starts_with('#') && !current_heading.is_empty() {
                chunk_text = format!("{}\n\n{}", current_heading, chunk_text);
            }

            if chunk_text.len() >= MIN_SIZE {
                chunks.push(Chunk {
                    index: chunks.len(),
                    offset: chunk_start_offset,
                    text: chunk_text,
                });
            }

            // Start new chunk with overlap from the end of current
            let overlap_start = if current_text.len() > OVERLAP {
                // Find a good break point near the overlap boundary
                let mut boundary = current_text.len() - OVERLAP;
                // Snap to a char boundary (walk backward)
                while boundary > 0 && !current_text.is_char_boundary(boundary) {
                    boundary -= 1;
                }
                current_text[boundary..]
                    .find('\n')
                    .map(|i| boundary + i + 1)
                    .unwrap_or(boundary)
            } else {
                0
            };
            current_text = current_text[overlap_start..].to_string();
            chunk_start_offset = current_offset.saturating_sub(current_text.len());
        }

        if current_text.is_empty() {
            chunk_start_offset = current_offset;
        }

        if !current_text.is_empty() {
            current_text.push_str("\n\n");
        }
        current_text.push_str(trimmed);

        current_offset += para.len() + 2;
    }

    // Flush final chunk
    if !current_text.trim().is_empty() {
        let mut chunk_text = current_text.trim().to_string();
        if !chunk_text.starts_with('#') && !current_heading.is_empty() {
            chunk_text = format!("{}\n\n{}", current_heading, chunk_text);
        }
        if chunk_text.len() >= MIN_SIZE {
            chunks.push(Chunk {
                index: chunks.len(),
                offset: chunk_start_offset,
                text: chunk_text,
            });
        }
    }

    chunks
}
