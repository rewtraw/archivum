//! Agentic RAG — multi-step tool-use loop for document and library Q&A.
//!
//! The agent receives a question, optional summary context, and a set of tools.
//! It iterates: call LLM → parse tool calls → execute tools → feed results back,
//! until the LLM produces a final text answer (capped at MAX_ITERATIONS).

use crate::commands::ChatEvent;
use crate::db::Database;
use crate::embeddings::EmbeddingEngine;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;

const MAX_ITERATIONS: usize = 6;

/// Truncate a string at a char boundary, appending "..." if truncated.
fn truncate_str(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}...", &s[..end])
}

/// Scope determines which tools and context the agent has access to.
#[derive(Clone)]
pub enum AgentScope {
    Document {
        document_id: String,
        title: String,
    },
    Library,
}

/// A tool call parsed from the LLM response.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
}

/// Result of executing a tool.
struct ToolResult {
    tool_use_id: String,
    content: String,
}

/// Tool definitions for Claude's tool-use API.
pub fn tool_definitions(scope: &AgentScope) -> Vec<serde_json::Value> {
    let mut tools = vec![
        serde_json::json!({
            "name": "search_content",
            "description": "Semantic search for relevant passages. Returns the most relevant text chunks matching the query.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query — be specific about what information you need"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results (default 6)",
                        "default": 6
                    }
                },
                "required": ["query"]
            }
        }),
        serde_json::json!({
            "name": "get_document_summary",
            "description": "Get a summary of a specific document. Use this to understand what a document is about before searching its contents.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "document_id": {
                        "type": "string",
                        "description": "The document ID"
                    }
                },
                "required": ["document_id"]
            }
        }),
        serde_json::json!({
            "name": "get_section_summaries",
            "description": "Get section-level summaries for a document. Useful for understanding the structure and finding which section to search.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "document_id": {
                        "type": "string",
                        "description": "The document ID"
                    }
                },
                "required": ["document_id"]
            }
        }),
    ];

    if matches!(scope, AgentScope::Library) {
        tools.push(serde_json::json!({
            "name": "keyword_search",
            "description": "Full-text keyword search across the library. Good for finding specific terms, names, or phrases.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Keywords to search for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default 5)",
                        "default": 5
                    }
                },
                "required": ["query"]
            }
        }));
        tools.push(serde_json::json!({
            "name": "list_documents",
            "description": "List documents in the library with their titles, authors, and tags. Useful for understanding what's available.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "integer",
                        "description": "Maximum documents to list (default 20)",
                        "default": 20
                    }
                },
                "required": []
            }
        }));
        tools.push(serde_json::json!({
            "name": "get_related_documents",
            "description": "Find documents similar to a given document.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "document_id": {
                        "type": "string",
                        "description": "The document ID to find related documents for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results (default 5)",
                        "default": 5
                    }
                },
                "required": ["document_id"]
            }
        }));
    }

    tools
}

/// Build the system prompt with optional summary context.
pub fn build_system_prompt(scope: &AgentScope, summary_context: &str) -> String {
    let base = match scope {
        AgentScope::Document { title, .. } => format!(
            "You are a research assistant helping the user explore the document \"{}\". \
             You have tools to search the document's content, read summaries, and examine sections. \
             Use these tools to find relevant information before answering. \
             Always cite specific passages when possible. Be thorough but concise.",
            title
        ),
        AgentScope::Library => {
            "You are a research assistant with access to the user's document library. \
             You have tools to search across all documents, look up specific documents, \
             find related works, and read summaries. Use these tools to research the question \
             thoroughly before answering. Synthesize information across multiple sources. \
             Always cite documents by title.".to_string()
        }
    };

    if summary_context.is_empty() {
        base
    } else {
        format!("{}\n\nHere is an overview of the available content:\n{}", base, summary_context)
    }
}

