use crate::db::{Document, LibraryStats, SearchResult, Task};
use crate::pipeline;
use crate::AppState;
use std::path::PathBuf;
use tauri::State;

/// Find the largest byte index <= `max` that is a char boundary.
fn floor_char_boundary(s: &str, max: usize) -> usize {
    if max >= s.len() {
        return s.len();
    }
    let mut i = max;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

#[derive(serde::Serialize)]
pub struct ImportResult {
    pub document_id: String,
    pub task_id: String,
    pub filename: String,
}

#[tauri::command]
pub async fn import_files(
    paths: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<ImportResult>, String> {
    let mut results = Vec::new();

    for path_str in &paths {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            continue;
        }

        if path.is_dir() {
            let entries: Vec<_> = walkdir::WalkDir::new(&path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
                .filter(|e| {
                    {
                        let ext = e.path().extension().and_then(|x| x.to_str()).unwrap_or("");
                        matches!(ext,
                            "pdf" | "epub" | "mobi" | "txt" | "html" | "htm" | "md" | "djvu" | "cbz" | "cbr"
                        ) || crate::whisper::is_media_format(ext)
                    }
                })
                .collect();

            for entry in entries {
                let doc_id = uuid::Uuid::new_v4().to_string();
                let task_id = uuid::Uuid::new_v4().to_string();
                let filename = entry.file_name().to_string_lossy().to_string();

                {
                    let db = state.db.lock().unwrap();
                    db.create_task(&task_id, &doc_id, "import")
                        .map_err(|e| e.to_string())?;
                }

                let db = state.db.clone();
                let storage = crate::storage::StorageLayout::new(
                    state.storage.originals_dir().parent().unwrap().to_path_buf(),
                );
                let claude = crate::claude::ClaudeClient::new();
                let config = state.config.clone();
                let embeddings = state.embeddings.clone();
                let file_path = entry.path().to_path_buf();
                let tid = task_id.clone();
                let did = doc_id.clone();

                std::thread::spawn({
                    let config2 = config.clone();
                    let db2 = db.clone();
                    move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                    let result = if crate::whisper::is_media_format(ext) {
                        rt.block_on(pipeline::import_media_file(
                            &db, &storage, &claude, &config, &file_path, &tid, &did, &embeddings,
                        ))
                    } else {
                        rt.block_on(pipeline::import_file(
                            &db, &storage, &claude, &config, &file_path, &tid, &did, &embeddings,
                        ))
                    };
                    if let Err(e) = &result {
                        eprintln!("[archivum] import error: {}", e);
                    }
                    // Post-import: generate section summaries (non-fatal)
                    if result.is_ok() {
                        if let Err(e) = rt.block_on(pipeline::generate_section_summaries_async(&db2, &config2, &did)) {
                            eprintln!("[pipeline] section summary generation failed: {}", e);
                        }
                    }
                }});

                results.push(ImportResult {
                    document_id: doc_id,
                    task_id,
                    filename,
                });
            }
        } else {
            let doc_id = uuid::Uuid::new_v4().to_string();
            let task_id = uuid::Uuid::new_v4().to_string();
            let filename = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            {
                let db = state.db.lock().unwrap();
                db.create_task(&task_id, &doc_id, "import")
                    .map_err(|e| e.to_string())?;
            }

            let db = state.db.clone();
            let storage = crate::storage::StorageLayout::new(
                state.storage.originals_dir().parent().unwrap().to_path_buf(),
            );
            let claude = crate::claude::ClaudeClient::new();
            let config = state.config.clone();
            let embeddings = state.embeddings.clone();
            let file_path = path.clone();
            let tid = task_id.clone();
            let did = doc_id.clone();

            std::thread::spawn({
                let config2 = config.clone();
                let db2 = db.clone();
                move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                let ext = file_path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let result = if crate::whisper::is_media_format(ext) {
                    rt.block_on(pipeline::import_media_file(
                        &db, &storage, &claude, &config, &file_path, &tid, &did, &embeddings,
                    ))
                } else {
                    rt.block_on(pipeline::import_file(
                        &db, &storage, &claude, &config, &file_path, &tid, &did, &embeddings,
                    ))
                };
                if let Err(e) = &result {
                    eprintln!("[archivum] import error: {}", e);
                }
                // Post-import: generate section summaries (non-fatal)
                if result.is_ok() {
                    if let Err(e) = rt.block_on(pipeline::generate_section_summaries_async(&db2, &config2, &did)) {
                        eprintln!("[pipeline] section summary generation failed: {}", e);
                    }
                }
            }});

            results.push(ImportResult {
                document_id: doc_id,
                task_id,
                filename,
            });
        }
    }

    Ok(results)
}

