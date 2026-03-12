use base64::Engine;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";
const MODEL: &str = "claude-sonnet-4-20250514";
const DEFAULT_MAX_TOKENS: u32 = 64_000;
const MIN_USEFUL_OUTPUT_TOKENS: u64 = 4_000;

pub struct ClaudeClient {
    client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub title: String,
    pub author: String,
    pub description: Option<String>,
    pub language: Option<String>,
    pub isbn: Option<String>,
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub page_count: Option<i32>,
    pub tags: Vec<String>,
    pub markdown_content: String,
}

#[derive(Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: Vec<ContentBlock>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { source: ImageSource },
    #[serde(rename = "document")]
    Document { source: DocumentSource },
}

#[derive(Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Serialize)]
struct DocumentSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Deserialize)]
struct ApiResponse {
    content: Vec<ResponseContent>,
}

#[derive(Deserialize)]
struct ResponseContent {
    text: Option<String>,
}

const EXTRACTION_PROMPT: &str = r#"You are a document metadata extractor and content converter. Analyze this document and return a JSON object with these fields:

{
  "title": "The document's title",
  "author": "The author(s)",
  "description": "A 1-3 sentence summary",
  "language": "ISO 639-1 language code (e.g. 'en')",
  "isbn": "ISBN if found, null otherwise",
  "publisher": "Publisher if found, null otherwise",
  "published_date": "Publication date if found, null otherwise",
  "page_count": null,
  "tags": ["relevant", "topic", "tags"],
  "markdown_content": "The full document content converted to clean, well-structured Markdown"
}

For the markdown_content:
- Preserve all headings, lists, tables, emphasis, and document structure
- Clean up OCR artifacts, fix broken words, remove page headers/footers/numbers
- Preserve code blocks, blockquotes, and special formatting
- For images/figures, note them as [Figure: description] placeholders
- Make the markdown beautiful and readable

Return ONLY the JSON object, no other text."#;

/// Parse a context overflow error to extract input token count and context limit.
/// Error format: "... context limit: 152815 + 64000 > 200000, ..."
fn parse_context_overflow(body: &str) -> Option<(u64, u64)> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;
    let message = parsed["error"]["message"].as_str()?;
    if !message.contains("exceed context limit") {
        return None;
    }
    let after_colon = message.rsplit(':').next()?;
    let parts: Vec<&str> = after_colon.split('>').collect();
    if parts.len() != 2 {
        return None;
    }
    let input_tokens: u64 = parts[0].split('+').next()?.trim().parse().ok()?;
    let limit: u64 = parts[1].split(',').next()?.trim().parse().ok()?;
    Some((input_tokens, limit))
}