/// Execute a tool call against the database and embeddings.
fn execute_tool(
    tool: &ToolCall,
    scope: &AgentScope,
    db: &Database,
    engine: &EmbeddingEngine,
) -> Result<ToolResult, String> {
    let content = match tool.name.as_str() {
        "search_content" => {
            let query = tool.input["query"].as_str().unwrap_or("");
            let limit = tool.input["limit"].as_u64().unwrap_or(6) as usize;

            let query_embedding = engine.embed_query(query)?;

            match scope {
                AgentScope::Document { document_id, .. } => {
                    let chunks = db.search_chunks(&query_embedding, document_id, limit)
                        .map_err(|e| e.to_string())?;
                    if chunks.is_empty() {
                        "No matching passages found.".to_string()
                    } else {
                        chunks.iter().enumerate()
                            .map(|(i, c)| format!("[{}] (chunk {}) {}", i + 1, c.chunk_index, c.content))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    }
                }
                AgentScope::Library => {
                    let chunks = db.search_all_chunks(&query_embedding, limit * 3)
                        .map_err(|e| e.to_string())?;
                    // Diversify across documents
                    let diversified = crate::commands::diversify_chunks(chunks, limit);
                    if diversified.is_empty() {
                        "No matching passages found.".to_string()
                    } else {
                        diversified.iter().enumerate()
                            .map(|(i, c)| format!("[{} — \"{}\"] {}", i + 1, c.document_title, c.content))
                            .collect::<Vec<_>>()
                            .join("\n\n")
                    }
                }
            }
        }
        "get_document_summary" => {
            let doc_id = tool.input["document_id"].as_str().unwrap_or("");
            let doc_id = if doc_id.is_empty() {
                if let AgentScope::Document { document_id, .. } = scope {
                    document_id.as_str()
                } else {
                    return Ok(ToolResult {
                        tool_use_id: tool.id.clone(),
                        content: "No document_id provided.".to_string(),
                    });
                }
            } else {
                doc_id
            };

            // Try medium summary first, fall back to short
            let summary = db.get_summary(doc_id, "medium")
                .map_err(|e| e.to_string())?
                .or_else(|| db.get_summary(doc_id, "short").ok().flatten());

            match summary {
                Some(s) => s,
                None => "No summary available for this document.".to_string(),
            }
        }
        "get_section_summaries" => {
            let doc_id = tool.input["document_id"].as_str().unwrap_or("");
            let doc_id = if doc_id.is_empty() {
                if let AgentScope::Document { document_id, .. } = scope {
                    document_id.as_str()
                } else {
                    return Ok(ToolResult {
                        tool_use_id: tool.id.clone(),
                        content: "No document_id provided.".to_string(),
                    });
                }
            } else {
                doc_id
            };

            let sections = db.get_section_summaries(doc_id)
                .map_err(|e| e.to_string())?;

            if sections.is_empty() {
                "No section summaries available.".to_string()
            } else {
                sections.iter()
                    .map(|s| {
                        let title_part = s.title.as_deref().unwrap_or("Untitled section");
                        let concepts = s.key_concepts.as_deref().unwrap_or("");
                        format!("## {} (chunks {}-{})\n{}\nKey concepts: {}",
                            title_part, s.start_chunk, s.end_chunk, s.summary, concepts)
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        }
        "keyword_search" => {
            let query = tool.input["query"].as_str().unwrap_or("");
            let limit = tool.input["limit"].as_u64().unwrap_or(5) as usize;

            let results = db.search(query, limit as i64)
                .map_err(|e| e.to_string())?;

            if results.is_empty() {
                "No keyword matches found.".to_string()
            } else {
                results.iter()
                    .map(|r| format!("\"{}\" by {} — {}", r.title, r.author, r.snippet))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        }
        "list_documents" => {
            let limit = tool.input["limit"].as_u64().unwrap_or(20) as usize;

            let docs = db.list_documents(0, limit as i64, None, None, None, None, None)
                .map_err(|e| e.to_string())?;

            if docs.is_empty() {
                "No documents in the library.".to_string()
            } else {
                docs.iter()
                    .map(|d| {
                        let tags = if d.tags.is_empty() { String::new() } else { format!(" [{}]", d.tags.join(", ")) };
                        format!("- {} (id: {}) by {}{}", d.title, d.id, d.author, tags)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        }
        "get_related_documents" => {
            let doc_id = tool.input["document_id"].as_str().unwrap_or("");
            let limit = tool.input["limit"].as_u64().unwrap_or(5) as usize;

            // Get first chunk's embedding as a proxy for the document
            let chunks = db.search_chunks_by_document(doc_id, 1)
                .map_err(|e| e.to_string())?;

            if chunks.is_empty() {
                "Document has no indexed content.".to_string()
            } else {
                let query_embedding = engine.embed_query(&chunks[0].content)?;
                let related = db.search_semantic_excluding(&query_embedding, doc_id, limit)
                    .map_err(|e| e.to_string())?;

                if related.is_empty() {
                    "No related documents found.".to_string()
                } else {
                    related.iter()
                        .map(|r| format!("- \"{}\" by {} (id: {})", r.title, r.author, r.id))
                        .collect::<Vec<_>>()
                        .join("\n")
                }
            }
        }
        _ => format!("Unknown tool: {}", tool.name),
    };

    Ok(ToolResult {
        tool_use_id: tool.id.clone(),
        content,
    })
}

/// Run the agentic RAG loop with Claude's tool-use API.
pub async fn run_claude_agent(
    question: &str,
    scope: AgentScope,
    summary_context: &str,
    db: &Arc<Mutex<Database>>,
    engine: &Arc<OnceCell<EmbeddingEngine>>,
    data_dir: &std::path::Path,
    api_key: &str,
    model: &str,
    channel: &tauri::ipc::Channel<ChatEvent>,
) -> Result<String, String> {
    let engine = engine
        .get_or_try_init(|| async {
            crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .map_err(|e| format!("Embedding engine failed: {}", e))?;

    let system_prompt = build_system_prompt(&scope, summary_context);
    let tools = tool_definitions(&scope);

    let mut messages = vec![
        serde_json::json!({
            "role": "user",
            "content": question
        })
    ];

    let client = reqwest::Client::new();

    for _iteration in 0..MAX_ITERATIONS {
        let request = serde_json::json!({
            "model": model,
            "max_tokens": 8192,
            "system": system_prompt,
            "tools": tools,
            "messages": messages
        });

        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Agent request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Claude API error ({}): {}", status, body));
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        let stop_reason = body["stop_reason"].as_str().unwrap_or("");
        let content = body["content"].as_array()
            .ok_or_else(|| "No content in response".to_string())?;

        // Collect text and tool_use blocks
        let mut text_parts = String::new();
        let mut tool_calls = Vec::new();
        let mut response_content = Vec::new();

        for block in content {
            let block_type = block["type"].as_str().unwrap_or("");
            match block_type {
                "text" => {
                    let text = block["text"].as_str().unwrap_or("");
                    text_parts.push_str(text);
                    response_content.push(block.clone());
                }
                "tool_use" => {
                    let tc = ToolCall {
                        id: block["id"].as_str().unwrap_or("").to_string(),
                        name: block["name"].as_str().unwrap_or("").to_string(),
                        input: block["input"].clone(),
                    };
                    tool_calls.push(tc);
                    response_content.push(block.clone());
                }
                _ => {}
            }
        }

        if stop_reason == "end_turn" || tool_calls.is_empty() {
            // Final answer — stream the text to the frontend
            for ch in text_parts.chars() {
                // Send in small batches for smooth streaming feel
                let _ = channel.send(ChatEvent::Token {
                    text: ch.to_string(),
                });
            }
            return Ok(text_parts);
        }

        // Tool use — execute tools and continue the loop
        // Add assistant message with tool_use blocks
        messages.push(serde_json::json!({
            "role": "assistant",
            "content": response_content
        }));

        // Execute each tool call
        let mut tool_results = Vec::new();
        for tc in &tool_calls {
            // Notify frontend about tool activity
            let _ = channel.send(ChatEvent::ToolCall {
                tool: tc.name.clone(),
                query: tc.input["query"].as_str()
                    .or_else(|| tc.input["document_id"].as_str())
                    .unwrap_or("")
                    .to_string(),
            });

            let result = {
                let db_lock = db.lock().unwrap();
                execute_tool(tc, &scope, &db_lock, engine)?
            };

            let _ = channel.send(ChatEvent::ToolResult {
                tool: tc.name.clone(),
                summary: truncate_str(&result.content, 100),
            });

            tool_results.push(serde_json::json!({
                "type": "tool_result",
                "tool_use_id": result.tool_use_id,
                "content": result.content
            }));
        }

        // Add tool results as user message
        messages.push(serde_json::json!({
            "role": "user",
            "content": tool_results
        }));

        // Stream any intermediate text
        if !text_parts.is_empty() {
            let _ = channel.send(ChatEvent::Token {
                text: text_parts.clone(),
            });
        }
    }

    Err("Agent exceeded maximum iterations".to_string())
}

/// Run the agentic RAG loop with Ollama's tool-use support.
/// Falls back to a prompt-based approach for models without native tool use.
pub async fn run_ollama_agent(
    question: &str,
    scope: AgentScope,
    summary_context: &str,
    db: &Arc<Mutex<Database>>,
    engine: &Arc<OnceCell<EmbeddingEngine>>,
    data_dir: &std::path::Path,
    base_url: &str,
    model: &str,
    channel: &tauri::ipc::Channel<ChatEvent>,
) -> Result<String, String> {
    let engine = engine
        .get_or_try_init(|| async {
            crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .map_err(|e| format!("Embedding engine failed: {}", e))?;

    let system_prompt = build_system_prompt(&scope, summary_context);
    let tools = tool_definitions(&scope);

    // Convert tool definitions to Ollama's format
    let ollama_tools: Vec<serde_json::Value> = tools.iter().map(|t| {
        serde_json::json!({
            "type": "function",
            "function": {
                "name": t["name"],
                "description": t["description"],
                "parameters": t["input_schema"]
            }
        })
    }).collect();

    let mut messages = vec![
        serde_json::json!({ "role": "system", "content": system_prompt }),
        serde_json::json!({ "role": "user", "content": question }),
    ];

    let client = reqwest::Client::new();
    let url = format!("{}/api/chat", base_url);

    for _iteration in 0..MAX_ITERATIONS {
        let request = serde_json::json!({
            "model": model,
            "messages": messages,
            "tools": ollama_tools,
            "stream": false
        });

        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Ollama agent request failed: {}", e))?;

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!("Ollama error: {}", body));
        }

        let body: serde_json::Value = response.json().await
            .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

        let message = &body["message"];
        let content = message["content"].as_str().unwrap_or("");
        let tool_calls = message["tool_calls"].as_array();

        if tool_calls.is_none() || tool_calls.unwrap().is_empty() {
            // Final answer — strip thinking tags and stream to frontend
            let final_text = if let Some(end) = content.find("</think>") {
                content[end + 8..].trim()
            } else {
                content.trim()
            };

            for ch in final_text.chars() {
                let _ = channel.send(ChatEvent::Token {
                    text: ch.to_string(),
                });
            }
            return Ok(final_text.to_string());
        }

        // Add assistant message to conversation
        messages.push(message.clone());

        // Process tool calls
        let tool_calls = tool_calls.unwrap();
        for tc_val in tool_calls {
            let func = &tc_val["function"];
            let name = func["name"].as_str().unwrap_or("");
            let args = &func["arguments"];

            let tc = ToolCall {
                id: format!("ollama_{}", _iteration),
                name: name.to_string(),
                input: args.clone(),
            };

            let _ = channel.send(ChatEvent::ToolCall {
                tool: tc.name.clone(),
                query: tc.input["query"].as_str()
                    .or_else(|| tc.input["document_id"].as_str())
                    .unwrap_or("")
                    .to_string(),
            });

            let result = {
                let db_lock = db.lock().unwrap();
                execute_tool(&tc, &scope, &db_lock, engine)?
            };

            let _ = channel.send(ChatEvent::ToolResult {
                tool: tc.name.clone(),
                summary: truncate_str(&result.content, 100),
            });

            // Add tool result as a tool message
            messages.push(serde_json::json!({
                "role": "tool",
                "content": result.content
            }));
        }
    }

    Err("Agent exceeded maximum iterations".to_string())
}
