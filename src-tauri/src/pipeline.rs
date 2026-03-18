use crate::claude::ClaudeClient;
use crate::config::ConfigManager;
use crate::db::Database;
use crate::embeddings::{self, EmbeddingEngine};
use crate::storage::StorageLayout;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::sync::OnceCell;

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

/// Dispatch metadata enrichment to the configured AI provider.
async fn enrich_metadata_dispatch(
    text: &str,
    config: &Arc<Mutex<ConfigManager>>,
) -> Result<crate::claude::MetadataResult, String> {
    let cfg = config.lock().unwrap().load();
    if cfg.ai_provider == "ollama" && !cfg.ollama_model.is_empty() {
        let ollama = crate::ollama::OllamaClient::new();
        ollama.enrich_metadata(text, &cfg.ollama_base_url, &cfg.ollama_model).await
    } else {
        let api_key = cfg.anthropic_api_key.unwrap_or_default();
        if api_key.is_empty() {
            return Err("No API key configured".to_string());
        }
        let claude = ClaudeClient::new();
        claude.enrich_metadata(text, &api_key, &cfg.model).await
    }
}

/// Extract text locally from a PDF using pdf-extract
fn extract_pdf_text(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read PDF: {}", e))?;
    // pdf-extract can panic on malformed PDFs — catch it
    std::panic::catch_unwind(|| {
        pdf_extract::extract_text_from_mem(&bytes)
    })
    .map_err(|_| "PDF text extraction panicked (malformed PDF)".to_string())?
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

/// Extract cover/thumbnail from a PDF using macOS Quick Look
fn extract_pdf_cover(path: &Path) -> Option<Vec<u8>> {
    let tmp_dir = std::env::temp_dir().join("archivum_covers");
    std::fs::create_dir_all(&tmp_dir).ok()?;

    let output = std::process::Command::new("qlmanage")
        .args(["-t", "-s", "480", "-o"])
        .arg(&tmp_dir)
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    // qlmanage outputs <output_dir>/<filename>.png
    let filename = path.file_name()?.to_string_lossy();
    let thumbnail_path = tmp_dir.join(format!("{}.png", filename));

    let data = std::fs::read(&thumbnail_path).ok()?;
    let _ = std::fs::remove_file(&thumbnail_path);
    Some(data)
}

/// Extract cover image from an EPUB file
fn extract_epub_cover(path: &Path) -> Option<Vec<u8>> {
    let file = std::fs::File::open(path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    // Find OPF path from container.xml
    let opf_path = {
        let mut container = archive.by_name("META-INF/container.xml").ok()?;
        let mut xml = String::new();
        container.read_to_string(&mut xml).ok()?;
        let marker = "full-path=\"";
        let start = xml.find(marker)? + marker.len();
        let end = xml[start..].find('"')? + start;
        xml[start..end].to_string()
    };

    let opf_dir = opf_path
        .rfind('/')
        .map(|i| opf_path[..i + 1].to_string())
        .unwrap_or_default();

    // Parse OPF to find cover image href
    let cover_href = {
        let mut opf_entry = archive.by_name(&opf_path).ok()?;
        let mut xml = String::new();
        opf_entry.read_to_string(&mut xml).ok()?;

        // Method 1: <item properties="cover-image" href="..."/>
        let by_prop = xml.match_indices("<item ").find_map(|(i, _)| {
            let chunk = &xml[i..];
            let end = chunk.find("/>").or_else(|| chunk.find("</item>"))?;
            let tag = &chunk[..end];
            if tag.contains("cover-image") {
                extract_attr(tag, "href")
            } else {
                None
            }
        });

        if by_prop.is_some() {
            by_prop
        } else {
            // Method 2: <meta name="cover" content="item-id"/> → resolve item id to href
            let cover_id = xml.match_indices("<meta ").find_map(|(i, _)| {
                let chunk = &xml[i..];
                let end = chunk.find("/>").or_else(|| chunk.find("</meta>"))?;
                let tag = &chunk[..end];
                if tag.contains("name=\"cover\"") {
                    extract_attr(tag, "content")
                } else {
                    None
                }
            })?;

            xml.match_indices("<item ").find_map(|(i, _)| {
                let chunk = &xml[i..];
                let end = chunk.find("/>").or_else(|| chunk.find("</item>"))?;
                let tag = &chunk[..end];
                if extract_attr(tag, "id").as_deref() == Some(&cover_id) {
                    extract_attr(tag, "href")
                } else {
                    None
                }
            })
        }
    }?;

    let full_path = format!("{}{}", opf_dir, cover_href);
    let mut entry = archive.by_name(&full_path).ok()?;
    let mut data = Vec::new();
    entry.read_to_end(&mut data).ok()?;
    Some(data)
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
    embeddings: &Arc<OnceCell<EmbeddingEngine>>,
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

    // 6b. Extract cover image if available
    let cover_path = match format.as_str() {
        "epub" => extract_epub_cover(&stored_path),
        "pdf" => extract_pdf_cover(&stored_path),
        _ => None,
    }
    .and_then(|data| storage.write_cover(doc_id, &data).ok());

    // 6c. Extract table of contents if available
    let toc_entries = match format.as_str() {
        "epub" => {
            let toc = extract_epub_toc(&stored_path);
            if toc.is_empty() { None } else { Some(toc) }
        }
        _ => None,
    };

    // 7. Determine AI enrichment strategy based on what we have
    let has_ai = {
        let cfg = config.lock().unwrap().load();
        if cfg.ai_provider == "ollama" {
            !cfg.ollama_model.is_empty()
        } else {
            cfg.anthropic_api_key.as_ref().is_some_and(|k| !k.is_empty())
        }
    };

    // Resolve embedding engine (lazy init)
    let data_dir = storage.originals_dir().parent().unwrap().to_path_buf();
    let embed_engine = embeddings
        .get_or_try_init(|| async {
            EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .ok();
    let has_local_text = local_text.as_ref().is_ok_and(|t| !t.trim().is_empty());

    if has_local_text {
        // Local extraction succeeded — use local content, enrich metadata with Claude
        let content = local_text.unwrap();

        if has_ai {
            {
                let db_lock = db.lock().unwrap();
                db_lock
                    .update_task(
                        task_id,
                        "running",
                        0.6,
                        Some("Enriching metadata..."),
                        None,
                    )
                    .map_err(|e| e.to_string())?;
            }

            match enrich_metadata_dispatch(&content, config).await {
                Ok(meta) => {
                    return finish_import(
                        db, storage, task_id, doc_id,
                        &meta.title, &meta.author,
                        meta.description.as_deref(), meta.language.as_deref(),
                        meta.isbn.as_deref(), meta.publisher.as_deref(),
                        meta.published_date.as_deref(), meta.page_count,
                        &content, &meta.tags, cover_path.as_deref(), None,
                        embed_engine, toc_entries,
                    );
                }
                Err(e) => {
                    eprintln!("[pipeline] metadata enrichment failed: {}", e);
                    // Fall through — use local text with filename as title
                }
            }
        }

        return finish_import(
            db, storage, task_id, doc_id,
            &fallback_title, "Unknown",
            None, None, None, None, None, None,
            &content, &[], cover_path.as_deref(), None,
            embed_engine, toc_entries,
        );
    }

    // 8. No local text — try full Claude extraction (for image-based formats like CBZ/CBR)
    //    Only Claude can do document extraction (requires vision/file upload)
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
                    db, storage, task_id, doc_id,
                    &result.title, &result.author,
                    result.description.as_deref(), result.language.as_deref(),
                    result.isbn.as_deref(), result.publisher.as_deref(),
                    result.published_date.as_deref(), result.page_count,
                    &result.markdown_content, &result.tags, cover_path.as_deref(), None,
                    embed_engine, toc_entries,
                );
            }
            Err(e) => {
                eprintln!("[pipeline] Claude extraction failed: {}", e);
            }
        }
    }

    // 9. Nothing worked — store with filename only
    let db_lock = db.lock().unwrap();
    db_lock
        .update_document_metadata(
            doc_id, &fallback_title, "Unknown",
            None, None, None, None, None, None, None, None, "complete",
        )
        .map_err(|e| e.to_string())?;
    db_lock
        .update_task(task_id, "complete", 1.0, Some("Imported (no text extracted)"), None)
        .map_err(|e| e.to_string())?;
    Ok(doc_id.to_string())
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
    cover_path: Option<&str>,
    task_note: Option<&str>,
    embeddings: Option<&EmbeddingEngine>,
    toc_entries: Option<Vec<crate::db::TocEntry>>,
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
            cover_path,
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

    // Store tags in the relational tables
    if !tags.is_empty() {
        if let Err(e) = db_lock.set_document_tags(doc_id, tags) {
            eprintln!("[pipeline] tag storage failed: {}", e);
        }
    }

    // Embed chunks for RAG (non-fatal)
    if let Some(engine) = embeddings {
        let chunks = embeddings::chunk_markdown(markdown_content);
        if !chunks.is_empty() {
            match engine.embed_chunks(&chunks) {
                Ok(chunk_embeddings) => {
                    if let Err(e) = db_lock.insert_chunks(doc_id, &chunks, &chunk_embeddings) {
                        eprintln!("[pipeline] chunk storage failed: {}", e);
                    }
                }
                Err(e) => eprintln!("[pipeline] embedding failed: {}", e),
            }
        }
    }

    // Store table of contents (non-fatal)
    let toc = toc_entries.unwrap_or_else(|| parse_markdown_toc(markdown_content));
    if !toc.is_empty() {
        if let Err(e) = db_lock.insert_toc(doc_id, &toc) {
            eprintln!("[pipeline] TOC storage failed: {}", e);
        }
    }

    let message = task_note.unwrap_or("Import complete");
    db_lock
        .update_task(task_id, "complete", 1.0, Some(message), None)
        .map_err(|e| e.to_string())?;

    Ok(doc_id.to_string())
}

