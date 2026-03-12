use crate::db::{Document, LibraryStats, SearchResult, Task};
use crate::pipeline;
use crate::AppState;
use std::path::PathBuf;
use tauri::State;

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
                    matches!(
                        e.path().extension().and_then(|x| x.to_str()),
                        Some("pdf" | "epub" | "mobi" | "txt" | "html" | "htm" | "md" | "djvu" | "cbz" | "cbr")
                    )
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
                let file_path = entry.path().to_path_buf();
                let tid = task_id.clone();
                let did = doc_id.clone();

                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().unwrap();
                    if let Err(e) = rt.block_on(pipeline::import_file(
                        &db, &storage, &claude, &config, &file_path, &tid, &did,
                    )) {
                        eprintln!("[archivum] import error: {}", e);
                    }
                });

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
            let file_path = path.clone();
            let tid = task_id.clone();
            let did = doc_id.clone();

            std::thread::spawn(move || {
                let rt = tokio::runtime::Runtime::new().unwrap();
                if let Err(e) = rt.block_on(pipeline::import_file(
                    &db, &storage, &claude, &config, &file_path, &tid, &did,
                )) {
                    eprintln!("[archivum] import error: {}", e);
                }
            });

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
    state: State<'_, AppState>,
) -> Result<Vec<Document>, String> {
    let db = state.db.lock().unwrap();
    db.list_documents(offset.unwrap_or(0), limit.unwrap_or(50))
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

    Ok(SettingsResponse {
        has_api_key: has_key,
        api_key_preview: preview,
        model: config.model,
    })
}

#[tauri::command]
pub async fn save_settings(
    api_key: Option<String>,
    model: Option<String>,
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

    Ok(SettingsResponse {
        has_api_key: has_key,
        api_key_preview: preview,
        model: config.model,
    })
}

#[tauri::command]
pub async fn validate_api_key(api_key: String) -> Result<bool, String> {
    let claude = crate::claude::ClaudeClient::new();
    claude.validate_key(&api_key).await
}
