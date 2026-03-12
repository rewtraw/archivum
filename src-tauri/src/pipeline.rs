use crate::claude::ClaudeClient;
use crate::config::ConfigManager;
use crate::db::Database;
use crate::storage::StorageLayout;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Extract text locally from a PDF using pdf-extract
fn extract_pdf_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF: {}", e))?;
    pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))
}

/// Extract text locally from a text-based file
fn extract_text_content(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))
}

/// Derive a basic title from the filename
fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

/// Import a single file: hash → dedup → store → extract text → optionally enrich with Claude
pub async fn import_file(
    db: &Arc<Mutex<Database>>,
    storage: &StorageLayout,
    claude: &ClaudeClient,
    config: &Arc<Mutex<ConfigManager>>,
    source_path: &Path,
    task_id: &str,
    doc_id: &str,
) -> Result<String, String> {
    // 1. Detect format
    let format = StorageLayout::detect_format(source_path)
        .ok_or_else(|| "Could not detect file format".to_string())?;

    // 2. Hash file
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.1, Some("Hashing file..."), None)
            .map_err(|e| e.to_string())?;
    }

    let file_hash = StorageLayout::compute_hash(source_path)
        .map_err(|e| format!("Failed to hash file: {}", e))?;

    // 3. Dedup check
    {
        let db_lock = db.lock().unwrap();
        if db_lock.has_hash(&file_hash).map_err(|e| e.to_string())? {
            db_lock
                .update_task(
                    task_id,
                    "complete",
                    1.0,
                    Some("Duplicate — already in library"),
                    None,
                )
                .map_err(|e| e.to_string())?;
            return Err("File already exists in library".to_string());
        }
    }

    // 4. Store original
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.2, Some("Storing file..."), None)
            .map_err(|e| e.to_string())?;
    }

    let file_size = StorageLayout::file_size(source_path)
        .map_err(|e| format!("Failed to get file size: {}", e))? as i64;

    let original_path = storage
        .store_original(source_path, &file_hash, &format)
        .map_err(|e| format!("Failed to store file: {}", e))?;

    // 5. Create document record
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .insert_document(doc_id, &format, &file_hash, file_size, &original_path)
            .map_err(|e| format!("Failed to create document: {}", e))?;
    }

    // 6. Local text extraction (always works, no API needed)
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.3, Some("Extracting text..."), None)
            .map_err(|e| e.to_string())?;
    }

    let stored_path = storage.resolve_original(&original_path);
    let local_text = match format.as_str() {
        "pdf" => extract_pdf_text(&stored_path),
        "txt" | "md" | "html" | "htm" => extract_text_content(&stored_path),
        _ => Ok(String::new()),
    };

    let fallback_title = title_from_filename(source_path);

    // 7. Try Claude enrichment if API key is configured
    let (api_key, model) = {
        let cfg = config.lock().unwrap().load();
        (cfg.anthropic_api_key.unwrap_or_default(), cfg.model)
    };

    if !api_key.is_empty() {
        {
            let db_lock = db.lock().unwrap();
            db_lock
                .update_task(
                    task_id,
                    "running",
                    0.4,
                    Some("Extracting with Claude..."),
                    None,
                )
                .map_err(|e| e.to_string())?;
        }

        match claude
            .extract_document(&stored_path, &format, &api_key, &model)
            .await
        {
            Ok(result) => {
                return finish_import(
                    db,
                    storage,
                    task_id,
                    doc_id,
                    &result.title,
                    &result.author,
                    result.description.as_deref(),
                    result.language.as_deref(),
                    result.isbn.as_deref(),
                    result.publisher.as_deref(),
                    result.published_date.as_deref(),
                    result.page_count,
                    &result.markdown_content,
                    &result.tags,
                    None,
                );
            }
            Err(e) => {
                eprintln!("[pipeline] Claude extraction failed, using local text: {}", e);
                // Fall through to local-only path
            }
        }
    }

    // 8. Local-only import (no Claude or Claude failed)
    let content = local_text.unwrap_or_default();
    if content.is_empty() {
        // Store the document but mark it as needing processing
        let db_lock = db.lock().unwrap();
        db_lock
            .update_document_metadata(
                doc_id,
                &fallback_title,
                "Unknown",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                "complete",
            )
            .map_err(|e| e.to_string())?;
        db_lock
            .update_task(
                task_id,
                "complete",
                1.0,
                Some("Imported (no text extracted)"),
                None,
            )
            .map_err(|e| e.to_string())?;
        return Ok(doc_id.to_string());
    }

    finish_import(
        db,
        storage,
        task_id,
        doc_id,
        &fallback_title,
        "Unknown",
        None,
        None,
        None,
        None,
        None,
        None,
        &content,
        &[],
        if api_key.is_empty() {
            None
        } else {
            Some("Claude unavailable — imported with local text extraction")
        },
    )
}

fn finish_import(
    db: &Arc<Mutex<Database>>,
    storage: &StorageLayout,
    task_id: &str,
    doc_id: &str,
    title: &str,
    author: &str,
    description: Option<&str>,
    language: Option<&str>,
    isbn: Option<&str>,
    publisher: Option<&str>,
    published_date: Option<&str>,
    page_count: Option<i32>,
    markdown_content: &str,
    tags: &[String],
    task_note: Option<&str>,
) -> Result<String, String> {
    let md_path = storage
        .write_markdown(doc_id, markdown_content)
        .map_err(|e| format!("Failed to write markdown: {}", e))?;

    let db_lock = db.lock().unwrap();
    db_lock
        .update_document_metadata(
            doc_id,
            title,
            author,
            description,
            language,
            isbn,
            publisher,
            published_date,
            page_count,
            Some(&md_path),
            None,
            "complete",
        )
        .map_err(|e| e.to_string())?;

    db_lock
        .upsert_fts(
            doc_id,
            title,
            author,
            description.unwrap_or(""),
            markdown_content,
            &tags.join(" "),
        )
        .map_err(|e| e.to_string())?;

    let message = task_note.unwrap_or("Import complete");
    db_lock
        .update_task(task_id, "complete", 1.0, Some(message), None)
        .map_err(|e| e.to_string())?;

    Ok(doc_id.to_string())
}