/// Parse markdown headings into a TOC structure.
pub fn parse_markdown_toc(markdown: &str) -> Vec<crate::db::TocEntry> {
    let mut entries = Vec::new();
    for line in markdown.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('#') {
            continue;
        }
        let level = trimmed.chars().take_while(|&c| c == '#').count();
        if level > 6 {
            continue;
        }
        let title = trimmed[level..].trim().trim_start_matches('#').trim();
        if title.is_empty() {
            continue;
        }
        // Create a simple slug for the href
        let slug: String = title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        entries.push(crate::db::TocEntry {
            title: title.to_string(),
            level: level as i32,
            href: Some(slug),
        });
    }
    entries
}

/// Extract table of contents from an EPUB file's NCX or nav document.
pub fn extract_epub_toc(path: &Path) -> Vec<crate::db::TocEntry> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return Vec::new(),
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };

    // Find OPF path
    let opf_path = (|| -> Option<String> {
        let mut container = archive.by_name("META-INF/container.xml").ok()?;
        let mut xml = String::new();
        container.read_to_string(&mut xml).ok()?;
        let marker = "full-path=\"";
        let start = xml.find(marker)? + marker.len();
        let end = xml[start..].find('"')? + start;
        Some(xml[start..end].to_string())
    })();

    let opf_path = match opf_path {
        Some(p) => p,
        None => return Vec::new(),
    };

    let opf_dir = opf_path
        .rfind('/')
        .map(|i| opf_path[..i + 1].to_string())
        .unwrap_or_default();

    // Read OPF to find the NCX file
    let ncx_href = (|| -> Option<String> {
        let mut opf_entry = archive.by_name(&opf_path).ok()?;
        let mut xml = String::new();
        opf_entry.read_to_string(&mut xml).ok()?;

        // Look for NCX item in manifest
        for item_start in xml.match_indices("<item ").map(|(i, _)| i) {
            let chunk = &xml[item_start..];
            let end = chunk.find("/>").or_else(|| chunk.find("</item>"))?;
            let tag = &chunk[..end];
            let media = extract_attr(tag, "media-type").unwrap_or_default();
            if media == "application/x-dtbncx+xml" {
                return extract_attr(tag, "href");
            }
        }
        None
    })();

    if let Some(href) = ncx_href {
        let ncx_path = format!("{}{}", opf_dir, href);
        if let Ok(mut entry) = archive.by_name(&ncx_path) {
            let mut xml = String::new();
            if entry.read_to_string(&mut xml).is_ok() {
                return parse_ncx_toc(&xml);
            }
        }
    }

    Vec::new()
}

