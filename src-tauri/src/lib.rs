mod claude;
mod commands;
mod config;
mod db;
mod pipeline;
mod storage;

use std::sync::{Arc, Mutex};
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<db::Database>>,
    pub storage: storage::StorageLayout,
    pub config: Arc<Mutex<config::ConfigManager>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
