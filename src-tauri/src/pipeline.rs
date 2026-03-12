use crate::claude::ClaudeClient;
use crate::config::ConfigManager;
use crate::db::Database;
use crate::storage::StorageLayout;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Extract text locally from a PDF using pdf-extract
fn extract_pdf_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF: {}", e))?;
    pdf_extract::extract_text_from_mem(&bytes)
        .map_err(|e| format!("Failed to extract PDF text: {}", e))
}

/// Extract text from an EPUB file (ZIP of XHTML files)
fn extract_epub_text(path: &Path) -> Result<String, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("Failed to open EPUB: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read EPUB archive: {}", e))?;

    // Read the container.xml to find the content files in order
    let content_paths = epub_content_order(&mut archive);

    let mut all_text = String::new();
    for content_path in &content_paths {
        if let Ok(mut entry) = archive.by_name(content_path) {
            let mut html = String::new();
            if entry.read_to_string(&mut html).is_ok() {
                let text = html2text::from_read(html.as_bytes(), 120)
                    .unwrap_or_default();
                if !text.trim().is_empty() {
                    all_text.push_str(&text);
                    all_text.push('\n');
                }
            }
        }
    }

    // Fallback: if content order detection failed, just grab all xhtml/html files
    if all_text.trim().is_empty() {
        for i in 0..archive.len() {
            if let Ok(mut entry) = archive.by_index(i) {
                let name = entry.name().to_lowercase();
                if name.ends_with(".xhtml")
                    || name.ends_with(".html")
                    || name.ends_with(".htm")
                    || name.ends_with(".xml")
                {
                    let mut html = String::new();
                    if entry.read_to_string(&mut html).is_ok() {
                        let text = html2text::from_read(html.as_bytes(), 120)
                            .unwrap_or_default();
                        if !text.trim().is_empty() {
                            all_text.push_str(&text);
                            all_text.push('\n');
                        }
                    }
                }
            }
        }
    }

    if all_text.trim().is_empty() {
        Err("No text content found in EPUB".to_string())
    } else {
        Ok(all_text)
    }
}

/// Parse EPUB's container.xml → content.opf → spine to get reading order
fn epub_content_order(archive: &mut zip::ZipArchive<std::fs::File>) -> Vec<String> {
    // 1. Find the .opf file path from META-INF/container.xml
    let opf_path = (|| -> Option<String> {
        let mut container = archive.by_name("META-INF/container.xml").ok()?;
        let mut xml = String::new();
        container.read_to_string(&mut xml).ok()?;
        // Quick parse: find rootfile full-path="..."
        let marker = "full-path=\"";
        let start = xml.find(marker)? + marker.len();
        let end = xml[start..].find('"')? + start;
        Some(xml[start..end].to_string())
    })();

    let opf_path = match opf_path {
        Some(p) => p,
        None => return Vec::new(),
    };

    // Base directory of the OPF file
    let opf_dir = opf_path
        .rfind('/')
        .map(|i| &opf_path[..i + 1])
        .unwrap_or("");

    // 2. Parse the OPF to get manifest (id → href) and spine (ordered idrefs)
    let parsed = (|| -> Option<(Vec<(String, String)>, Vec<String>)> {
        let mut opf_entry = archive.by_name(&opf_path).ok()?;
        let mut xml = String::new();
        opf_entry.read_to_string(&mut xml).ok()?;

        // Extract manifest items: <item id="..." href="..." .../>
        let mut manifest = Vec::new();
        for item_start in xml.match_indices("<item ").map(|(i, _)| i) {
            let chunk = &xml[item_start..];
            let item_end = match chunk.find("/>").or_else(|| chunk.find("</item>")) {
                Some(e) => e,
                None => continue,
            };
            let tag = &chunk[..item_end];

            if let (Some(id), Some(href)) = (extract_attr(tag, "id"), extract_attr(tag, "href")) {
                manifest.push((id, href));
            }
        }

        // Extract spine idrefs: <itemref idref="..."/>
        let mut spine = Vec::new();
        for itemref_start in xml.match_indices("<itemref ").map(|(i, _)| i) {
            let chunk = &xml[itemref_start..];
            let tag_end = match chunk.find("/>") {
                Some(e) => e,
                None => continue,
            };
            let tag = &chunk[..tag_end];
            if let Some(idref) = extract_attr(tag, "idref") {
                spine.push(idref);
            }
        }

        Some((manifest, spine))
    })();

    let (manifest, spine) = match parsed {
        Some((m, s)) => (m, s),
        None => return Vec::new(),
    };

    // 3. Resolve spine idrefs to file paths
    spine
        .iter()
        .filter_map(|idref| {
            manifest
                .iter()
                .find(|(id, _)| id == idref)
                .map(|(_, href)| format!("{}{}", opf_dir, href))
        })
        .collect()
}

/// Extract an XML attribute value from a tag string
fn extract_attr(tag: &str, attr: &str) -> Option<String> {
    let pattern = format!("{}=\"", attr);
    let start = tag.find(&pattern)? + pattern.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

/// Extract text from a MOBI file (content is HTML internally)
fn extract_mobi_text(path: &Path) -> Result<String, String> {
    let mobi =
        mobi::Mobi::from_path(path).map_err(|e| format!("Failed to parse MOBI: {}", e))?;
    let html = mobi.content_as_string_lossy();
    html2text::from_read(html.as_bytes(), 120)
        .map_err(|e| format!("Failed to convert MOBI HTML to text: {}", e))
}

/// Extract text from a DjVu file using djvutxt (if installed)
fn extract_djvu_text(path: &Path) -> Result<String, String> {
    let output = std::process::Command::new("djvutxt")
        .arg(path)
        .output()
        .map_err(|_| "DjVu text extraction requires djvutxt (brew install djvulibre)".to_string())?;

    if !output.status.success() {
        return Err("djvutxt failed — file may not have a text layer".to_string());
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    if text.trim().is_empty() {
        Err("DjVu file has no text layer".to_string())
    } else {
        Ok(text)
    }
}

/// Extract text from an HTML file
fn extract_html_text(path: &Path) -> Result<String, String> {
    let html = std::fs::read(path).map_err(|e| format!("Failed to read HTML: {}", e))?;
    html2text::from_read(&html[..], 120)
        .map_err(|e| format!("Failed to convert HTML to text: {}", e))
}

/// Extract text locally from a plain text file
fn extract_text_content(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))
}

/// Attempt local text extraction for any supported format
fn extract_local_text(path: &Path, format: &str) -> Result<String, String> {
    match format {
        "pdf" => extract_pdf_text(path),
        "epub" => extract_epub_text(path),
        "mobi" => extract_mobi_text(path),
        "djvu" => extract_djvu_text(path),
        "html" | "htm" => extract_html_text(path),
        "txt" | "md" => extract_text_content(path),
        // CBZ/CBR are image-only — Claude vision or store as-is
        _ => Err(format!("No local text extractor for .{} files", format)),
    }
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
    let local_text = extract_local_text(&stored_path, &format);

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
