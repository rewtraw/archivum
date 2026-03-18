use rusqlite::{Connection, Result, params};
use serde::{Deserialize, Serialize};

pub struct Database {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub author: String,
    pub isbn: Option<String>,
    pub language: Option<String>,
    pub publisher: Option<String>,
    pub published_date: Option<String>,
    pub description: Option<String>,
    pub page_count: Option<i32>,
    pub original_format: String,
    pub file_hash: String,
    pub file_size: i64,
    pub original_path: String,
    pub markdown_path: Option<String>,
    pub cover_path: Option<String>,
    pub status: String,
    pub error_message: Option<String>,
    pub imported_at: String,
    pub updated_at: String,
    pub processed_at: Option<String>,
    pub reading_status: Option<String>,
    pub reading_progress: Option<f64>,
    pub last_read_at: Option<String>,
    pub tags: Vec<String>,
    pub duration_seconds: Option<f64>,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub author: String,
    pub original_format: String,
    pub snippet: String,
    pub cover_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub document_id: Option<String>,
    pub task_type: String,
    pub status: String,
    pub progress: f64,
    pub message: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryStats {
    pub total_documents: i64,
    pub total_size_bytes: i64,
    pub format_counts: Vec<(String, i64)>,
    pub status_counts: Vec<(String, i64)>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkSearchResult {
    pub content: String,
    pub chunk_index: usize,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LibraryChunkResult {
    pub document_id: String,
    pub document_title: String,
    pub content: String,
    pub chunk_index: usize,
    pub distance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub document_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub sources: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TocEntry {
    pub title: String,
    pub level: i32,
    pub href: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SectionSummary {
    pub id: i64,
    pub document_id: String,
    pub start_chunk: i32,
    pub end_chunk: i32,
    pub title: Option<String>,
    pub summary: String,
    pub key_concepts: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicCluster {
    pub id: String,
    pub label: String,
    pub summary: Option<String>,
    pub document_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrarySummary {
    pub summary: String,
    pub themes: Option<String>,
    pub document_count: i64,
    pub updated_at: String,
}

impl Database {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
        Ok(Self { conn })
    }

    pub fn initialize(&self) -> Result<()> {
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS documents (
                id              TEXT PRIMARY KEY,
                title           TEXT NOT NULL DEFAULT '',
                author          TEXT NOT NULL DEFAULT '',
                isbn            TEXT,
                language        TEXT,
                publisher       TEXT,
                published_date  TEXT,
                description     TEXT,
                page_count      INTEGER,
                original_format TEXT NOT NULL,
                file_hash       TEXT NOT NULL UNIQUE,
                file_size       INTEGER NOT NULL,
                original_path   TEXT NOT NULL,
                markdown_path   TEXT,
                cover_path      TEXT,
                status          TEXT NOT NULL DEFAULT 'pending',
                error_message   TEXT,
                imported_at     TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
                processed_at    TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_documents_file_hash ON documents(file_hash);
            CREATE INDEX IF NOT EXISTS idx_documents_status ON documents(status);
            CREATE INDEX IF NOT EXISTS idx_documents_imported_at ON documents(imported_at);

            CREATE TABLE IF NOT EXISTS tags (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE COLLATE NOCASE
            );

            CREATE TABLE IF NOT EXISTS document_tags (
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                tag_id      INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
                PRIMARY KEY (document_id, tag_id)
            );

            CREATE TABLE IF NOT EXISTS document_sources (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                source_type TEXT NOT NULL,
                source_uri  TEXT,
                imported_at TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
                title,
                author,
                description,
                content,
                tags,
                content='documents_fts_content',
                content_rowid='rowid',
                tokenize='porter unicode61'
            );

            CREATE TABLE IF NOT EXISTS documents_fts_content (
                rowid       INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL UNIQUE REFERENCES documents(id) ON DELETE CASCADE,
                title       TEXT NOT NULL DEFAULT '',
                author      TEXT NOT NULL DEFAULT '',
                description TEXT NOT NULL DEFAULT '',
                content     TEXT NOT NULL DEFAULT '',
                tags        TEXT NOT NULL DEFAULT ''
            );

            CREATE TRIGGER IF NOT EXISTS documents_fts_ai AFTER INSERT ON documents_fts_content BEGIN
                INSERT INTO documents_fts(rowid, title, author, description, content, tags)
                VALUES (new.rowid, new.title, new.author, new.description, new.content, new.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_fts_ad AFTER DELETE ON documents_fts_content BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, title, author, description, content, tags)
                VALUES ('delete', old.rowid, old.title, old.author, old.description, old.content, old.tags);
            END;

            CREATE TRIGGER IF NOT EXISTS documents_fts_au AFTER UPDATE ON documents_fts_content BEGIN
                INSERT INTO documents_fts(documents_fts, rowid, title, author, description, content, tags)
                VALUES ('delete', old.rowid, old.title, old.author, old.description, old.content, old.tags);
                INSERT INTO documents_fts(rowid, title, author, description, content, tags)
                VALUES (new.rowid, new.title, new.author, new.description, new.content, new.tags);
            END;

            CREATE TABLE IF NOT EXISTS tasks (
                id           TEXT PRIMARY KEY,
                document_id  TEXT,
                task_type    TEXT NOT NULL,
                status       TEXT NOT NULL DEFAULT 'queued',
                progress     REAL DEFAULT 0.0,
                message      TEXT,
                error        TEXT,
                created_at   TEXT NOT NULL DEFAULT (datetime('now')),
                started_at   TEXT,
                completed_at TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);

            CREATE TABLE IF NOT EXISTS document_chunks (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                chunk_index INTEGER NOT NULL,
                byte_offset INTEGER NOT NULL,
                content     TEXT NOT NULL,
                UNIQUE(document_id, chunk_index)
            );

            CREATE INDEX IF NOT EXISTS idx_chunks_document ON document_chunks(document_id);

            CREATE VIRTUAL TABLE IF NOT EXISTS chunk_embeddings USING vec0(
                chunk_id INTEGER PRIMARY KEY,
                embedding float[384]
            );

            CREATE TABLE IF NOT EXISTS chat_sessions (
                id          TEXT PRIMARY KEY,
                title       TEXT NOT NULL DEFAULT 'New chat',
                document_id TEXT REFERENCES documents(id) ON DELETE CASCADE,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_chat_sessions_updated ON chat_sessions(updated_at DESC);

            CREATE TABLE IF NOT EXISTS chat_messages (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id  TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
                role        TEXT NOT NULL,
                content     TEXT NOT NULL,
                sources     TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE INDEX IF NOT EXISTS idx_chat_messages_session ON chat_messages(session_id, created_at);
            ",
        )?;
        // Add reading_status column if it doesn't exist (migration)
        let _ = self.conn.execute(
            "ALTER TABLE documents ADD COLUMN reading_status TEXT",
            [],
        );

        // Reading progress tracking
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS reading_progress (
                document_id TEXT PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
                scroll_position REAL NOT NULL DEFAULT 0.0,
                last_read_at    TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS document_toc (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                title       TEXT NOT NULL,
                level       INTEGER NOT NULL DEFAULT 1,
                href        TEXT,
                sort_order  INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_toc_document ON document_toc(document_id, sort_order);

            CREATE TABLE IF NOT EXISTS document_summaries (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                length      TEXT NOT NULL,
                content     TEXT NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(document_id, length)
            );

            CREATE TABLE IF NOT EXISTS whisper_models (
                id               TEXT PRIMARY KEY,
                name             TEXT NOT NULL,
                filename         TEXT NOT NULL,
                size_bytes       INTEGER NOT NULL,
                status           TEXT NOT NULL DEFAULT 'available',
                download_progress REAL DEFAULT 0.0,
                downloaded_at    TEXT,
                error            TEXT
            );
            ",
        )?;

        // Media-related columns on documents
        let _ = self.conn.execute(
            "ALTER TABLE documents ADD COLUMN duration_seconds REAL",
            [],
        );
        let _ = self.conn.execute(
            "ALTER TABLE documents ADD COLUMN source_url TEXT",
            [],
        );

        // Hierarchical summaries
        self.conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS section_summaries (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                start_chunk INTEGER NOT NULL,
                end_chunk   INTEGER NOT NULL,
                title       TEXT,
                summary     TEXT NOT NULL,
                key_concepts TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                UNIQUE(document_id, start_chunk, end_chunk)
            );
            CREATE INDEX IF NOT EXISTS idx_section_summaries_doc ON section_summaries(document_id);

            CREATE TABLE IF NOT EXISTS topic_clusters (
                id          TEXT PRIMARY KEY,
                label       TEXT NOT NULL,
                summary     TEXT,
                created_at  TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
            );

            CREATE TABLE IF NOT EXISTS topic_cluster_members (
                cluster_id  TEXT NOT NULL REFERENCES topic_clusters(id) ON DELETE CASCADE,
                document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
                PRIMARY KEY (cluster_id, document_id)
            );

            CREATE TABLE IF NOT EXISTS library_summary (
                id              INTEGER PRIMARY KEY CHECK (id = 1),
                summary         TEXT NOT NULL,
                themes          TEXT,
                document_count  INTEGER NOT NULL,
                updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
            );
            ",
        )?;

        Ok(())
    }

    pub fn insert_document(
        &self,
        id: &str,
        format: &str,
        file_hash: &str,
        file_size: i64,
        original_path: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO documents (id, original_format, file_hash, file_size, original_path)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, format, file_hash, file_size, original_path],
        )?;
        Ok(())
    }

    pub fn has_hash(&self, hash: &str) -> Result<bool> {
        // Only count completed documents as duplicates.
        // Delete any pending/failed documents with this hash so retries work.
        self.conn.execute(
            "DELETE FROM documents WHERE file_hash = ?1 AND status = 'pending'",
            params![hash],
        )?;
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE file_hash = ?1",
            params![hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn set_markdown_path(&self, id: &str, md_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET markdown_path = ?1 WHERE id = ?2",
            params![md_path, id],
        )?;
        Ok(())
    }

    pub fn update_document_metadata(
        &self,
        id: &str,
        title: &str,
        author: &str,
        description: Option<&str>,
        language: Option<&str>,
        isbn: Option<&str>,
        publisher: Option<&str>,
        published_date: Option<&str>,
        page_count: Option<i32>,
        markdown_path: Option<&str>,
        cover_path: Option<&str>,
        status: &str,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET
                title = ?2, author = ?3, description = ?4, language = ?5,
                isbn = ?6, publisher = ?7, published_date = ?8, page_count = ?9,
                markdown_path = ?10, cover_path = ?11, status = ?12,
                updated_at = datetime('now'), processed_at = datetime('now')
             WHERE id = ?1",
            params![
                id, title, author, description, language,
                isbn, publisher, published_date, page_count,
                markdown_path, cover_path, status,
            ],
        )?;
        Ok(())
    }

    pub fn set_document_error(&self, id: &str, error: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET status = 'error', error_message = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, error],
        )?;
        Ok(())
    }

    pub fn upsert_fts(
        &self,
        document_id: &str,
        title: &str,
        author: &str,
        description: &str,
        content: &str,
        tags: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO documents_fts_content (document_id, title, author, description, content, tags)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(document_id) DO UPDATE SET
                title = excluded.title, author = excluded.author,
                description = excluded.description, content = excluded.content,
                tags = excluded.tags",
            params![document_id, title, author, description, content, tags],
        )?;
        Ok(())
    }

    pub fn list_documents(
        &self,
        offset: i64,
        limit: i64,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
        format_filter: Option<&str>,
        status_filter: Option<&str>,
        tag_filter: Option<&str>,
    ) -> Result<Vec<Document>> {
        let order_col = match sort_by.unwrap_or("imported_at") {
            "title" => "d.title COLLATE NOCASE",
            "author" => "d.author COLLATE NOCASE",
            "file_size" => "d.file_size",
            "last_read" => "rp.last_read_at",
            _ => "d.imported_at",
        };
        let dir = if sort_dir.unwrap_or("desc") == "asc" { "ASC" } else { "DESC" };
        let nulls = if dir == "DESC" { "NULLS LAST" } else { "NULLS FIRST" };

        let mut sql = String::from(
            "SELECT d.*, rp.scroll_position, rp.last_read_at as rp_last_read,
                    GROUP_CONCAT(t.name, ', ') as tag_list
             FROM documents d
             LEFT JOIN reading_progress rp ON d.id = rp.document_id
             LEFT JOIN document_tags dt ON d.id = dt.document_id
             LEFT JOIN tags t ON dt.tag_id = t.id",
        );

        let mut conditions: Vec<String> = Vec::new();
        let mut param_values: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(fmt) = format_filter {
            if !fmt.is_empty() {
                param_values.push(Box::new(fmt.to_string()));
                conditions.push(format!("d.original_format = ?{}", param_values.len()));
            }
        }
        if let Some(status) = status_filter {
            if !status.is_empty() {
                param_values.push(Box::new(status.to_string()));
                conditions.push(format!("d.reading_status = ?{}", param_values.len()));
            }
        }
        if let Some(tag) = tag_filter {
            if !tag.is_empty() {
                param_values.push(Box::new(tag.to_string()));
                conditions.push(format!(
                    "d.id IN (SELECT dt2.document_id FROM document_tags dt2 JOIN tags t2 ON dt2.tag_id = t2.id WHERE t2.name = ?{})",
                    param_values.len()
                ));
            }
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" GROUP BY d.id");
        sql.push_str(&format!(" ORDER BY {} {} {}", order_col, dir, nulls));

        param_values.push(Box::new(limit));
        sql.push_str(&format!(" LIMIT ?{}", param_values.len()));
        param_values.push(Box::new(offset));
        sql.push_str(&format!(" OFFSET ?{}", param_values.len()));

        let mut stmt = self.conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> =
            param_values.iter().map(|p| p.as_ref()).collect();

        let docs = stmt
            .query_map(param_refs.as_slice(), |row| {
                let tag_str: Option<String> = row.get("tag_list")?;
                let tags = tag_str
                    .map(|s| s.split(", ").map(String::from).collect())
                    .unwrap_or_default();

                Ok(Document {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    author: row.get("author")?,
                    isbn: row.get("isbn")?,
                    language: row.get("language")?,
                    publisher: row.get("publisher")?,
                    published_date: row.get("published_date")?,
                    description: row.get("description")?,
                    page_count: row.get("page_count")?,
                    original_format: row.get("original_format")?,
                    file_hash: row.get("file_hash")?,
                    file_size: row.get("file_size")?,
                    original_path: row.get("original_path")?,
                    markdown_path: row.get("markdown_path")?,
                    cover_path: row.get("cover_path")?,
                    status: row.get("status")?,
                    error_message: row.get("error_message")?,
                    imported_at: row.get("imported_at")?,
                    updated_at: row.get("updated_at")?,
                    processed_at: row.get("processed_at")?,
                    reading_status: row.get("reading_status").unwrap_or(None),
                    reading_progress: row.get("scroll_position").unwrap_or(None),
                    last_read_at: row.get("rp_last_read").unwrap_or(None),
                    tags,
                    duration_seconds: row.get("duration_seconds").unwrap_or(None),
                    source_url: row.get("source_url").unwrap_or(None),
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(docs)
    }

    pub fn get_document(&self, id: &str) -> Result<Option<Document>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.*, rp.scroll_position, rp.last_read_at as rp_last_read,
                    GROUP_CONCAT(t.name, ', ') as tag_list
             FROM documents d
             LEFT JOIN reading_progress rp ON d.id = rp.document_id
             LEFT JOIN document_tags dt ON d.id = dt.document_id
             LEFT JOIN tags t ON dt.tag_id = t.id
             WHERE d.id = ?1
             GROUP BY d.id",
        )?;

        let mut rows = stmt.query_map(params![id], |row| {
            let tag_str: Option<String> = row.get("tag_list")?;
            let tags = tag_str
                .map(|s| s.split(", ").map(String::from).collect())
                .unwrap_or_default();

            Ok(Document {
                id: row.get("id")?,
                title: row.get("title")?,
                author: row.get("author")?,
                isbn: row.get("isbn")?,
                language: row.get("language")?,
                publisher: row.get("publisher")?,
                published_date: row.get("published_date")?,
                description: row.get("description")?,
                page_count: row.get("page_count")?,
                original_format: row.get("original_format")?,
                file_hash: row.get("file_hash")?,
                file_size: row.get("file_size")?,
                original_path: row.get("original_path")?,
                markdown_path: row.get("markdown_path")?,
                cover_path: row.get("cover_path")?,
                status: row.get("status")?,
                error_message: row.get("error_message")?,
                imported_at: row.get("imported_at")?,
                updated_at: row.get("updated_at")?,
                processed_at: row.get("processed_at")?,
                reading_status: row.get("reading_status").unwrap_or(None),
                reading_progress: row.get("scroll_position").unwrap_or(None),
                last_read_at: row.get("rp_last_read").unwrap_or(None),
                tags,
                duration_seconds: row.get("duration_seconds").unwrap_or(None),
                source_url: row.get("source_url").unwrap_or(None),
            })
        })?;

        match rows.next() {
            Some(doc) => Ok(Some(doc?)),
            None => Ok(None),
        }
    }

    pub fn delete_document(&self, id: &str) -> Result<Option<String>> {
        let path: Option<String> = self
            .conn
            .query_row(
                "SELECT original_path FROM documents WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .ok();

        // Clean up embeddings first (vec0 virtual table doesn't cascade)
        self.delete_document_chunks(id)?;

        self.conn
            .execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        Ok(path)
    }

    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
        // Build FTS query with prefix matching: each term gets a trailing *
        let fts_query = query
            .split_whitespace()
            .map(|term| {
                let escaped = term.replace('"', "\"\"");
                format!("\"{}\"*", escaped)
            })
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self.conn.prepare(
            "SELECT
                fc.document_id as id,
                d.title, d.author, d.original_format, d.cover_path,
                snippet(documents_fts, 3, '<mark>', '</mark>', '…', 40) as snippet
             FROM documents_fts
             JOIN documents_fts_content fc ON documents_fts.rowid = fc.rowid
             JOIN documents d ON fc.document_id = d.id
             WHERE documents_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![fts_query, limit], |row| {
                Ok(SearchResult {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    author: row.get("author")?,
                    original_format: row.get("original_format")?,
                    snippet: row.get("snippet")?,
                    cover_path: row.get("cover_path")?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        // LIKE fallback when FTS returns nothing
        if results.is_empty() {
            return self.search_like(query, limit);
        }

        Ok(results)
    }

    fn search_like(&self, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
        let pattern = format!("%{}%", query);
        let mut stmt = self.conn.prepare(
            "SELECT d.id, d.title, d.author, d.original_format, d.cover_path,
                    COALESCE(d.description, '') as snippet
             FROM documents d
             WHERE d.title LIKE ?1 OR d.author LIKE ?1 OR d.description LIKE ?1
             ORDER BY d.imported_at DESC
             LIMIT ?2",
        )?;

        let results = stmt
            .query_map(params![pattern, limit], |row| {
                Ok(SearchResult {
                    id: row.get("id")?,
                    title: row.get("title")?,
                    author: row.get("author")?,
                    original_format: row.get("original_format")?,
                    snippet: row.get("snippet")?,
                    cover_path: row.get("cover_path")?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(results)
    }

    pub fn get_stats(&self) -> Result<LibraryStats> {
        let total_documents: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM documents", [], |row| row.get(0))?;

        let total_size_bytes: i64 = self.conn.query_row(
            "SELECT COALESCE(SUM(file_size), 0) FROM documents",
            [],
            |row| row.get(0),
        )?;

        let mut fmt_stmt = self
            .conn
            .prepare("SELECT original_format, COUNT(*) FROM documents GROUP BY original_format")?;
        let format_counts = fmt_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, i64)>>>()?;

        let mut status_stmt = self
            .conn
            .prepare("SELECT status, COUNT(*) FROM documents GROUP BY status")?;
        let status_counts = status_stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<(String, i64)>>>()?;

        Ok(LibraryStats {
            total_documents,
            total_size_bytes,
            format_counts,
            status_counts,
        })
    }

    pub fn create_task(&self, id: &str, document_id: &str, task_type: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO tasks (id, document_id, task_type) VALUES (?1, ?2, ?3)",
            params![id, document_id, task_type],
        )?;
        Ok(())
    }

    pub fn update_task(
        &self,
        id: &str,
        status: &str,
        progress: f64,
        message: Option<&str>,
        error: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE tasks SET status = ?2, progress = ?3, message = ?4, error = ?5,
             started_at = CASE WHEN ?2 = 'running' AND started_at IS NULL THEN datetime('now') ELSE started_at END,
             completed_at = CASE WHEN ?2 IN ('complete', 'failed') THEN datetime('now') ELSE completed_at END
             WHERE id = ?1",
            params![id, status, progress, message, error],
        )?;
        Ok(())
    }

    pub fn list_tasks(&self, limit: i64) -> Result<Vec<Task>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, task_type, status, progress, message, error, created_at
             FROM tasks ORDER BY created_at DESC LIMIT ?1",
        )?;

        let tasks = stmt
            .query_map(params![limit], |row| {
                Ok(Task {
                    id: row.get("id")?,
                    document_id: row.get("document_id")?,
                    task_type: row.get("task_type")?,
                    status: row.get("status")?,
                    progress: row.get("progress")?,
                    message: row.get("message")?,
                    error: row.get("error")?,
                    created_at: row.get("created_at")?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(tasks)
    }

    pub fn delete_task(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn clear_finished_tasks(&self) -> Result<()> {
        self.conn.execute(
            "DELETE FROM tasks WHERE status IN ('complete', 'failed')",
            [],
        )?;
        Ok(())
    }

    pub fn insert_chunks(
        &self,
        document_id: &str,
        chunks: &[crate::embeddings::Chunk],
        embeddings: &[Vec<f32>],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;

        for (chunk, embedding) in chunks.iter().zip(embeddings.iter()) {
            tx.execute(
                "INSERT INTO document_chunks (document_id, chunk_index, byte_offset, content)
                 VALUES (?1, ?2, ?3, ?4)",
                params![document_id, chunk.index as i64, chunk.offset as i64, chunk.text],
            )?;

            let chunk_id = tx.last_insert_rowid();
            let embedding_bytes: Vec<u8> = embedding
                .iter()
                .flat_map(|f| f.to_le_bytes())
                .collect();

            tx.execute(
                "INSERT INTO chunk_embeddings (chunk_id, embedding) VALUES (?1, ?2)",
                params![chunk_id, embedding_bytes],
            )?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn search_chunks(
        &self,
        query_embedding: &[f32],
        document_id: &str,
        limit: usize,
    ) -> Result<Vec<ChunkSearchResult>> {
        let embedding_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // sqlite-vec MATCH returns the global top-k nearest neighbors.
        // We need to fetch more than `limit` to account for chunks from other
        // documents, then filter to the target document in the outer query.
        let total_chunks: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM chunk_embeddings",
            [],
            |row| row.get(0),
        )?;
        // Fetch up to 200 or total chunks (whichever is smaller) to ensure
        // we find enough results for this specific document.
        let k = std::cmp::min(total_chunks, 200);

        let mut stmt = self.conn.prepare(
            "SELECT dc.content, dc.chunk_index, ce.distance
             FROM chunk_embeddings ce
             JOIN document_chunks dc ON dc.id = ce.chunk_id
             WHERE ce.embedding MATCH ?1
               AND ce.k = ?2
               AND dc.document_id = ?3
             ORDER BY ce.distance
             LIMIT ?4",
        )?;

        let results = stmt
            .query_map(params![embedding_bytes, k, document_id, limit as i64], |row| {
                Ok(ChunkSearchResult {
                    content: row.get(0)?,
                    chunk_index: row.get::<_, i64>(1)? as usize,
                    distance: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(results)
    }

    pub fn has_chunks(&self, document_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM document_chunks WHERE document_id = ?1",
            params![document_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    pub fn delete_document_chunks(&self, document_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM chunk_embeddings WHERE chunk_id IN
             (SELECT id FROM document_chunks WHERE document_id = ?1)",
            params![document_id],
        )?;
        self.conn.execute(
            "DELETE FROM document_chunks WHERE document_id = ?1",
            params![document_id],
        )?;
        Ok(())
    }

    pub fn search_semantic(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let embedding_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        // Fetch a large candidate pool, then deduplicate by document
        let fetch_k = limit * 6;
        let mut stmt = self.conn.prepare(
            "SELECT dc.content, dc.chunk_index, ce.distance,
                    dc.document_id, d.title, d.author, d.original_format, d.cover_path
             FROM chunk_embeddings ce
             JOIN document_chunks dc ON dc.id = ce.chunk_id
             JOIN documents d ON d.id = dc.document_id
             WHERE ce.embedding MATCH ?1
               AND ce.k = ?2
             ORDER BY ce.distance",
        )?;

        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();

        let rows = stmt.query_map(params![embedding_bytes, fetch_k as i64], |row| {
            Ok((
                row.get::<_, String>(3)?,  // document_id
                row.get::<_, String>(4)?,  // title
                row.get::<_, String>(5)?,  // author
                row.get::<_, String>(6)?,  // original_format
                row.get::<_, Option<String>>(7)?, // cover_path
                row.get::<_, String>(0)?,  // chunk content (for snippet)
            ))
        })?;

        for row in rows {
            let (doc_id, title, author, format, cover_path, content) = row?;
            if seen.contains(&doc_id) {
                continue;
            }
            seen.insert(doc_id.clone());

            let snippet = if content.len() > 300 {
                format!("{}...", &content[..300])
            } else {
                content
            };

            results.push(SearchResult {
                id: doc_id,
                title,
                author,
                original_format: format,
                snippet,
                cover_path,
            });

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    pub fn search_semantic_excluding(
        &self,
        query_embedding: &[f32],
        exclude_document_id: &str,
        limit: usize,
    ) -> Result<Vec<SearchResult>> {
        let embedding_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        let fetch_k = (limit + 1) * 6;
        let mut stmt = self.conn.prepare(
            "SELECT dc.content, dc.chunk_index, ce.distance,
                    dc.document_id, d.title, d.author, d.original_format, d.cover_path
             FROM chunk_embeddings ce
             JOIN document_chunks dc ON dc.id = ce.chunk_id
             JOIN documents d ON d.id = dc.document_id
             WHERE ce.embedding MATCH ?1
               AND ce.k = ?2
             ORDER BY ce.distance",
        )?;

        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();

        let rows = stmt.query_map(params![embedding_bytes, fetch_k as i64], |row| {
            Ok((
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(0)?,
            ))
        })?;

        for row in rows {
            let (doc_id, title, author, format, cover_path, content) = row?;
            if doc_id == exclude_document_id || seen.contains(&doc_id) {
                continue;
            }
            seen.insert(doc_id.clone());

            let snippet = if content.len() > 300 {
                format!("{}...", &content[..300])
            } else {
                content
            };

            results.push(SearchResult {
                id: doc_id,
                title,
                author,
                original_format: format,
                snippet,
                cover_path,
            });

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    pub fn list_unembedded_document_ids(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.id FROM documents d
             WHERE d.status = 'complete'
               AND d.markdown_path IS NOT NULL
               AND NOT EXISTS (SELECT 1 FROM document_chunks dc WHERE dc.document_id = d.id)",
        )?;
        let ids = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        Ok(ids)
    }

    pub fn get_embedding_stats(&self) -> Result<(i64, i64)> {
        let total: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE status = 'complete' AND markdown_path IS NOT NULL",
            [],
            |row| row.get(0),
        )?;
        let embedded: i64 = self.conn.query_row(
            "SELECT COUNT(DISTINCT document_id) FROM document_chunks",
            [],
            |row| row.get(0),
        )?;
        Ok((total, embedded))
    }

    pub fn list_documents_without_covers(&self) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, original_format, original_path FROM documents
             WHERE cover_path IS NULL AND status = 'complete'",
        )?;
        let results = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    pub fn set_cover_path(&self, id: &str, cover_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET cover_path = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, cover_path],
        )?;
        Ok(())
    }

    pub fn update_reading_status(&self, id: &str, status: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET reading_status = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, status],
        )?;
        Ok(())
    }

    pub fn search_all_chunks(
        &self,
        query_embedding: &[f32],
        limit: usize,
    ) -> Result<Vec<LibraryChunkResult>> {
        let embedding_bytes: Vec<u8> = query_embedding
            .iter()
            .flat_map(|f| f.to_le_bytes())
            .collect();

        let mut stmt = self.conn.prepare(
            "SELECT dc.content, dc.chunk_index, ce.distance,
                    dc.document_id, d.title AS document_title
             FROM chunk_embeddings ce
             JOIN document_chunks dc ON dc.id = ce.chunk_id
             JOIN documents d ON d.id = dc.document_id
             WHERE ce.embedding MATCH ?1
               AND ce.k = ?2
             ORDER BY ce.distance",
        )?;

        let results = stmt
            .query_map(params![embedding_bytes, limit as i64], |row| {
                Ok(LibraryChunkResult {
                    content: row.get(0)?,
                    chunk_index: row.get::<_, i64>(1)? as usize,
                    distance: row.get(2)?,
                    document_id: row.get(3)?,
                    document_title: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(results)
    }

    pub fn create_chat_session(
        &self,
        id: &str,
        title: &str,
        document_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chat_sessions (id, title, document_id) VALUES (?1, ?2, ?3)",
            params![id, title, document_id],
        )?;
        Ok(())
    }

    pub fn list_chat_sessions(
        &self,
        document_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<ChatSession>> {
        let (sql, use_doc_filter) = match document_id {
            Some(_) => (
                "SELECT id, title, document_id, created_at, updated_at
                 FROM chat_sessions WHERE document_id = ?1
                 ORDER BY updated_at DESC LIMIT ?2",
                true,
            ),
            None => (
                "SELECT id, title, document_id, created_at, updated_at
                 FROM chat_sessions WHERE document_id IS NULL
                 ORDER BY updated_at DESC LIMIT ?1",
                false,
            ),
        };

        let mut stmt = self.conn.prepare(sql)?;

        let sessions = if use_doc_filter {
            stmt.query_map(params![document_id.unwrap(), limit], |row| {
                Ok(ChatSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    document_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?
        } else {
            stmt.query_map(params![limit], |row| {
                Ok(ChatSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    document_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?
        };

        Ok(sessions)
    }

    pub fn delete_chat_session(&self, id: &str) -> Result<()> {
        self.conn.execute("DELETE FROM chat_sessions WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn update_chat_session_title(&self, id: &str, title: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE chat_sessions SET title = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![id, title],
        )?;
        Ok(())
    }

    pub fn insert_chat_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        sources: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO chat_messages (session_id, role, content, sources) VALUES (?1, ?2, ?3, ?4)",
            params![session_id, role, content, sources],
        )?;
        self.conn.execute(
            "UPDATE chat_sessions SET updated_at = datetime('now') WHERE id = ?1",
            params![session_id],
        )?;
        Ok(())
    }

    // --- Reading progress ---

    pub fn save_reading_progress(&self, document_id: &str, position: f64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO reading_progress (document_id, scroll_position, last_read_at)
             VALUES (?1, ?2, datetime('now'))
             ON CONFLICT(document_id) DO UPDATE SET
                scroll_position = excluded.scroll_position,
                last_read_at = excluded.last_read_at",
            params![document_id, position],
        )?;
        Ok(())
    }

    pub fn get_reading_progress(&self, document_id: &str) -> Result<Option<f64>> {
        let result = self.conn.query_row(
            "SELECT scroll_position FROM reading_progress WHERE document_id = ?1",
            params![document_id],
            |row| row.get(0),
        );
        match result {
            Ok(pos) => Ok(Some(pos)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    // --- Table of contents ---

    pub fn insert_toc(&self, document_id: &str, entries: &[TocEntry]) -> Result<()> {
        self.conn.execute(
            "DELETE FROM document_toc WHERE document_id = ?1",
            params![document_id],
        )?;
        let mut stmt = self.conn.prepare(
            "INSERT INTO document_toc (document_id, title, level, href, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for (i, entry) in entries.iter().enumerate() {
            stmt.execute(params![
                document_id,
                entry.title,
                entry.level,
                entry.href,
                i as i64
            ])?;
        }
        Ok(())
    }

    pub fn get_toc(&self, document_id: &str) -> Result<Vec<TocEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT title, level, href FROM document_toc
             WHERE document_id = ?1 ORDER BY sort_order",
        )?;
        let entries = stmt
            .query_map(params![document_id], |row| {
                Ok(TocEntry {
                    title: row.get(0)?,
                    level: row.get(1)?,
                    href: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(entries)
    }

    // --- Summaries ---

    pub fn insert_summary(&self, document_id: &str, length: &str, content: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO document_summaries (document_id, length, content)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(document_id, length) DO UPDATE SET
                content = excluded.content,
                created_at = datetime('now')",
            params![document_id, length, content],
        )?;
        Ok(())
    }

    pub fn get_summary(&self, document_id: &str, length: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT content FROM document_summaries WHERE document_id = ?1 AND length = ?2",
            params![document_id, length],
            |row| row.get(0),
        );
        match result {
            Ok(content) => Ok(Some(content)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_all_summaries(&self, document_id: &str) -> Result<Vec<(String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT length, content FROM document_summaries WHERE document_id = ?1",
        )?;
        let results = stmt
            .query_map(params![document_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    // --- Tags ---

    pub fn set_document_tags(&self, doc_id: &str, tags: &[String]) -> Result<()> {
        // Clear existing tags for this document
        self.conn.execute(
            "DELETE FROM document_tags WHERE document_id = ?1",
            params![doc_id],
        )?;

        for tag_name in tags {
            let name = tag_name.trim();
            if name.is_empty() {
                continue;
            }
            // Insert tag if it doesn't exist
            self.conn.execute(
                "INSERT OR IGNORE INTO tags (name) VALUES (?1)",
                params![name],
            )?;
            // Link tag to document
            self.conn.execute(
                "INSERT OR IGNORE INTO document_tags (document_id, tag_id)
                 SELECT ?1, id FROM tags WHERE name = ?2",
                params![doc_id, name],
            )?;
        }
        Ok(())
    }

    pub fn list_all_tags(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT t.name FROM tags t
             JOIN document_tags dt ON t.id = dt.tag_id
             ORDER BY t.name COLLATE NOCASE",
        )?;
        let tags = stmt
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        Ok(tags)
    }

    // --- Whisper model management ---

    pub fn seed_whisper_models(&self) -> Result<()> {
        for model in crate::whisper::WHISPER_MODELS {
            self.conn.execute(
                "INSERT OR IGNORE INTO whisper_models (id, name, filename, size_bytes, status) VALUES (?1, ?2, ?3, ?4, 'available')",
                params![model.id, model.name, model.filename, model.size_bytes],
            )?;
        }
        Ok(())
    }

    pub fn list_whisper_models(&self) -> Result<Vec<crate::whisper::WhisperModel>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, filename, size_bytes, status, download_progress, downloaded_at, error FROM whisper_models ORDER BY size_bytes ASC",
        )?;
        let models = stmt
            .query_map([], |row| {
                Ok(crate::whisper::WhisperModel {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    filename: row.get(2)?,
                    size_bytes: row.get(3)?,
                    status: row.get(4)?,
                    download_progress: row.get(5)?,
                    downloaded_at: row.get(6)?,
                    error: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(models)
    }

    pub fn update_whisper_model_status(
        &self,
        id: &str,
        status: &str,
        progress: f64,
        error: Option<&str>,
    ) -> Result<()> {
        if status == "ready" {
            self.conn.execute(
                "UPDATE whisper_models SET status = ?2, download_progress = ?3, error = ?4, downloaded_at = datetime('now') WHERE id = ?1",
                params![id, status, progress, error],
            )?;
        } else {
            self.conn.execute(
                "UPDATE whisper_models SET status = ?2, download_progress = ?3, error = ?4 WHERE id = ?1",
                params![id, status, progress, error],
            )?;
        }
        Ok(())
    }

    pub fn update_document_media_metadata(
        &self,
        id: &str,
        duration_seconds: Option<f64>,
        source_url: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE documents SET duration_seconds = ?2, source_url = ?3 WHERE id = ?1",
            params![id, duration_seconds, source_url],
        )?;
        Ok(())
    }

    pub fn get_chat_messages(&self, session_id: &str) -> Result<Vec<ChatMessage>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, session_id, role, content, sources, created_at
             FROM chat_messages WHERE session_id = ?1
             ORDER BY created_at ASC",
        )?;

        let messages = stmt
            .query_map(params![session_id], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: row.get(2)?,
                    content: row.get(3)?,
                    sources: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(messages)
    }

    // --- Section Summaries ---

    pub fn insert_section_summaries(
        &self,
        document_id: &str,
        sections: &[(i32, i32, Option<&str>, &str, Option<&str>)], // (start, end, title, summary, key_concepts)
    ) -> Result<()> {
        // Clear existing sections for this document
        self.conn.execute(
            "DELETE FROM section_summaries WHERE document_id = ?1",
            params![document_id],
        )?;

        let mut stmt = self.conn.prepare(
            "INSERT INTO section_summaries (document_id, start_chunk, end_chunk, title, summary, key_concepts)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        )?;

        for (start, end, title, summary, concepts) in sections {
            stmt.execute(params![document_id, start, end, title, summary, concepts])?;
        }
        Ok(())
    }

    pub fn get_section_summaries(&self, document_id: &str) -> Result<Vec<SectionSummary>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, document_id, start_chunk, end_chunk, title, summary, key_concepts
             FROM section_summaries WHERE document_id = ?1
             ORDER BY start_chunk ASC",
        )?;
        let results = stmt
            .query_map(params![document_id], |row| {
                Ok(SectionSummary {
                    id: row.get(0)?,
                    document_id: row.get(1)?,
                    start_chunk: row.get(2)?,
                    end_chunk: row.get(3)?,
                    title: row.get(4)?,
                    summary: row.get(5)?,
                    key_concepts: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    pub fn has_section_summaries(&self, document_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM section_summaries WHERE document_id = ?1",
            params![document_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Get chunks by document, ordered by index.
    pub fn get_document_chunks(&self, document_id: &str) -> Result<Vec<(i32, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT chunk_index, content FROM document_chunks
             WHERE document_id = ?1 ORDER BY chunk_index ASC",
        )?;
        let results = stmt
            .query_map(params![document_id], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    /// Search chunks by document (for getting a representative chunk).
    pub fn search_chunks_by_document(&self, document_id: &str, limit: usize) -> Result<Vec<ChunkSearchResult>> {
        let mut stmt = self.conn.prepare(
            "SELECT content, chunk_index FROM document_chunks
             WHERE document_id = ?1 ORDER BY chunk_index ASC LIMIT ?2",
        )?;
        let results = stmt
            .query_map(params![document_id, limit as i64], |row| {
                Ok(ChunkSearchResult {
                    content: row.get(0)?,
                    chunk_index: row.get::<_, i64>(1)? as usize,
                    distance: 0.0,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    // --- Topic Clusters ---

    pub fn create_topic_cluster(&self, id: &str, label: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO topic_clusters (id, label, updated_at) VALUES (?1, ?2, datetime('now'))",
            params![id, label],
        )?;
        Ok(())
    }

    pub fn set_cluster_members(&self, cluster_id: &str, doc_ids: &[String]) -> Result<()> {
        self.conn.execute(
            "DELETE FROM topic_cluster_members WHERE cluster_id = ?1",
            params![cluster_id],
        )?;
        let mut stmt = self.conn.prepare(
            "INSERT INTO topic_cluster_members (cluster_id, document_id) VALUES (?1, ?2)",
        )?;
        for doc_id in doc_ids {
            stmt.execute(params![cluster_id, doc_id])?;
        }
        Ok(())
    }

    pub fn upsert_cluster_summary(&self, cluster_id: &str, summary: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE topic_clusters SET summary = ?2, updated_at = datetime('now') WHERE id = ?1",
            params![cluster_id, summary],
        )?;
        Ok(())
    }

    pub fn get_topic_clusters(&self) -> Result<Vec<TopicCluster>> {
        let mut stmt = self.conn.prepare(
            "SELECT tc.id, tc.label, tc.summary,
                    (SELECT COUNT(*) FROM topic_cluster_members WHERE cluster_id = tc.id)
             FROM topic_clusters tc ORDER BY tc.label",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok(TopicCluster {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    summary: row.get(2)?,
                    document_count: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }

    pub fn get_cluster_document_ids(&self, cluster_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT document_id FROM topic_cluster_members WHERE cluster_id = ?1",
        )?;
        let results = stmt
            .query_map(params![cluster_id], |row| row.get(0))?
            .collect::<Result<Vec<String>>>()?;
        Ok(results)
    }

    pub fn clear_topic_clusters(&self) -> Result<()> {
        self.conn.execute("DELETE FROM topic_cluster_members", [])?;
        self.conn.execute("DELETE FROM topic_clusters", [])?;
        Ok(())
    }

    // --- Library Summary ---

    pub fn upsert_library_summary(&self, summary: &str, themes: Option<&str>, doc_count: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO library_summary (id, summary, themes, document_count, updated_at)
             VALUES (1, ?1, ?2, ?3, datetime('now'))
             ON CONFLICT(id) DO UPDATE SET
                summary = excluded.summary,
                themes = excluded.themes,
                document_count = excluded.document_count,
                updated_at = datetime('now')",
            params![summary, themes, doc_count],
        )?;
        Ok(())
    }

    pub fn get_library_summary(&self) -> Result<Option<LibrarySummary>> {
        let result = self.conn.query_row(
            "SELECT summary, themes, document_count, updated_at FROM library_summary WHERE id = 1",
            [],
            |row| {
                Ok(LibrarySummary {
                    summary: row.get(0)?,
                    themes: row.get(1)?,
                    document_count: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            },
        );
        match result {
            Ok(s) => Ok(Some(s)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get all document IDs and titles (for clustering).
    pub fn list_document_ids_and_titles(&self) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, author FROM documents WHERE status = 'complete' ORDER BY title",
        )?;
        let results = stmt
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })?
            .collect::<Result<Vec<_>>>()?;
        Ok(results)
    }
}
