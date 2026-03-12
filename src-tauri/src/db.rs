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
    pub tags: Vec<String>,
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
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE file_hash = ?1",
            params![hash],
            |row| row.get(0),
        )?;
        Ok(count > 0)
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

    pub fn list_documents(&self, offset: i64, limit: i64) -> Result<Vec<Document>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.*, GROUP_CONCAT(t.name, ', ') as tag_list
             FROM documents d
             LEFT JOIN document_tags dt ON d.id = dt.document_id
             LEFT JOIN tags t ON dt.tag_id = t.id
             GROUP BY d.id
             ORDER BY d.imported_at DESC
             LIMIT ?1 OFFSET ?2",
        )?;

        let docs = stmt
            .query_map(params![limit, offset], |row| {
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
                    tags,
                })
            })?
            .collect::<Result<Vec<_>>>()?;

        Ok(docs)
    }

    pub fn get_document(&self, id: &str) -> Result<Option<Document>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.*, GROUP_CONCAT(t.name, ', ') as tag_list
             FROM documents d
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
                tags,
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

        self.conn
            .execute("DELETE FROM documents WHERE id = ?1", params![id])?;
        Ok(path)
    }

    pub fn search(&self, query: &str, limit: i64) -> Result<Vec<SearchResult>> {
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
            .query_map(params![query, limit], |row| {
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
}
