mod claude;
mod commands;
mod config;
mod db;
mod embeddings;
pub mod ollama;
mod pipeline;
mod storage;
pub mod whisper;

use std::sync::{Arc, Mutex};
use tauri::Manager;
use tokio::sync::OnceCell;

pub struct AppState {
    pub db: Arc<Mutex<db::Database>>,
    pub storage: storage::StorageLayout,
    pub config: Arc<Mutex<config::ConfigManager>>,
    pub embeddings: Arc<OnceCell<embeddings::EmbeddingEngine>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Register sqlite-vec extension before any database connection
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(
            std::mem::transmute(sqlite_vec::sqlite3_vec_init as *const ()),
        ));
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");

            std::fs::create_dir_all(&data_dir).expect("failed to create data dir");

            let database = db::Database::open(&data_dir.join("archivum.db"))
                .expect("failed to open database");
            database.initialize().expect("failed to initialize schema");

            let storage = storage::StorageLayout::new(data_dir.join("storage"));
            storage.ensure_dirs().expect("failed to create storage dirs");

            let config_mgr = config::ConfigManager::new(&data_dir);

            let state = AppState {
                db: Arc::new(Mutex::new(database)),
                storage,
                config: Arc::new(Mutex::new(config_mgr)),
                embeddings: Arc::new(OnceCell::new()),
            };

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::import_files,
            commands::list_documents,
            commands::get_document,
            commands::delete_document,
            commands::search_documents,
            commands::get_stats,
            commands::get_document_markdown,
            commands::get_document_cover,
            commands::get_tasks,
            commands::delete_task,
            commands::clear_finished_tasks,
            commands::get_settings,
            commands::save_settings,
            commands::validate_api_key,
            commands::get_mobi_html,
            commands::get_original_bytes,
            commands::get_original_path,
            commands::import_url,
            commands::ask_document,
            commands::get_document_has_chunks,
            commands::reembed_document,
            commands::ask_library,
            commands::create_chat_session,
            commands::list_chat_sessions,
            commands::get_chat_messages,
            commands::delete_chat_session,
            commands::save_chat_message,
            commands::update_session_title,
            commands::search_semantic,
            commands::get_related_documents,
            commands::get_embedding_stats,
            commands::batch_reembed,
            commands::update_reading_status,
            commands::regenerate_covers,
            commands::save_reading_progress,
            commands::get_reading_progress,
            commands::get_document_toc,
            commands::get_summary,
            commands::get_all_summaries,
            commands::generate_summary,
            commands::list_tags,
            commands::list_whisper_models,
            commands::download_whisper_model,
            commands::delete_whisper_model,
            commands::select_whisper_model,
            commands::check_external_tools,
            commands::import_youtube,
            commands::check_ollama_status,
            commands::list_ollama_models,
            commands::list_recommended_ollama_models,
            commands::pull_ollama_model,
            commands::delete_ollama_model,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