/// Parse NCX XML to extract navPoint entries as a flat TOC.
fn parse_ncx_toc(xml: &str) -> Vec<crate::db::TocEntry> {
    let mut entries = Vec::new();
    parse_navpoints(xml, 1, &mut entries);
    entries
}

fn parse_navpoints(xml: &str, level: i32, entries: &mut Vec<crate::db::TocEntry>) {
    let mut search_from = 0;
    while let Some(start) = xml[search_from..].find("<navPoint") {
        let abs_start = search_from + start;
        // Find the text label
        let chunk = &xml[abs_start..];

        // Find navLabel > text
        let label = (|| -> Option<String> {
            let label_start = chunk.find("<navLabel")? + 9;
            let text_start = chunk[label_start..].find("<text")? + label_start;
            let text_chunk = &chunk[text_start..];
            let content_start = text_chunk.find('>')? + 1;
            let content_end = text_chunk.find("</text>")?;
            let text = text_chunk[content_start..content_end].trim().to_string();
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        })();

        // Find content src
        let href = (|| -> Option<String> {
            let content_start = chunk.find("<content")?.min(
                chunk
                    .find("</navPoint>")
                    .unwrap_or(chunk.len()),
            );
            let tag = &chunk[content_start..];
            let end = tag.find("/>")?;
            extract_attr(&tag[..end], "src")
        })();

        if let Some(title) = label {
            entries.push(crate::db::TocEntry {
                title,
                level,
                href,
            });
        }

        // Move past this navPoint opening tag to find the next one
        search_from = abs_start + "<navPoint".len();
    }
}

