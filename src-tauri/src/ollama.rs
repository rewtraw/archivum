use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Extract JSON from a response that may contain thinking tags or other text.
/// Tries: raw string → strip code fences → find first `{...}` block.
fn extract_json(text: &str) -> &str {
    let stripped = crate::claude::strip_code_fences(text);
    if !stripped.trim().is_empty() && stripped.trim().starts_with('{') {
        return stripped;
    }

    // Try to find JSON object within the text (skip <think> blocks, etc.)
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            if end > start {
                return &text[start..=end];
            }
        }
    }

    stripped
}

pub struct OllamaClient {
    client: Client,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub available: bool,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaModelInfo {
    pub name: String,
    pub size: i64,
    pub modified_at: String,
    pub parameter_size: Option<String>,
    pub family: Option<String>,
}

pub struct RecommendedModel {
    pub name: &'static str,
    pub label: &'static str,
    pub description: &'static str,
}

pub const RECOMMENDED_MODELS: &[RecommendedModel] = &[
    RecommendedModel {
        name: "qwen3:8b",
        label: "Qwen 3 8B",
        description: "Strong general-purpose, great quality",
    },
    RecommendedModel {
        name: "llama3.2:8b",
        label: "Llama 3.2 8B",
        description: "Well-rounded, fast",
    },
    RecommendedModel {
        name: "gemma3:12b",
        label: "Gemma 3 12B",
        description: "High quality, efficient",
    },
    RecommendedModel {
        name: "phi4:14b",
        label: "Phi 4 14B",
        description: "Strong reasoning, compact",
    },
    RecommendedModel {
        name: "mistral:7b",
        label: "Mistral 7B",
        description: "Fast, lightweight",
    },
    RecommendedModel {
        name: "deepseek-r1:14b",
        label: "DeepSeek R1 14B",
        description: "Excellent reasoning",
    },
];

impl OllamaClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Check if Ollama is running
    pub async fn check_status(&self, base_url: &str) -> OllamaStatus {
        let url = format!("{}/api/version", base_url);
        match self.client.get(&url).timeout(std::time::Duration::from_secs(3)).send().await {
            Ok(resp) if resp.status().is_success() => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                OllamaStatus {
                    available: true,
                    version: body["version"].as_str().map(|s| s.to_string()),
                }
            }
            _ => OllamaStatus {
                available: false,
                version: None,
            },
        }
    }

    /// List installed models
    pub async fn list_models(&self, base_url: &str) -> Result<Vec<OllamaModelInfo>, String> {
        let url = format!("{}/api/tags", base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Ollama: {}. Is it running?", e))?;

        if !resp.status().is_success() {
            return Err(format!("Ollama API error: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let models = body["models"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|m| {
                OllamaModelInfo {
                    name: m["name"].as_str().unwrap_or("").to_string(),
                    size: m["size"].as_i64().unwrap_or(0),
                    modified_at: m["modified_at"].as_str().unwrap_or("").to_string(),
                    parameter_size: m["details"]["parameter_size"].as_str().map(|s| s.to_string()),
                    family: m["details"]["family"].as_str().map(|s| s.to_string()),
                }
            })
            .collect();

        Ok(models)
    }

    /// Pull a model with progress callback
    pub async fn pull_model(
        &self,
        base_url: &str,
        name: &str,
        progress_callback: impl Fn(f64, &str),
    ) -> Result<(), String> {
        let url = format!("{}/api/pull", base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({"name": name}))
            .send()
            .await
            .map_err(|e| format!("Pull request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama pull failed: {}", body));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                    let status = event["status"].as_str().unwrap_or("");
                    let completed = event["completed"].as_f64().unwrap_or(0.0);
                    let total = event["total"].as_f64().unwrap_or(0.0);

                    if total > 0.0 {
                        progress_callback(completed / total, status);
                    } else {
                        progress_callback(0.0, status);
                    }
                }
            }
        }

        Ok(())
    }

    /// Delete a model
    pub async fn delete_model(&self, base_url: &str, name: &str) -> Result<(), String> {
        let url = format!("{}/api/delete", base_url);
        let resp = self
            .client
            .delete(&url)
            .json(&serde_json::json!({"name": name}))
            .send()
            .await
            .map_err(|e| format!("Delete request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Delete failed: {}", body));
        }
        Ok(())
    }

    /// Enrich metadata using an Ollama model (mirrors ClaudeClient::enrich_metadata)
    pub async fn enrich_metadata(
        &self,
        text_excerpt: &str,
        base_url: &str,
        model: &str,
    ) -> Result<crate::claude::MetadataResult, String> {
        let excerpt = if text_excerpt.len() > 4000 {
            &text_excerpt[..4000]
        } else {
            text_excerpt
        };

        let user_msg = format!(
            "Here is an excerpt from a document:\n\n---\n{}\n---\n\n{}\n\nRespond with ONLY the JSON object.",
            excerpt, crate::claude::METADATA_PROMPT
        );

        // Use /api/chat instead of /api/generate — handles thinking models (Qwen 3, etc.) better
        let url = format!("{}/api/chat", base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "user", "content": user_msg}
                ],
                "stream": false,
                "format": "json",
                "options": {"num_predict": 2048, "num_ctx": 8192},
            }))
            .send()
            .await
            .map_err(|e| format!("Ollama request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama error: {}", body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let text = body["message"]["content"]
            .as_str()
            .ok_or_else(|| format!("No message content from Ollama: {}", body))?;

        if text.trim().is_empty() {
            return Err("Ollama returned empty response for metadata".to_string());
        }

        let json_str = extract_json(text);

        if json_str.trim().is_empty() {
            return Err(format!(
                "Ollama response contained no JSON (raw: {})",
                &text[..text.len().min(200)]
            ));
        }

        serde_json::from_str::<crate::claude::MetadataResult>(json_str)
            .map_err(|e| format!("Failed to parse metadata from Ollama: {} (raw: {})", e, &json_str[..json_str.len().min(200)]))
    }

    /// Generate a document summary (mirrors ClaudeClient::generate_summary)
    pub async fn generate_summary(
        &self,
        text: &str,
        length: &str,
        base_url: &str,
        model: &str,
    ) -> Result<String, String> {
        let (max_input, max_tokens, instruction) = match length {
            "short" => (8_000, 256, "Write a 2-3 sentence summary of this document."),
            "medium" => (
                20_000,
                1024,
                "Write a 1-2 paragraph summary covering the main themes and arguments.",
            ),
            _ => (
                40_000,
                4096,
                "Write a comprehensive multi-paragraph summary covering all key points, arguments, and conclusions.",
            ),
        };

        let excerpt = if text.len() > max_input {
            &text[..max_input]
        } else {
            text
        };

        let user_msg = format!(
            "Here is a document:\n\n---\n{}\n---\n\n{}\n\nReturn ONLY the summary text, no preamble.",
            excerpt, instruction
        );

        let url = format!("{}/api/chat", base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "user", "content": user_msg}
                ],
                "stream": false,
                "options": {"num_predict": max_tokens},
            }))
            .send()
            .await
            .map_err(|e| format!("Ollama summary request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama error: {}", body));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let content = body["message"]["content"]
            .as_str()
            .ok_or_else(|| "No message content from Ollama".to_string())?;

        // Strip thinking tags if present
        let result = if let Some(end) = content.find("</think>") {
            content[end + 8..].trim()
        } else {
            content.trim()
        };

        Ok(result.to_string())
    }

    /// Stream a chat response with document context (mirrors ClaudeClient::chat_with_context)
    pub async fn chat_with_context(
        &self,
        question: &str,
        context_chunks: &[crate::db::ChunkSearchResult],
        document_title: &str,
        base_url: &str,
        model: &str,
        channel: tauri::ipc::Channel<crate::commands::ChatEvent>,
    ) -> Result<String, String> {
        let mut context = String::new();
        for (i, chunk) in context_chunks.iter().enumerate() {
            context.push_str(&format!("[{}] {}\n\n", i + 1, chunk.content));
        }

        let system_prompt = format!(
            "You are a helpful assistant answering questions about the document \"{}\". \
             Use ONLY the following excerpts to answer. If the excerpts don't contain enough \
             information, say so clearly. Quote relevant passages when helpful. Be concise but thorough.",
            document_title
        );

        let user_message = format!(
            "Here are relevant excerpts from the document:\n\n{}\nQuestion: {}",
            context, question
        );

        self.stream_chat(base_url, model, &system_prompt, &user_message, channel)
            .await
    }

    /// Stream a library-wide chat response (mirrors ClaudeClient::chat_with_library_context)
    pub async fn chat_with_library_context(
        &self,
        question: &str,
        context_chunks: &[crate::db::LibraryChunkResult],
        base_url: &str,
        model: &str,
        channel: tauri::ipc::Channel<crate::commands::ChatEvent>,
    ) -> Result<String, String> {
        let mut context = String::new();
        for (i, chunk) in context_chunks.iter().enumerate() {
            context.push_str(&format!(
                "[{} — \"{}\"] {}\n\n",
                i + 1,
                chunk.document_title,
                chunk.content
            ));
        }

        let system_prompt = "You are a research assistant with access to the user's document library. \
             You can see excerpts from MULTIPLE documents, each labeled with their source number and title. \
             Your job is to synthesize information ACROSS documents — find patterns, draw comparisons, \
             note agreements and contradictions between sources, and build a holistic answer. \
             Do NOT just answer from a single source when multiple are available. \
             Always cite sources by their title (e.g. \"as discussed in *Title of Book*\"), NOT by number. \
             If the excerpts don't contain enough information, say so clearly.";

        let user_message = format!(
            "Here are relevant excerpts from across your library:\n\n{}\nQuestion: {}",
            context, question
        );

        self.stream_chat(base_url, model, system_prompt, &user_message, channel)
            .await
    }

    /// Internal: stream a chat request via Ollama's NDJSON format
    async fn stream_chat(
        &self,
        base_url: &str,
        model: &str,
        system_prompt: &str,
        user_message: &str,
        channel: tauri::ipc::Channel<crate::commands::ChatEvent>,
    ) -> Result<String, String> {
        let url = format!("{}/api/chat", base_url);
        let resp = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "model": model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_message}
                ],
                "stream": true
            }))
            .send()
            .await
            .map_err(|e| format!("Ollama chat request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Ollama chat error: {}", body));
        }

        use futures_util::StreamExt;
        let mut stream = resp.bytes_stream();
        let mut full_text = String::new();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let bytes = chunk.map_err(|e| format!("Stream error: {}", e))?;
            buffer.push_str(&String::from_utf8_lossy(&bytes));

            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() {
                    continue;
                }

                if let Ok(event) = serde_json::from_str::<serde_json::Value>(&line) {
                    if let Some(content) = event["message"]["content"].as_str() {
                        if !content.is_empty() {
                            full_text.push_str(content);
                            let _ = channel.send(crate::commands::ChatEvent::Token {
                                text: content.to_string(),
                            });
                        }
                    }
                }
            }
        }

        Ok(full_text)
    }
}

impl Default for OllamaClient {
    fn default() -> Self {
        Self::new()
    }
}
