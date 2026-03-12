use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub struct StorageLayout {
    base: PathBuf,
}

impl StorageLayout {
    pub fn new(base: PathBuf) -> Self {
        Self { base }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        fs::create_dir_all(self.originals_dir())?;
        fs::create_dir_all(self.markdown_dir())?;
        fs::create_dir_all(self.covers_dir())?;
        Ok(())
    }

    pub fn originals_dir(&self) -> PathBuf {
        self.base.join("originals")
    }

    pub fn markdown_dir(&self) -> PathBuf {
        self.base.join("markdown")
    }

    pub fn covers_dir(&self) -> PathBuf {
        self.base.join("covers")
    }

    /// Compute SHA-256 hash of a file, streaming to handle large files
    pub fn compute_hash(path: &Path) -> std::io::Result<String> {
        let mut file = fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Get the storage path for an original file: originals/{prefix}/{hash}.{ext}
    pub fn original_path(&self, file_hash: &str, extension: &str) -> PathBuf {
        let prefix = &file_hash[..2];
        self.originals_dir()
            .join(prefix)
            .join(format!("{}.{}", file_hash, extension))
    }

    /// Copy a file into content-addressable storage, returns the relative path
    pub fn store_original(&self, source: &Path, file_hash: &str, extension: &str) -> std::io::Result<String> {
        let dest = self.original_path(file_hash, extension);
        fs::create_dir_all(dest.parent().unwrap())?;

        if !dest.exists() {
            fs::copy(source, &dest)?;
        }

        // Return relative path from storage base
        let rel = dest.strip_prefix(&self.base).unwrap();
        Ok(rel.to_string_lossy().to_string())
    }

    /// Path for a document's markdown file
    pub fn markdown_path(&self, document_id: &str) -> PathBuf {
        self.markdown_dir().join(format!("{}.md", document_id))
    }

    /// Write markdown content, returns relative path
    pub fn write_markdown(&self, document_id: &str, content: &str) -> std::io::Result<String> {
        let path = self.markdown_path(document_id);
        fs::write(&path, content)?;
        let rel = path.strip_prefix(&self.base).unwrap();
        Ok(rel.to_string_lossy().to_string())
    }

    /// Read markdown content
    pub fn read_markdown(&self, document_id: &str) -> std::io::Result<String> {
        let path = self.markdown_path(document_id);
        fs::read_to_string(path)
    }

    /// Path for a document's cover image
    pub fn cover_path(&self, document_id: &str) -> PathBuf {
        self.covers_dir().join(format!("{}.png", document_id))
    }

    /// Write cover image, returns relative path
    pub fn write_cover(&self, document_id: &str, data: &[u8]) -> std::io::Result<String> {
        let path = self.cover_path(document_id);
        fs::write(&path, data)?;
        let rel = path.strip_prefix(&self.base).unwrap();
        Ok(rel.to_string_lossy().to_string())
    }

    /// Read cover image bytes
    pub fn read_cover(&self, document_id: &str) -> std::io::Result<Vec<u8>> {
        let path = self.cover_path(document_id);
        fs::read(path)
    }

    /// Get absolute path to an original file
    pub fn resolve_original(&self, relative_path: &str) -> PathBuf {
        self.base.join(relative_path)
    }

    /// Get file size
    pub fn file_size(path: &Path) -> std::io::Result<u64> {
        Ok(fs::metadata(path)?.len())
    }

    /// Detect format from file extension
    pub fn detect_format(path: &Path) -> Option<String> {
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
    }
}