/// Import a local audio or video file: convert → transcribe → store
pub async fn import_media_file(
    db: &Arc<Mutex<Database>>,
    storage: &StorageLayout,
    _claude: &ClaudeClient,
    config: &Arc<Mutex<ConfigManager>>,
    source_path: &Path,
    task_id: &str,
    doc_id: &str,
    embeddings: &Arc<OnceCell<EmbeddingEngine>>,
) -> Result<String, String> {
    let format = StorageLayout::detect_format(source_path)
        .ok_or_else(|| "Could not detect file format".to_string())?;

    // 1. Hash original file
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.05, Some("Hashing file..."), None)
            .map_err(|e| e.to_string())?;
    }

    let file_hash = StorageLayout::compute_hash(source_path)
        .map_err(|e| format!("Failed to hash file: {}", e))?;

    {
        let db_lock = db.lock().unwrap();
        if db_lock.has_hash(&file_hash).map_err(|e| e.to_string())? {
            db_lock.update_task(task_id, "complete", 1.0, Some("Duplicate — already in library"), None)
                .map_err(|e| e.to_string())?;
            return Err("File already exists in library".to_string());
        }
    }

    // 2. Store original
    let file_size = StorageLayout::file_size(source_path)
        .map_err(|e| format!("Failed to get file size: {}", e))? as i64;
    let original_path = storage.store_original(source_path, &file_hash, &format)
        .map_err(|e| format!("Failed to store file: {}", e))?;

    {
        let db_lock = db.lock().unwrap();
        db_lock.insert_document(doc_id, &format, &file_hash, file_size, &original_path)
            .map_err(|e| format!("Failed to create document: {}", e))?;
    }

    // 3. Get duration
    let duration = crate::whisper::get_duration(source_path);

    // 4. Convert to WAV
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.15, Some("Converting audio..."), None)
            .map_err(|e| e.to_string())?;
    }

    let temp_dir = storage.originals_dir().parent().unwrap().join("temp");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
    let wav_path = temp_dir.join(format!("{}.wav", doc_id));

    // If already WAV at 16kHz mono, we might still need to convert
    crate::whisper::convert_to_wav(source_path, &wav_path)?;

    // 5. Transcribe
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.3, Some("Transcribing with Whisper..."), None)
            .map_err(|e| e.to_string())?;
    }

    let (model_path, model_name) = get_whisper_model(config, storage)?;
    let transcript = crate::whisper::transcribe(&wav_path, &model_path)?;

    // Clean up temp WAV
    let _ = std::fs::remove_file(&wav_path);

    // 6. Format as markdown
    let fallback_title = title_from_filename(source_path);
    let markdown = crate::whisper::format_transcript_markdown(
        &transcript,
        &fallback_title,
        None,
        duration,
        &model_name,
    );

    // 7. Update media metadata
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_document_media_metadata(doc_id, duration, None)
            .map_err(|e| e.to_string())?;
    }

    // 8. Enrich metadata and finish
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.7, Some("Enriching metadata..."), None)
            .map_err(|e| e.to_string())?;
    }

    let data_dir = storage.originals_dir().parent().unwrap().to_path_buf();
    let embed_engine = embeddings
        .get_or_try_init(|| async { EmbeddingEngine::new(&data_dir.join("models")) })
        .await
        .ok();

    if let Ok(meta) = enrich_metadata_dispatch(&markdown, config).await {
        return finish_import(
            db, storage, task_id, doc_id,
            &meta.title, &meta.author,
            meta.description.as_deref(), meta.language.as_deref(),
            meta.isbn.as_deref(), meta.publisher.as_deref(),
            meta.published_date.as_deref(), meta.page_count,
            &markdown, &meta.tags, None,
            Some("Transcription complete"),
            embed_engine, None,
        );
    }

    finish_import(
        db, storage, task_id, doc_id,
        &fallback_title, "Unknown",
        None, None, None, None, None, None,
        &markdown, &[], None,
        Some("Transcription complete"),
        embed_engine, None,
    )
}