impl ClaudeClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Validate an API key by making a lightweight request
    pub async fn validate_key(&self, api_key: &str) -> Result<bool, String> {
        let request = ApiRequest {
            model: MODEL.to_string(),
            max_tokens: 16,
            messages: vec![Message {
                role: "user".to_string(),
                content: vec![ContentBlock::Text {
                    text: "Say \"ok\"".to_string(),
                }],
            }],
        };

        let response = self
            .client
            .post(CLAUDE_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        Ok(response.status().is_success())
    }

    /// Build content blocks for the extraction request
    fn build_content_blocks(file_data: &[u8], b64: &str, format: &str) -> Vec<ContentBlock> {
        match format {
            "pdf" => vec![
                ContentBlock::Document {
                    source: DocumentSource {
                        source_type: "base64".to_string(),
                        media_type: "application/pdf".to_string(),
                        data: b64.to_string(),
                    },
                },
                ContentBlock::Text {
                    text: EXTRACTION_PROMPT.to_string(),
                },
            ],
            "png" | "jpg" | "jpeg" | "webp" | "gif" => {
                let media_type = match format {
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "webp" => "image/webp",
                    "gif" => "image/gif",
                    _ => "image/png",
                };
                vec![
                    ContentBlock::Image {
                        source: ImageSource {
                            source_type: "base64".to_string(),
                            media_type: media_type.to_string(),
                            data: b64.to_string(),
                        },
                    },
                    ContentBlock::Text {
                        text: EXTRACTION_PROMPT.to_string(),
                    },
                ]
            }
            _ => {
                let text_content = String::from_utf8(file_data.to_vec())
                    .unwrap_or_else(|_| "[Binary content - could not decode as text]".to_string());

                let truncated = if text_content.len() > 400_000 {
                    format!(
                        "{}...\n\n[Content truncated at 400k characters]",
                        &text_content[..400_000]
                    )
                } else {
                    text_content
                };

                vec![ContentBlock::Text {
                    text: format!(
                        "Here is a document in {} format:\n\n---\n{}\n---\n\n{}",
                        format, truncated, EXTRACTION_PROMPT
                    ),
                }]
            }
        }
    }

    pub async fn extract_document(
        &self,
        file_path: &Path,
        format: &str,
        api_key: &str,
        model: &str,
    ) -> Result<ExtractionResult, String> {
        if api_key.is_empty() {
            return Err(
                "No API key configured. Go to Settings to add your Anthropic API key.".to_string(),
            );
        }

        let file_data =
            std::fs::read(file_path).map_err(|e| format!("Failed to read file: {}", e))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&file_data);

        let mut max_tokens = DEFAULT_MAX_TOKENS;
        let mut attempts = 0;

        loop {
            attempts += 1;
            if attempts > 2 {
                return Err(
                    "Document too large — could not fit within Claude's context window after retry."
                        .to_string(),
                );
            }

            let content_blocks = Self::build_content_blocks(&file_data, &b64, format);

            let request = ApiRequest {
                model: model.to_string(),
                max_tokens,
                messages: vec![Message {
                    role: "user".to_string(),
                    content: content_blocks,
                }],
            };

            let response = self
                .client
                .post(CLAUDE_API_URL)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&request)
                .send()
                .await
                .map_err(|e| format!("API request failed: {}", e))?;

            let status = response.status();

            // 413: request body too large for the API
            if status == 413 {
                return Err(
                    "Document is too large for the Claude API (max ~30MB). Try a smaller file."
                        .to_string(),
                );
            }

            if status == 400 {
                let body = response.text().await.unwrap_or_default();

                // PDF page limit
                if body.contains("PDF pages") {
                    return Err(
                        "PDF exceeds the 100-page limit. Try splitting into smaller files."
                            .to_string(),
                    );
                }

                // Context overflow — retry with reduced max_tokens
                if let Some((input_tokens, context_limit)) = parse_context_overflow(&body) {
                    let available = context_limit
                        .saturating_sub(input_tokens)
                        .saturating_sub(200);

                    if available >= MIN_USEFUL_OUTPUT_TOKENS && max_tokens > available as u32 {
                        eprintln!(
                            "[claude] context overflow: input={}t, retrying with max_tokens={}",
                            input_tokens, available
                        );
                        max_tokens = available as u32;
                        continue;
                    }

                    return Err(format!(
                        "Document uses {}k tokens — too large for Claude's {}k context window. Try a smaller file.",
                        input_tokens / 1000,
                        context_limit / 1000,
                    ));
                }

                return Err(format!("Claude API error ({}): {}", status, body));
            }

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(format!("Claude API error ({}): {}", status, body));
            }

            // Parse successful response
            let api_response: ApiResponse = response
                .json()
                .await
                .map_err(|e| format!("Failed to parse API response: {}", e))?;

            let text = api_response
                .content
                .into_iter()
                .find_map(|c| c.text)
                .ok_or_else(|| "No text in API response".to_string())?;

            let json_str = text
                .trim()
                .strip_prefix("```json")
                .unwrap_or(text.trim())
                .strip_prefix("```")
                .unwrap_or(text.trim())
                .strip_suffix("```")
                .unwrap_or(text.trim())
                .trim();

            let result: ExtractionResult = serde_json::from_str(json_str).map_err(|e| {
                format!(
                    "Failed to parse extraction result: {}. Raw response: {}",
                    e,
                    &text[..text.len().min(500)]
                )
            })?;

            return Ok(result);
        }
    }
}
