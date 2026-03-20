//! Curated collections — pre-packaged document bundles distributed via GitHub.
//!
//! A collection manifest is a JSON file that defines a set of documents
//! with download URLs, metadata, and tags. Users can browse available
//! collections and import them with one click.

use serde::{Deserialize, Serialize};

/// A collection manifest — defines a curated set of documents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionManifest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub documents: Vec<CollectionDocument>,
}

/// A document entry within a collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionDocument {
    pub title: String,
    pub author: String,
    pub url: String,
    pub format: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// SHA-256 hash for dedup/integrity (optional)
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Summary of an available collection (for UI listing).
#[derive(Debug, Clone, Serialize)]
pub struct CollectionInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub document_count: usize,
    pub tags: Vec<String>,
    pub installed_count: usize,
}

/// Fetch a collection manifest from a URL.
pub async fn fetch_manifest(url: &str) -> Result<CollectionManifest, String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
        .map_err(|e| format!("Failed to fetch collection: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Collection fetch failed: {}", resp.status()));
    }

    resp.json::<CollectionManifest>()
        .await
        .map_err(|e| format!("Invalid collection manifest: {}", e))
}

/// Download a single document from a collection to a temp path.
pub async fn download_document(
    url: &str,
    dest: &std::path::Path,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed: {}", resp.status()));
    }

    let bytes = resp.bytes().await
        .map_err(|e| format!("Failed to read download: {}", e))?;

    std::fs::write(dest, &bytes)
        .map_err(|e| format!("Failed to write file: {}", e))?;

    Ok(())
}