/// Import a YouTube video: download audio → transcribe → store
pub async fn import_youtube(
    db: &Arc<Mutex<Database>>,
    storage: &StorageLayout,
    _claude: &ClaudeClient,
    config: &Arc<Mutex<ConfigManager>>,
    url: &str,
    task_id: &str,
    doc_id: &str,
    embeddings: &Arc<OnceCell<EmbeddingEngine>>,
) -> Result<String, String> {
    // 1. Download audio with yt-dlp
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.1, Some("Downloading audio from YouTube..."), None)
            .map_err(|e| e.to_string())?;
    }

    let temp_dir = storage.originals_dir().parent().unwrap().join("temp").join(doc_id);
    let yt_result = crate::whisper::download_youtube_audio(url, &temp_dir)?;

    // 2. Hash the downloaded audio
    let file_hash = StorageLayout::compute_hash(&yt_result.audio_path)
        .map_err(|e| format!("Failed to hash audio: {}", e))?;

    {
        let db_lock = db.lock().unwrap();
        if db_lock.has_hash(&file_hash).map_err(|e| e.to_string())? {
            let _ = std::fs::remove_dir_all(&temp_dir);
            db_lock.update_task(task_id, "complete", 1.0, Some("Duplicate — already in library"), None)
                .map_err(|e| e.to_string())?;
            return Err("Audio already exists in library".to_string());
        }
    }

    // 3. Store the audio file
    let file_size = StorageLayout::file_size(&yt_result.audio_path)
        .map_err(|e| format!("Failed to get file size: {}", e))? as i64;
    let original_path = storage.store_original(&yt_result.audio_path, &file_hash, "wav")
        .map_err(|e| format!("Failed to store audio: {}", e))?;

    {
        let db_lock = db.lock().unwrap();
        db_lock.insert_document(doc_id, "youtube", &file_hash, file_size, &original_path)
            .map_err(|e| format!("Failed to create document: {}", e))?;
    }

    // 4. Convert to 16kHz mono WAV if needed
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.3, Some("Preparing audio..."), None)
            .map_err(|e| e.to_string())?;
    }

    let wav_path = temp_dir.join("transcribe.wav");
    crate::whisper::convert_to_wav(&yt_result.audio_path, &wav_path)?;

    // 5. Transcribe
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.4, Some("Transcribing with Whisper..."), None)
            .map_err(|e| e.to_string())?;
    }

    let (model_path, model_name) = get_whisper_model(config, storage)?;
    let transcript = crate::whisper::transcribe(&wav_path, &model_path)?;

    // Clean up temp dir
    let _ = std::fs::remove_dir_all(&temp_dir);

    // 6. Format as markdown
    let markdown = crate::whisper::format_transcript_markdown(
        &transcript,
        &yt_result.title,
        Some(url),
        Some(yt_result.duration),
        &model_name,
    );

    // 7. Update media metadata
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_document_media_metadata(doc_id, Some(yt_result.duration), Some(url))
            .map_err(|e| e.to_string())?;
    }

    // 8. Finish with metadata enrichment
    {
        let db_lock = db.lock().unwrap();
        db_lock.update_task(task_id, "running", 0.8, Some("Enriching metadata..."), None)
            .map_err(|e| e.to_string())?;
    }

    let data_dir = storage.originals_dir().parent().unwrap().to_path_buf();
    let embed_engine = embeddings
        .get_or_try_init(|| async { EmbeddingEngine::new(&data_dir.join("models")) })
        .await
        .ok();

    if let Ok(meta) = enrich_metadata_dispatch(&markdown, config).await {
        return finish_import(
            db, storage, task_id, doc_id,
            &meta.title, &meta.author,
            meta.description.as_deref(), meta.language.as_deref(),
            meta.isbn.as_deref(), meta.publisher.as_deref(),
            meta.published_date.as_deref(), meta.page_count,
            &markdown, &meta.tags, None,
            Some("YouTube transcription complete"),
            embed_engine, None,
        );
    }

    finish_import(
        db, storage, task_id, doc_id,
        &yt_result.title, &yt_result.uploader,
        Some(&yt_result.description), None, None, None, None, None,
        &markdown, &[], None,
        Some("YouTube transcription complete"),
        embed_engine, None,
    )
}