#[tauri::command]
pub async fn list_documents(
    offset: Option<i64>,
    limit: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
    format_filter: Option<String>,
    status_filter: Option<String>,
    tag_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<Document>, String> {
    let db = state.db.lock().unwrap();
    db.list_documents(
        offset.unwrap_or(0),
        limit.unwrap_or(50),
        sort_by.as_deref(),
        sort_dir.as_deref(),
        format_filter.as_deref(),
        status_filter.as_deref(),
        tag_filter.as_deref(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_document(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<Document>, String> {
    let db = state.db.lock().unwrap();
    db.get_document(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_document(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.delete_document(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn search_documents(
    query: String,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let db = state.db.lock().unwrap();
    db.search(&query, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_stats(state: State<'_, AppState>) -> Result<LibraryStats, String> {
    let db = state.db.lock().unwrap();
    db.get_stats().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_document_markdown(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state
        .storage
        .read_markdown(&id)
        .map_err(|e| format!("Failed to read markdown: {}", e))
}

#[tauri::command]
pub async fn get_document_cover(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    state
        .storage
        .read_cover(&id)
        .map_err(|e| format!("No cover available: {}", e))
}

#[tauri::command]
pub async fn get_tasks(
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<Task>, String> {
    let db = state.db.lock().unwrap();
    db.list_tasks(limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_task(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.delete_task(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_finished_tasks(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.clear_finished_tasks().map_err(|e| e.to_string())
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct SettingsResponse {
    pub has_api_key: bool,
    pub api_key_preview: String,
    pub model: String,
    pub has_cloudflare: bool,
    pub cloudflare_account_id_preview: String,
    pub selected_whisper_model: Option<String>,
    pub ai_provider: String,
    pub ollama_model: String,
    pub ollama_base_url: String,
}

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsResponse, String> {
    let config_mgr = state.config.lock().unwrap();
    let config = config_mgr.load();

    let (has_key, preview) = match &config.anthropic_api_key {
        Some(key) if !key.is_empty() => {
            let preview = if key.len() > 12 {
                format!("{}...{}", &key[..8], &key[key.len() - 4..])
            } else {
                "*".repeat(key.len())
            };
            (true, preview)
        }
        _ => (false, String::new()),
    };

    let has_cf = config.cloudflare_account_id.as_ref().is_some_and(|s| !s.is_empty())
        && config.cloudflare_api_token.as_ref().is_some_and(|s| !s.is_empty());
    let cf_preview = config.cloudflare_account_id.as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| if s.len() > 8 { format!("{}...", &s[..8]) } else { s.to_string() })
        .unwrap_or_default();

    Ok(SettingsResponse {
        has_api_key: has_key,
        api_key_preview: preview,
        model: config.model.clone(),
        has_cloudflare: has_cf,
        cloudflare_account_id_preview: cf_preview,
        selected_whisper_model: config.selected_whisper_model.clone(),
        ai_provider: config.ai_provider.clone(),
        ollama_model: config.ollama_model.clone(),
        ollama_base_url: config.ollama_base_url.clone(),
    })
}

#[tauri::command]
pub async fn save_settings(
    api_key: Option<String>,
    model: Option<String>,
    cloudflare_account_id: Option<String>,
    cloudflare_api_token: Option<String>,
    ai_provider: Option<String>,
    ollama_model: Option<String>,
    ollama_base_url: Option<String>,
    state: State<'_, AppState>,
) -> Result<SettingsResponse, String> {
    let config_mgr = state.config.lock().unwrap();
    let mut config = config_mgr.load();

    if let Some(key) = api_key {
        config.anthropic_api_key = if key.is_empty() { None } else { Some(key) };
    }
    if let Some(m) = model {
        config.model = m;
    }
    if let Some(id) = cloudflare_account_id {
        config.cloudflare_account_id = if id.is_empty() { None } else { Some(id) };
    }
    if let Some(token) = cloudflare_api_token {
        config.cloudflare_api_token = if token.is_empty() { None } else { Some(token) };
    }
    if let Some(p) = ai_provider {
        config.ai_provider = p;
    }
    if let Some(m) = ollama_model {
        config.ollama_model = m;
    }
    if let Some(u) = ollama_base_url {
        config.ollama_base_url = u;
    }

    config_mgr.save(&config)?;

    let (has_key, preview) = match &config.anthropic_api_key {
        Some(key) if !key.is_empty() => {
            let preview = if key.len() > 12 {
                format!("{}...{}", &key[..8], &key[key.len() - 4..])
            } else {
                "*".repeat(key.len())
            };
            (true, preview)
        }
        _ => (false, String::new()),
    };

    let has_cf = config.cloudflare_account_id.as_ref().is_some_and(|s| !s.is_empty())
        && config.cloudflare_api_token.as_ref().is_some_and(|s| !s.is_empty());
    let cf_preview = config.cloudflare_account_id.as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| if s.len() > 8 { format!("{}...", &s[..8]) } else { s.to_string() })
        .unwrap_or_default();

    Ok(SettingsResponse {
        has_api_key: has_key,
        api_key_preview: preview,
        model: config.model.clone(),
        has_cloudflare: has_cf,
        cloudflare_account_id_preview: cf_preview,
        selected_whisper_model: config.selected_whisper_model.clone(),
        ai_provider: config.ai_provider.clone(),
        ollama_model: config.ollama_model.clone(),
        ollama_base_url: config.ollama_base_url.clone(),
    })
}

#[tauri::command]
pub async fn validate_api_key(api_key: String) -> Result<bool, String> {
    let claude = crate::claude::ClaudeClient::new();
    claude.validate_key(&api_key).await
}

#[tauri::command]
pub async fn import_url(
    url: String,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    let (account_id, api_token) = {
        let cfg = state.config.lock().unwrap().load();
        (
            cfg.cloudflare_account_id
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "Cloudflare Account ID not configured".to_string())?,
            cfg.cloudflare_api_token
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "Cloudflare API Token not configured".to_string())?,
        )
    };

    let doc_id = uuid::Uuid::new_v4().to_string();
    let task_id = uuid::Uuid::new_v4().to_string();

    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, &doc_id, "import")
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let config = state.config.clone();
    let storage = crate::storage::StorageLayout::new(
        state.storage.originals_dir().parent().unwrap().to_path_buf(),
    );
    let claude = crate::claude::ClaudeClient::new();
    let embeddings = state.embeddings.clone();
    let tid = task_id.clone();
    let did = doc_id.clone();
    let import_url = url.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        if let Err(e) = rt.block_on(crate::pipeline::import_from_url(
            &db, &storage, &claude, &config,
            &import_url, &account_id, &api_token,
            &tid, &did, &embeddings,
        )) {
            eprintln!("[archivum] url import error: {}", e);
        }
    });

    let filename = url::Url::parse(&url)
        .map(|u| u.host_str().unwrap_or("webpage").to_string())
        .unwrap_or_else(|_| "webpage".to_string());

    Ok(ImportResult {
        document_id: doc_id,
        task_id,
        filename,
    })
}

#[tauri::command]
pub async fn get_mobi_html(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let original_path = {
        let db = state.db.lock().unwrap();
        let doc = db
            .get_document(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Document not found".to_string())?;
        doc.original_path.clone()
    };
    let full_path = state.storage.resolve_original(&original_path);
    let mobi = mobi::Mobi::from_path(&full_path)
        .map_err(|e| format!("Failed to parse MOBI: {}", e))?;
    Ok(mobi.content_as_string_lossy())
}

#[tauri::command]
pub async fn get_original_bytes(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<u8>, String> {
    let original_path = {
        let db = state.db.lock().unwrap();
        let doc = db
            .get_document(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Document not found".to_string())?;
        doc.original_path.clone()
    };
    let full_path = state.storage.resolve_original(&original_path);
    std::fs::read(&full_path).map_err(|e| format!("Failed to read original: {}", e))
}

#[tauri::command]
pub async fn get_original_path(
    id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let original_path = {
        let db = state.db.lock().unwrap();
        let doc = db
            .get_document(&id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Document not found".to_string())?;
        doc.original_path.clone()
    };
    let full_path = state.storage.resolve_original(&original_path);
    Ok(full_path.to_string_lossy().to_string())
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ChatEvent {
    #[serde(rename = "token")]
    Token { text: String },
    #[serde(rename = "done")]
    Done { full_text: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "context")]
    Context { chunks: Vec<ContextChunk> },
    #[serde(rename = "toolCall")]
    ToolCall { tool: String, query: String },
    #[serde(rename = "toolResult")]
    ToolResult { tool: String, summary: String },
}

#[derive(Clone, serde::Serialize)]
pub struct ContextChunk {
    pub content: String,
    pub chunk_index: usize,
    pub distance: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_title: Option<String>,
}

#[tauri::command]
pub async fn ask_document(
    document_id: String,
    question: String,
    on_token: tauri::ipc::Channel<ChatEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cfg = state.config.lock().unwrap().load();
    let provider = cfg.ai_provider.clone();

    let (title, has_chunks_val) = {
        let db = state.db.lock().unwrap();
        let doc = db
            .get_document(&document_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Document not found".to_string())?;
        let hc = db.has_chunks(&document_id).unwrap_or(false);
        (doc.title.clone(), hc)
    };

    if !has_chunks_val {
        let _ = on_token.send(ChatEvent::Error {
            message: "This document hasn't been indexed yet. Close the chat and click \"Generate index\" first.".to_string(),
        });
        return Ok(());
    }

    // Build summary context from document summary + section summaries
    let summary_context = {
        let db = state.db.lock().unwrap();
        let mut ctx = String::new();
        if let Ok(Some(summary)) = db.get_summary(&document_id, "medium") {
            ctx.push_str(&format!("Document summary: {}\n\n", summary));
        }
        let sections = db.get_section_summaries(&document_id).unwrap_or_default();
        if !sections.is_empty() {
            ctx.push_str("Sections:\n");
            for s in &sections {
                let title_part = s.title.as_deref().unwrap_or("Untitled");
                ctx.push_str(&format!("- {} (chunks {}-{}): {}\n", title_part, s.start_chunk, s.end_chunk, s.summary));
            }
        }
        ctx
    };

    let scope = crate::agent::AgentScope::Document {
        document_id: document_id.clone(),
        title: title.clone(),
    };
    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();

    let result = if provider == "ollama" {
        crate::agent::run_ollama_agent(
            &question, scope, &summary_context,
            &state.db, &state.embeddings, &data_dir,
            &cfg.ollama_base_url, &cfg.ollama_model,
            &on_token,
        ).await
    } else {
        let api_key = cfg.anthropic_api_key
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No API key configured. Add your Anthropic API key in Settings.".to_string())?;
        crate::agent::run_claude_agent(
            &question, scope, &summary_context,
            &state.db, &state.embeddings, &data_dir,
            &api_key, &cfg.model,
            &on_token,
        ).await
    };

    match result {
        Ok(full_text) => {
            let _ = on_token.send(ChatEvent::Done { full_text });
        }
        Err(e) => {
            let _ = on_token.send(ChatEvent::Error { message: e });
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn get_document_has_chunks(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let db = state.db.lock().unwrap();
    db.has_chunks(&document_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn reembed_document(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Try reading existing markdown; if missing, extract from original file
    let markdown = match state.storage.read_markdown(&document_id) {
        Ok(md) => {
            eprintln!("[reembed] Found existing markdown for {} ({} bytes)", document_id, md.len());
            md
        }
        Err(e) => {
            eprintln!("[reembed] No markdown file for {}: {}", document_id, e);
            // No markdown on disk — extract text from the original file
            let doc = {
                let db = state.db.lock().unwrap();
                db.get_document(&document_id)
                    .map_err(|e| e.to_string())?
                    .ok_or_else(|| "Document not found".to_string())?
            };
            let stored_path = state.storage.resolve_original(&doc.original_path);
            let text = crate::pipeline::extract_local_text_public(&stored_path, &doc.original_format)
                .map_err(|e| format!("Failed to extract text: {}", e))?;
            if text.trim().is_empty() {
                return Err("No text content could be extracted from this document".to_string());
            }
            // Write the markdown so future calls don't need to re-extract
            let md_path = state.storage
                .write_markdown(&document_id, &text)
                .map_err(|e| format!("Failed to write markdown: {}", e))?;
            let db = state.db.lock().unwrap();
            db.set_markdown_path(&document_id, &md_path)
                .map_err(|e| e.to_string())?;
            text
        }
    };

    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();
    let engine = state
        .embeddings
        .get_or_try_init(|| async {
            crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .map_err(|e| format!("Embedding engine failed: {}", e))?;

    let chunks = crate::embeddings::chunk_markdown(&markdown);
    eprintln!("[reembed] {} chunks from {} bytes of markdown", chunks.len(), markdown.len());
    if chunks.is_empty() {
        return Err("Document has no content to embed".to_string());
    }

    let embeddings = engine.embed_chunks(&chunks)?;
    eprintln!("[reembed] Embedded {} chunks, storing...", embeddings.len());

    let db = state.db.lock().unwrap();
    db.delete_document_chunks(&document_id)
        .map_err(|e| e.to_string())?;
    db.insert_chunks(&document_id, &chunks, &embeddings)
        .map_err(|e| e.to_string())?;
    eprintln!("[reembed] Done for {}", document_id);

    Ok(())
}

#[derive(serde::Serialize)]
pub struct EmbeddingStats {
    pub total_documents: i64,
    pub embedded_documents: i64,
}

#[tauri::command]
pub async fn search_semantic(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();
    let engine = state
        .embeddings
        .get_or_try_init(|| async {
            crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .map_err(|e| format!("Embedding engine failed: {}", e))?;

    let query_embedding = engine.embed_query(&query)?;

    let db = state.db.lock().unwrap();
    db.search_semantic(&query_embedding, limit.unwrap_or(20))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_related_documents(
    document_id: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String> {
    // Use title + description as the query for topic-level similarity
    let query_text = {
        let db = state.db.lock().unwrap();
        let doc = db
            .get_document(&document_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Document not found".to_string())?;
        let desc = doc.description.unwrap_or_default();
        format!("{}. {}", doc.title, desc)
    };

    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();
    let engine = state
        .embeddings
        .get_or_try_init(|| async {
            crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .map_err(|e| format!("Embedding engine failed: {}", e))?;

    let query_embedding = engine.embed_query(&query_text)?;

    let db = state.db.lock().unwrap();
    db.search_semantic_excluding(&query_embedding, &document_id, limit.unwrap_or(5))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_embedding_stats(
    state: State<'_, AppState>,
) -> Result<EmbeddingStats, String> {
    let db = state.db.lock().unwrap();
    let (total, embedded) = db.get_embedding_stats().map_err(|e| e.to_string())?;
    Ok(EmbeddingStats {
        total_documents: total,
        embedded_documents: embedded,
    })
}

#[tauri::command]
pub async fn batch_reembed(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();

    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, "", "batch-embed")
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let storage = crate::storage::StorageLayout::new(
        state.storage.originals_dir().parent().unwrap().to_path_buf(),
    );
    let embeddings = state.embeddings.clone();
    let tid = task_id.clone();

    let db2 = state.db.clone();
    let tid2 = task_id.clone();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                let data_dir = storage.originals_dir().parent().unwrap().to_path_buf();
                let engine = match embeddings
                    .get_or_try_init(|| async {
                        crate::embeddings::EmbeddingEngine::new(&data_dir.join("models"))
                    })
                    .await
                {
                    Ok(e) => e,
                    Err(e) => {
                        let db = db.lock().unwrap();
                        let _ = db.update_task(&tid, "failed", 0.0, None, Some(&e));
                        return;
                    }
                };

                let doc_ids = {
                    let db = db.lock().unwrap();
                    let _ = db.update_task(&tid, "running", 0.0, Some("Finding documents..."), None);
                    match db.list_unembedded_document_ids() {
                        Ok(ids) => ids,
                        Err(e) => {
                            let _ = db.update_task(&tid, "failed", 0.0, None, Some(&e.to_string()));
                            return;
                        }
                    }
                };

                if doc_ids.is_empty() {
                    let db = db.lock().unwrap();
                    let _ = db.update_task(&tid, "complete", 1.0, Some("All documents already indexed"), None);
                    return;
                }

                let total = doc_ids.len();
                let mut indexed = 0usize;
                let mut failed = 0usize;
                let mut last_error = String::new();

                for (i, doc_id) in doc_ids.iter().enumerate() {
                    {
                        let db = db.lock().unwrap();
                        let _ = db.update_task(
                            &tid,
                            "running",
                            (i as f64) / (total as f64),
                            Some(&format!("Indexing {}/{}", i + 1, total)),
                            None,
                        );
                    }

                    let markdown = match storage.read_markdown(doc_id) {
                        Ok(md) => md,
                        Err(e) => {
                            eprintln!("[batch-embed] Failed to read markdown for {}: {}", doc_id, e);
                            last_error = format!("Read error: {}", e);
                            failed += 1;
                            continue;
                        }
                    };

                    let chunks = crate::embeddings::chunk_markdown(&markdown);
                    if chunks.is_empty() {
                        failed += 1;
                        continue;
                    }

                    let chunk_embeddings = match engine.embed_chunks(&chunks) {
                        Ok(e) => e,
                        Err(e) => {
                            eprintln!("[batch-embed] Embedding failed for {}: {}", doc_id, e);
                            last_error = format!("Embed error: {}", e);
                            failed += 1;
                            continue;
                        }
                    };

                    let db = db.lock().unwrap();
                    match db.insert_chunks(doc_id, &chunks, &chunk_embeddings) {
                        Ok(_) => indexed += 1,
                        Err(e) => {
                            eprintln!("[batch-embed] Insert failed for {}: {}", doc_id, e);
                            last_error = format!("DB error: {}", e);
                            failed += 1;
                        }
                    }
                }

                let msg = if failed > 0 {
                    format!("Indexed {}. {} failed (last: {})", indexed, failed, last_error)
                } else {
                    format!("Indexed {} documents", indexed)
                };

                let db = db.lock().unwrap();
                let _ = db.update_task(&tid, "complete", 1.0, Some(&msg), None);
            });
        }));

        if result.is_err() {
            eprintln!("[batch-embed] Thread panicked");
            if let Ok(db) = db2.lock() {
                let _ = db.update_task(&tid2, "failed", 0.0, None, Some("Internal error (thread panic)"));
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn regenerate_covers(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let task_id = uuid::Uuid::new_v4().to_string();

    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, "", "regenerate-covers")
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let storage = crate::storage::StorageLayout::new(
        state.storage.originals_dir().parent().unwrap().to_path_buf(),
    );
    let tid = task_id.clone();

    let db2 = state.db.clone();
    let tid2 = task_id.clone();

    std::thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let docs = {
                let db = db.lock().unwrap();
                let _ = db.update_task(&tid, "running", 0.0, Some("Finding documents..."), None);
                match db.list_documents_without_covers() {
                    Ok(docs) => docs,
                    Err(e) => {
                        let _ = db.update_task(&tid, "failed", 0.0, None, Some(&e.to_string()));
                        return;
                    }
                }
            };

            if docs.is_empty() {
                let db = db.lock().unwrap();
                let _ = db.update_task(&tid, "complete", 1.0, Some("All documents already have covers"), None);
                return;
            }

            let total = docs.len();
            let mut extracted = 0usize;

            for (i, (doc_id, format, original_path)) in docs.iter().enumerate() {
                {
                    let db = db.lock().unwrap();
                    let _ = db.update_task(
                        &tid,
                        "running",
                        (i as f64) / (total as f64),
                        Some(&format!("Processing {}/{}", i + 1, total)),
                        None,
                    );
                }

                let stored_path = storage.resolve_original(original_path);
                let cover_data = match format.as_str() {
                    "epub" => crate::pipeline::extract_epub_cover_public(&stored_path),
                    "pdf" => crate::pipeline::extract_pdf_cover_public(&stored_path),
                    _ => None,
                };

                if let Some(data) = cover_data {
                    if let Ok(cover_path) = storage.write_cover(doc_id, &data) {
                        let db = db.lock().unwrap();
                        let _ = db.set_cover_path(doc_id, &cover_path);
                        extracted += 1;
                    }
                }
            }

            let msg = format!("Extracted {} covers from {} documents", extracted, total);
            let db = db.lock().unwrap();
            let _ = db.update_task(&tid, "complete", 1.0, Some(&msg), None);
        }));

        if result.is_err() {
            eprintln!("[regenerate-covers] Thread panicked");
            if let Ok(db) = db2.lock() {
                let _ = db.update_task(&tid2, "failed", 0.0, None, Some("Internal error (thread panic)"));
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn update_reading_status(
    id: String,
    reading_status: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.update_reading_status(&id, reading_status.as_deref())
        .map_err(|e| e.to_string())
}

// --- Reading progress ---

#[tauri::command]
pub async fn save_reading_progress(
    id: String,
    position: f64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.save_reading_progress(&id, position)
        .map_err(|e| e.to_string())?;
    // Auto-transition reading status
    let current_status: Option<String> = db
        .get_document(&id)
        .ok()
        .flatten()
        .and_then(|d| d.reading_status);
    if current_status.is_none() || current_status.as_deref() == Some("To Read") {
        let _ = db.update_reading_status(&id, Some("Reading"));
    }
    Ok(())
}

#[tauri::command]
pub async fn get_reading_progress(
    id: String,
    state: State<'_, AppState>,
) -> Result<Option<f64>, String> {
    let db = state.db.lock().unwrap();
    db.get_reading_progress(&id).map_err(|e| e.to_string())
}

// --- Table of contents ---

#[tauri::command]
pub async fn get_document_toc(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::TocEntry>, String> {
    let db = state.db.lock().unwrap();
    let toc = db.get_toc(&id).map_err(|e| e.to_string())?;
    if !toc.is_empty() {
        return Ok(toc);
    }
    // Generate TOC from markdown headings as fallback
    drop(db);
    let markdown = state
        .storage
        .read_markdown(&id)
        .map_err(|e| format!("Failed to read markdown: {}", e))?;
    let entries = crate::pipeline::parse_markdown_toc(&markdown);
    if !entries.is_empty() {
        let db = state.db.lock().unwrap();
        let _ = db.insert_toc(&id, &entries);
    }
    Ok(entries)
}

// --- Summaries ---

#[tauri::command]
pub async fn get_summary(
    id: String,
    length: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let db = state.db.lock().unwrap();
    db.get_summary(&id, &length).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_all_summaries(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<(String, String)>, String> {
    let db = state.db.lock().unwrap();
    db.get_all_summaries(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn generate_summary(
    id: String,
    length: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Check cache first
    {
        let db = state.db.lock().unwrap();
        if let Ok(Some(cached)) = db.get_summary(&id, &length) {
            return Ok(cached);
        }
    }

    let markdown = state
        .storage
        .read_markdown(&id)
        .map_err(|e| format!("Failed to read markdown: {}", e))?;

    let cfg = state.config.lock().unwrap().load();

    let summary = if cfg.ai_provider == "ollama" {
        let ollama = crate::ollama::OllamaClient::new();
        ollama
            .generate_summary(&markdown, &length, &cfg.ollama_base_url, &cfg.ollama_model)
            .await?
    } else {
        let api_key = cfg.anthropic_api_key
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No API key configured".to_string())?;
        let claude = crate::claude::ClaudeClient::new();
        claude
            .generate_summary(&markdown, &length, &api_key, &cfg.model)
            .await?
    };

    let db = state.db.lock().unwrap();
    let _ = db.insert_summary(&id, &length, &summary);

    Ok(summary)
}

// --- Tags ---

#[tauri::command]
pub async fn list_tags(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().unwrap();
    db.list_all_tags().map_err(|e| e.to_string())
}

/// Select chunks with diversity across documents.
/// Fetches a large candidate pool, then round-robins across unique documents
/// to ensure the context covers multiple sources.
pub fn diversify_chunks(
    candidates: Vec<crate::db::LibraryChunkResult>,
    budget: usize,
) -> Vec<crate::db::LibraryChunkResult> {
    use std::collections::HashMap;

    if candidates.len() <= budget {
        return candidates;
    }

    // Group by document, preserving relevance order within each group
    let mut by_doc: HashMap<String, Vec<crate::db::LibraryChunkResult>> = HashMap::new();
    let mut doc_order: Vec<String> = Vec::new();
    for chunk in candidates {
        if !by_doc.contains_key(&chunk.document_id) {
            doc_order.push(chunk.document_id.clone());
        }
        by_doc.entry(chunk.document_id.clone()).or_default().push(chunk);
    }

    // Round-robin: pick one chunk from each document in turn
    let mut result = Vec::with_capacity(budget);
    let mut cursors: HashMap<String, usize> = HashMap::new();
    let mut exhausted = 0;

    while result.len() < budget && exhausted < doc_order.len() {
        exhausted = 0;
        for doc_id in &doc_order {
            if result.len() >= budget {
                break;
            }
            let cursor = cursors.entry(doc_id.clone()).or_insert(0);
            if let Some(chunks) = by_doc.get(doc_id) {
                if *cursor < chunks.len() {
                    result.push(chunks[*cursor].clone());
                    *cursor += 1;
                } else {
                    exhausted += 1;
                }
            } else {
                exhausted += 1;
            }
        }
    }

    result
}

#[tauri::command]
pub async fn ask_library(
    question: String,
    on_token: tauri::ipc::Channel<ChatEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let cfg = state.config.lock().unwrap().load();
    let provider = cfg.ai_provider.clone();

    // Build summary context from library summary + topic clusters
    let summary_context = {
        let db = state.db.lock().unwrap();
        let mut ctx = String::new();
        if let Ok(Some(lib_summary)) = db.get_library_summary() {
            ctx.push_str(&format!("Library overview ({} documents): {}\n", lib_summary.document_count, lib_summary.summary));
            if let Some(themes) = &lib_summary.themes {
                ctx.push_str(&format!("Themes: {}\n", themes));
            }
            ctx.push('\n');
        }
        let clusters = db.get_topic_clusters().unwrap_or_default();
        if !clusters.is_empty() {
            ctx.push_str("Topic clusters:\n");
            for c in &clusters {
                let summary = c.summary.as_deref().unwrap_or("(no summary)");
                ctx.push_str(&format!("- {} ({} docs): {}\n", c.label, c.document_count, summary));
            }
        }
        ctx
    };

    let scope = crate::agent::AgentScope::Library;
    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();

    let result = if provider == "ollama" {
        crate::agent::run_ollama_agent(
            &question, scope, &summary_context,
            &state.db, &state.embeddings, &data_dir,
            &cfg.ollama_base_url, &cfg.ollama_model,
            &on_token,
        ).await
    } else {
        let api_key = cfg.anthropic_api_key
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No API key configured. Add your Anthropic API key in Settings.".to_string())?;
        crate::agent::run_claude_agent(
            &question, scope, &summary_context,
            &state.db, &state.embeddings, &data_dir,
            &api_key, &cfg.model,
            &on_token,
        ).await
    };

    match result {
        Ok(full_text) => {
            let _ = on_token.send(ChatEvent::Done { full_text });
        }
        Err(e) => {
            let _ = on_token.send(ChatEvent::Error { message: e });
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn create_chat_session(
    title: Option<String>,
    document_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<crate::db::ChatSession, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let t = title.unwrap_or_else(|| "New chat".to_string());
    let db = state.db.lock().unwrap();
    db.create_chat_session(&id, &t, document_id.as_deref())
        .map_err(|e| e.to_string())?;
    Ok(crate::db::ChatSession {
        id,
        title: t,
        document_id,
        created_at: String::new(),
        updated_at: String::new(),
    })
}

#[tauri::command]
pub async fn list_chat_sessions(
    document_id: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatSession>, String> {
    let db = state.db.lock().unwrap();
    db.list_chat_sessions(document_id.as_deref(), limit.unwrap_or(50))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_chat_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ChatMessage>, String> {
    let db = state.db.lock().unwrap();
    db.get_chat_messages(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_chat_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.delete_chat_session(&session_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn save_chat_message(
    session_id: String,
    role: String,
    content: String,
    sources: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.insert_chat_message(&session_id, &role, &content, sources.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_session_title(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let db = state.db.lock().unwrap();
    db.update_chat_session_title(&session_id, &title)
        .map_err(|e| e.to_string())
}

// --- Whisper model management ---

#[tauri::command]
pub async fn list_whisper_models(
    state: State<'_, AppState>,
) -> Result<Vec<crate::whisper::WhisperModel>, String> {
    let db = state.db.lock().unwrap();
    // Seed manifest on first call
    let _ = db.seed_whisper_models();

    let mut models = db.list_whisper_models().map_err(|e| e.to_string())?;

    // Sync status with filesystem — a model may have been deleted externally
    let originals = state.storage.originals_dir();
    let data_dir = originals.parent().unwrap();
    for model in &mut models {
        if model.status == "ready" && !crate::whisper::is_model_downloaded(data_dir, &model.filename) {
            model.status = "available".to_string();
            let _ = db.update_whisper_model_status(&model.id, "available", 0.0, None);
        }
    }

    Ok(models)
}

#[tauri::command]
pub async fn download_whisper_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Find the model info
    let model_info = crate::whisper::WHISPER_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let task_id = uuid::Uuid::new_v4().to_string();
    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, "", "model-download")
            .map_err(|e| e.to_string())?;
        db.update_whisper_model_status(&model_id, "downloading", 0.0, None)
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let data_dir = state.storage.originals_dir().parent().unwrap().to_path_buf();
    let url = model_info.url.to_string();
    let filename = model_info.filename.to_string();
    let mid = model_id.clone();
    let tid = task_id.clone();

    std::thread::spawn(move || {
        let dest = crate::whisper::model_path(&data_dir, &filename);
        let db_ref = &db;
        let mid_ref = &mid;
        let tid_ref = &tid;

        let result = crate::whisper::download_model(&url, &dest, &|progress| {
            let db_lock = db_ref.lock().unwrap();
            let _ = db_lock.update_whisper_model_status(mid_ref, "downloading", progress, None);
            let _ = db_lock.update_task(
                tid_ref,
                "running",
                progress,
                Some(&format!("Downloading... {:.0}%", progress * 100.0)),
                None,
            );
        });

        let db_lock = db_ref.lock().unwrap();
        match result {
            Ok(()) => {
                let _ = db_lock.update_whisper_model_status(&mid, "ready", 1.0, None);
                let _ = db_lock.update_task(&tid, "complete", 1.0, Some("Download complete"), None);
            }
            Err(e) => {
                let _ = db_lock.update_whisper_model_status(&mid, "error", 0.0, Some(&e));
                let _ = db_lock.update_task(&tid, "failed", 0.0, None, Some(&e));
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn delete_whisper_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let model_info = crate::whisper::WHISPER_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown model: {}", model_id))?;

    let originals_dir = state.storage.originals_dir();
    let data_dir = originals_dir.parent().unwrap();
    crate::whisper::delete_model_file(data_dir, model_info.filename)?;

    let db = state.db.lock().unwrap();
    db.update_whisper_model_status(&model_id, "available", 0.0, None)
        .map_err(|e| e.to_string())?;

    // If this was the selected model, clear selection
    let config = state.config.lock().unwrap();
    let mut cfg = config.load();
    if cfg.selected_whisper_model.as_deref() == Some(&model_id) {
        cfg.selected_whisper_model = None;
        config.save(&cfg)?;
    }

    Ok(())
}

#[tauri::command]
pub async fn select_whisper_model(
    model_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let config_mgr = state.config.lock().unwrap();
    let mut config = config_mgr.load();
    config.selected_whisper_model = if model_id.is_empty() { None } else { Some(model_id) };
    config_mgr.save(&config)
}

#[tauri::command]
pub async fn check_external_tools() -> Result<crate::whisper::ExternalToolsStatus, String> {
    Ok(crate::whisper::check_external_tools())
}

#[tauri::command]
pub async fn import_youtube(
    url: String,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    let doc_id = uuid::Uuid::new_v4().to_string();
    let task_id = uuid::Uuid::new_v4().to_string();

    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, &doc_id, "import")
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let storage = crate::storage::StorageLayout::new(
        state.storage.originals_dir().parent().unwrap().to_path_buf(),
    );
    let claude = crate::claude::ClaudeClient::new();
    let config = state.config.clone();
    let embeddings = state.embeddings.clone();
    let tid = task_id.clone();
    let did = doc_id.clone();
    let url_clone = url.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        if let Err(e) = rt.block_on(pipeline::import_youtube(
            &db, &storage, &claude, &config, &url_clone, &tid, &did, &embeddings,
        )) {
            eprintln!("[archivum] youtube import error: {}", e);
            let db_lock = db.lock().unwrap();
            let _ = db_lock.update_task(&tid, "failed", 0.0, None, Some(&e));
        }
    });

    Ok(ImportResult {
        document_id: doc_id,
        task_id,
        filename: url,
    })
}

// --- Ollama management ---

#[derive(serde::Serialize, Clone)]
pub struct OllamaStatusResponse {
    pub available: bool,
    pub version: Option<String>,
}

#[tauri::command]
pub async fn check_ollama_status(
    state: State<'_, AppState>,
) -> Result<OllamaStatusResponse, String> {
    let base_url = state.config.lock().unwrap().load().ollama_base_url;
    let client = crate::ollama::OllamaClient::new();
    let status = client.check_status(&base_url).await;
    Ok(OllamaStatusResponse {
        available: status.available,
        version: status.version,
    })
}

#[tauri::command]
pub async fn list_ollama_models(
    state: State<'_, AppState>,
) -> Result<Vec<crate::ollama::OllamaModelInfo>, String> {
    let base_url = state.config.lock().unwrap().load().ollama_base_url;
    let client = crate::ollama::OllamaClient::new();
    client.list_models(&base_url).await
}

#[derive(serde::Serialize, Clone)]
pub struct RecommendedModelInfo {
    pub name: String,
    pub label: String,
    pub description: String,
}

#[tauri::command]
pub async fn list_recommended_ollama_models() -> Result<Vec<RecommendedModelInfo>, String> {
    Ok(crate::ollama::RECOMMENDED_MODELS
        .iter()
        .map(|m| RecommendedModelInfo {
            name: m.name.to_string(),
            label: m.label.to_string(),
            description: m.description.to_string(),
        })
        .collect())
}

#[derive(serde::Serialize, Clone)]
pub struct HardwareInfo {
    pub total_ram_gb: f64,
    pub cpu_name: String,
    pub gpu_name: Option<String>,
    pub gpu_vram_gb: Option<f64>,
    pub unified_memory: bool,
    pub backend: String,
}

#[tauri::command]
pub async fn get_system_hardware(
    state: State<'_, AppState>,
) -> Result<HardwareInfo, String> {
    let specs = &state.system_specs;
    Ok(HardwareInfo {
        total_ram_gb: specs.total_ram_gb,
        cpu_name: specs.cpu_name.clone(),
        gpu_name: specs.gpu_name.clone(),
        gpu_vram_gb: specs.gpu_vram_gb,
        unified_memory: specs.unified_memory,
        backend: specs.backend.label().to_string(),
    })
}

#[derive(serde::Serialize, Clone)]
pub struct ModelFitInfo {
    pub name: String,
    pub parameter_count: String,
    pub use_case: String,
    pub fit_level: String,
    pub run_mode: String,
    pub memory_required_gb: f64,
    pub estimated_tps: f64,
    pub best_quant: String,
    pub score: f64,
    pub score_quality: f64,
    pub score_speed: f64,
    pub score_fit: f64,
    pub score_context: f64,
    pub context_length: u32,
    pub installed: bool,
}

#[tauri::command]
pub async fn get_model_fits(
    state: State<'_, AppState>,
    limit: Option<usize>,
    use_case_filter: Option<String>,
) -> Result<Vec<ModelFitInfo>, String> {
    let specs = &state.system_specs;
    let db = llmfit_core::ModelDatabase::new();

    // Get installed Ollama model names for cross-referencing
    let base_url = state.config.lock().unwrap().load().ollama_base_url;
    let client = crate::ollama::OllamaClient::new();
    let installed_names: std::collections::HashSet<String> =
        match client.list_models(&base_url).await {
            Ok(models) => models.into_iter().map(|m| m.name.to_lowercase()).collect(),
            Err(_) => std::collections::HashSet::new(),
        };

    let mut fits: Vec<ModelFitInfo> = db
        .get_all_models()
        .iter()
        .map(|model| {
            let fit = llmfit_core::ModelFit::analyze(model, specs);
            let name_lower = model.name.to_lowercase();
            let stem = name_lower.split(':').next().unwrap_or(&name_lower);
            let installed = installed_names.contains(&name_lower)
                || installed_names.contains(&format!("{}:latest", stem));
            ModelFitInfo {
                name: model.name.clone(),
                parameter_count: model.parameter_count.clone(),
                use_case: fit.use_case.label().to_string(),
                fit_level: format!("{:?}", fit.fit_level),
                run_mode: format!("{:?}", fit.run_mode),
                memory_required_gb: fit.memory_required_gb,
                estimated_tps: fit.estimated_tps,
                best_quant: fit.best_quant.clone(),
                score: fit.score,
                score_quality: fit.score_components.quality,
                score_speed: fit.score_components.speed,
                score_fit: fit.score_components.fit,
                score_context: fit.score_components.context,
                context_length: model.context_length,
                installed,
            }
        })
        .filter(|f| f.fit_level != "TooTight")
        .filter(|f| {
            if let Some(ref uc) = use_case_filter {
                f.use_case.to_lowercase() == uc.to_lowercase()
            } else {
                true
            }
        })
        .collect();

    fits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    let limit = limit.unwrap_or(30);
    fits.truncate(limit);

    Ok(fits)
}

#[tauri::command]
pub async fn pull_ollama_model(
    name: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let base_url = state.config.lock().unwrap().load().ollama_base_url;
    let task_id = uuid::Uuid::new_v4().to_string();

    {
        let db = state.db.lock().unwrap();
        db.create_task(&task_id, "", "ollama-pull")
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.clone();
    let tid = task_id.clone();

    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let client = crate::ollama::OllamaClient::new();
        let result = rt.block_on(client.pull_model(&base_url, &name, |progress, status| {
            let db_lock = db.lock().unwrap();
            let _ = db_lock.update_task(
                &tid,
                "running",
                progress,
                Some(&format!("{} ({:.0}%)", status, progress * 100.0)),
                None,
            );
        }));

        let db_lock = db.lock().unwrap();
        match result {
            Ok(()) => {
                let _ = db_lock.update_task(&tid, "complete", 1.0, Some("Model pulled"), None);
            }
            Err(e) => {
                let _ = db_lock.update_task(&tid, "failed", 0.0, None, Some(&e));
            }
        }
    });

    Ok(task_id)
}

#[tauri::command]
pub async fn delete_ollama_model(
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let base_url = state.config.lock().unwrap().load().ollama_base_url;
    let client = crate::ollama::OllamaClient::new();
    client.delete_model(&base_url, &name).await
}

// --- Hierarchical Summaries ---

#[tauri::command]
pub async fn generate_section_summaries(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::SectionSummary>, String> {
    let cfg = state.config.lock().unwrap().load();

    // Get all chunks for this document
    let chunks = {
        let db = state.db.lock().unwrap();
        db.get_document_chunks(&document_id).map_err(|e| e.to_string())?
    };

    if chunks.is_empty() {
        return Err("No chunks found. Index the document first.".to_string());
    }

    // Group chunks into sections of 4
    let section_size = 4;
    let mut sections = Vec::new();

    for group in chunks.chunks(section_size) {
        let start = group.first().unwrap().0;
        let end = group.last().unwrap().0;
        let combined_text: String = group.iter()
            .map(|(_, content)| content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");

        let prompt = format!(
            "Summarize the following text section in 2-3 sentences. \
             Also extract 3-5 key concepts as a comma-separated list.\n\n\
             Respond with JSON: {{\"title\": \"section title\", \"summary\": \"...\", \"key_concepts\": \"concept1, concept2, ...\"}}\n\n\
             Text:\n{}\n\nRespond with ONLY the JSON.",
            &combined_text[..floor_char_boundary(&combined_text, 6000)]
        );

        let result = if cfg.ai_provider == "ollama" {
            let ollama = crate::ollama::OllamaClient::new();
            ollama.generate_json(&cfg.ollama_base_url, &cfg.ollama_model, &prompt).await
        } else {
            let api_key = cfg.anthropic_api_key.as_deref()
                .filter(|s| !s.is_empty())
                .ok_or_else(|| "No API key configured".to_string())?;
            let claude = crate::claude::ClaudeClient::new();
            claude.generate_json(api_key, &cfg.model, &prompt).await
        };

        match result {
            Ok(json) => {
                let title = json["title"].as_str().map(|s| s.to_string());
                let summary = json["summary"].as_str().unwrap_or("").to_string();
                let key_concepts = json["key_concepts"].as_str().map(|s| s.to_string());
                sections.push((start, end, title, summary, key_concepts));
            }
            Err(e) => {
                eprintln!("[sections] Failed to summarize chunks {}-{}: {}", start, end, e);
                sections.push((start, end, None, combined_text[..floor_char_boundary(&combined_text, 200)].to_string(), None));
            }
        }
    }

    // Store in database
    {
        let db = state.db.lock().unwrap();
        let section_refs: Vec<(i32, i32, Option<&str>, &str, Option<&str>)> = sections.iter()
            .map(|(s, e, t, sum, c)| (*s, *e, t.as_deref(), sum.as_str(), c.as_deref()))
            .collect();
        db.insert_section_summaries(&document_id, &section_refs)
            .map_err(|e| e.to_string())?;
    }

    let db = state.db.lock().unwrap();
    db.get_section_summaries(&document_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_section_summaries(
    document_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::SectionSummary>, String> {
    let db = state.db.lock().unwrap();
    db.get_section_summaries(&document_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_topic_clusters(
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::TopicCluster>, String> {
    let db = state.db.lock().unwrap();
    db.get_topic_clusters().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_library_overview(
    state: State<'_, AppState>,
) -> Result<Option<crate::db::LibrarySummary>, String> {
    let db = state.db.lock().unwrap();
    db.get_library_summary().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn refresh_library_summary(
    state: State<'_, AppState>,
) -> Result<String, String> {
    let cfg = state.config.lock().unwrap().load();

    // Get all document summaries
    let docs = {
        let db = state.db.lock().unwrap();
        db.list_document_ids_and_titles().map_err(|e| e.to_string())?
    };

    if docs.is_empty() {
        return Err("No documents in library".to_string());
    }

    // Collect document descriptions — use cached summary if available, fall back to metadata
    let mut doc_summaries = Vec::new();
    {
        let db = state.db.lock().unwrap();
        for (id, title, author) in &docs {
            if let Ok(Some(summary)) = db.get_summary(id, "short") {
                doc_summaries.push(format!("\"{}\" by {}: {}", title, author, summary));
            } else if let Ok(Some(doc)) = db.get_document(id) {
                let desc = doc.description.unwrap_or_default();
                let tags = if doc.tags.is_empty() { String::new() } else { format!(" [{}]", doc.tags.join(", ")) };
                if !desc.is_empty() {
                    doc_summaries.push(format!("\"{}\" by {}: {}{}", title, author, desc, tags));
                } else {
                    doc_summaries.push(format!("\"{}\" by {}{}", title, author, tags));
                }
            }
        }
    }

    if doc_summaries.is_empty() {
        return Err("No documents found.".to_string());
    }

    // Cap the combined summaries to ~20k chars to stay within token limits
    let mut combined = String::new();
    for s in &doc_summaries {
        if combined.len() + s.len() > 20_000 {
            combined.push_str(&format!("\n\n... and {} more documents", doc_summaries.len() - combined.matches("\n\n").count()));
            break;
        }
        if !combined.is_empty() {
            combined.push_str("\n\n");
        }
        combined.push_str(s);
    }

    let prompt = format!(
        "You have a library of {} documents. Here are descriptions of each:\n\n{}\n\n\
         Write a 3-5 sentence overview of this library's contents, themes, and scope.\n\
         Also list 5-10 major themes as a JSON array.\n\n\
         Respond with JSON: {{\"summary\": \"...\", \"themes\": [\"theme1\", \"theme2\", ...]}}\n\n\
         Respond with ONLY the JSON.",
        docs.len(),
        combined
    );

    let result = if cfg.ai_provider == "ollama" {
        let ollama = crate::ollama::OllamaClient::new();
        ollama.generate_json(&cfg.ollama_base_url, &cfg.ollama_model, &prompt).await
    } else {
        let api_key = cfg.anthropic_api_key.as_deref()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| "No API key configured".to_string())?;
        let claude = crate::claude::ClaudeClient::new();
        claude.generate_json(api_key, &cfg.model, &prompt).await
    }?;

    let summary = result["summary"].as_str().unwrap_or("").to_string();
    let themes = result["themes"].as_array()
        .map(|arr| serde_json::to_string(arr).unwrap_or_default());

    {
        let db = state.db.lock().unwrap();
        db.upsert_library_summary(&summary, themes.as_deref(), docs.len() as i64)
            .map_err(|e| e.to_string())?;
    }

    Ok(summary)
}