/// Look up the selected whisper model from config and return its path + name
fn get_whisper_model(
    config: &Arc<Mutex<ConfigManager>>,
    storage: &StorageLayout,
) -> Result<(std::path::PathBuf, String), String> {
    let cfg = config.lock().unwrap().load();
    let model_id = cfg.selected_whisper_model
        .ok_or_else(|| "No whisper model selected. Go to Settings to download and select a model.".to_string())?;

    let model_info = crate::whisper::WHISPER_MODELS
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("Unknown whisper model: {}", model_id))?;

    let originals = storage.originals_dir();
    let data_dir = originals.parent().unwrap();
    let path = crate::whisper::model_path(data_dir, model_info.filename);

    if !path.exists() {
        return Err(format!("Model {} not downloaded. Go to Settings to download it.", model_info.name));
    }

    Ok((path, model_info.name.to_string()))
}

/// Public wrappers for cover extraction (used by regenerate_covers command)
pub fn extract_epub_cover_public(path: &Path) -> Option<Vec<u8>> {
    extract_epub_cover(path)
}

/// Public wrapper for local text extraction (used by reembed_document fallback)
pub fn extract_local_text_public(path: &Path, format: &str) -> Result<String, String> {
    extract_local_text(path, format)
}

pub fn extract_pdf_cover_public(path: &Path) -> Option<Vec<u8>> {
    extract_pdf_cover(path)
}

/// Import a webpage via Cloudflare Browser Rendering /crawl endpoint
pub async fn import_from_url(
    db: &Arc<Mutex<Database>>,
    storage: &StorageLayout,
    _claude: &ClaudeClient,
    config: &Arc<Mutex<ConfigManager>>,
    url: &str,
    account_id: &str,
    api_token: &str,
    task_id: &str,
    doc_id: &str,
    embeddings: &Arc<OnceCell<EmbeddingEngine>>,
) -> Result<String, String> {
    // 1. Start crawl job
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.1, Some("Starting webpage crawl..."), None)
            .map_err(|e| e.to_string())?;
    }

    let client = reqwest::Client::new();
    let crawl_url = format!(
        "https://api.cloudflare.com/client/v4/accounts/{}/browser-rendering/crawl",
        account_id
    );

    let resp = client
        .post(&crawl_url)
        .header("Authorization", format!("Bearer {}", api_token))
        .json(&serde_json::json!({
            "url": url,
            "limit": 1,
            "formats": ["markdown"],
            "render": true
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to start crawl: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let db_lock = db.lock().unwrap();
        let _ = db_lock.update_task(task_id, "failed", 0.0, None, Some(&format!("Crawl failed ({}): {}", status, body)));
        return Err(format!("Cloudflare crawl failed ({}): {}", status, body));
    }

    let init: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse crawl response: {}", e))?;

    let job_id = init["result"]
        .as_str()
        .ok_or_else(|| "No job ID in crawl response".to_string())?
        .to_string();

    // 2. Poll for results
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.3, Some("Waiting for crawl results..."), None)
            .map_err(|e| e.to_string())?;
    }

    let poll_url = format!("{}/{}", crawl_url, job_id);
    let mut markdown = String::new();
    let mut page_title = String::new();

    for attempt in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;

        let poll_resp = client
            .get(&poll_url)
            .header("Authorization", format!("Bearer {}", api_token))
            .send()
            .await
            .map_err(|e| format!("Failed to poll crawl: {}", e))?;

        let data: serde_json::Value = poll_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse poll response: {}", e))?;

        let status = data["result"]["status"].as_str().unwrap_or("");

        match status {
            "completed" => {
                if let Some(records) = data["result"]["records"].as_array() {
                    if let Some(record) = records.first() {
                        markdown = record["markdown"].as_str().unwrap_or("").to_string();
                        page_title = record["metadata"]["title"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                    }
                }
                break;
            }
            "errored" | "cancelled_due_to_timeout" | "cancelled_due_to_limits" | "cancelled_by_user" => {
                let db_lock = db.lock().unwrap();
                let _ = db_lock.update_task(task_id, "failed", 0.0, None, Some(&format!("Crawl {}", status)));
                return Err(format!("Crawl {}", status));
            }
            _ => {
                // Still running
                let progress = 0.3 + (attempt as f64 / 60.0) * 0.3;
                let db_lock = db.lock().unwrap();
                let _ = db_lock.update_task(task_id, "running", progress, Some("Crawling webpage..."), None);
            }
        }
    }

    if markdown.is_empty() {
        let db_lock = db.lock().unwrap();
        let _ = db_lock.update_task(task_id, "failed", 0.0, None, Some("Crawl timed out or returned no content"));
        return Err("Crawl returned no markdown content".to_string());
    }

    // 3. Create document record
    {
        let db_lock = db.lock().unwrap();
        db_lock
            .update_task(task_id, "running", 0.7, Some("Storing content..."), None)
            .map_err(|e| e.to_string())?;
    }

    let hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        format!("{:x}", hasher.finalize())
    };

    let file_size = markdown.len() as i64;
    let title = if page_title.is_empty() { url.to_string() } else { page_title };

    {
        let db_lock = db.lock().unwrap();
        db_lock
            .insert_document(doc_id, "url", &hash, file_size, url)
            .map_err(|e| format!("Failed to create document: {}", e))?;
    }

    // 4. Optionally enrich metadata with AI
    // Resolve embedding engine
    let data_dir = storage.originals_dir().parent().unwrap().to_path_buf();
    let embed_engine = embeddings
        .get_or_try_init(|| async {
            EmbeddingEngine::new(&data_dir.join("models"))
        })
        .await
        .ok();

    {
        {
            let db_lock = db.lock().unwrap();
            db_lock
                .update_task(task_id, "running", 0.8, Some("Enriching metadata..."), None)
                .map_err(|e| e.to_string())?;
        }

        match enrich_metadata_dispatch(&markdown, config).await {
            Ok(meta) => {
                return finish_import(
                    db, storage, task_id, doc_id,
                    &meta.title, &meta.author,
                    meta.description.as_deref(), meta.language.as_deref(),
                    meta.isbn.as_deref(), meta.publisher.as_deref(),
                    meta.published_date.as_deref(), meta.page_count,
                    &markdown, &meta.tags, None,
                    Some("Webpage imported"),
                    embed_engine, None,
                );
            }
            Err(e) => {
                eprintln!("[pipeline] metadata enrichment failed for URL: {}", e);
            }
        }
    }

    // 5. Fallback — store with page title only
    finish_import(
        db, storage, task_id, doc_id,
        &title, "Unknown",
        None, None, None, None, None, None,
        &markdown, &[], None,
        Some("Webpage imported"),
        embed_engine, None,
    )
}

/// Generate section summaries for a document after import.
/// Groups chunks into sections of 4 and summarizes each with AI.
pub async fn generate_section_summaries_async(
    db: &Arc<Mutex<Database>>,
    config: &Arc<Mutex<ConfigManager>>,
    doc_id: &str,
) -> Result<(), String> {
    let chunks = {
        let db_lock = db.lock().unwrap();
        // Skip if already has section summaries
        if db_lock.has_section_summaries(doc_id).unwrap_or(false) {
            return Ok(());
        }
        db_lock.get_document_chunks(doc_id).map_err(|e| e.to_string())?
    };

    if chunks.len() < 2 {
        return Ok(()); // Too few chunks to bother
    }

    let cfg = config.lock().unwrap().load();
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
            let api_key = cfg.anthropic_api_key.as_deref().unwrap_or("");
            if api_key.is_empty() {
                return Ok(()); // No API key, skip silently
            }
            let claude = ClaudeClient::new();
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
                eprintln!("[pipeline] section summary failed for chunks {}-{}: {}", start, end, e);
            }
        }
    }

    if !sections.is_empty() {
        let db_lock = db.lock().unwrap();
        let section_refs: Vec<(i32, i32, Option<&str>, &str, Option<&str>)> = sections.iter()
            .map(|(s, e, t, sum, c)| (*s, *e, t.as_deref(), sum.as_str(), c.as_deref()))
            .collect();
        db_lock.insert_section_summaries(doc_id, &section_refs)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}
