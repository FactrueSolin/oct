use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::error::{OctError, Result};
use crate::paths::PathDiscovery;

const MAX_BUNDLE_SIZE: usize = 10 * 1024 * 1024; // 10MB
const MAX_FILE_SIZE: usize = 1 * 1024 * 1024; // 1MB

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileEntry {
    pub path: String,
    pub content_base64: String,
    pub sha256: String,
    pub size: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ConfigBundle {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub platform: String,
    pub files: Vec<FileEntry>,
    #[serde(rename = "totalBytes")]
    pub total_bytes: usize,
    #[serde(rename = "bundleHash")]
    pub bundle_hash: String,
}

impl ConfigBundle {
    pub fn from_discovery(discovery: &PathDiscovery) -> Result<Self> {
        let files = discovery.config_files()?;
        let mut entries = Vec::new();
        let mut total_bytes = 0usize;

        for file_path in &files {
            let content = std::fs::read(file_path)?;
            if content.len() > MAX_FILE_SIZE {
                return Err(OctError::Bundle(format!(
                    "file {} exceeds max size ({} bytes)",
                    file_path.display(),
                    MAX_FILE_SIZE
                )));
            }

            let rel_path = make_relative_path(file_path, discovery)?;
            validate_path(&rel_path)?;

            let sha256 = compute_sha256(&content);
            let content_base64 = STANDARD.encode(&content);
            let size = content.len();
            total_bytes += size;

            entries.push(FileEntry {
                path: rel_path,
                content_base64,
                sha256,
                size,
            });
        }

        if total_bytes > MAX_BUNDLE_SIZE {
            return Err(OctError::Bundle(format!(
                "total bundle size {} bytes exceeds limit {} bytes",
                total_bytes, MAX_BUNDLE_SIZE
            )));
        }

        let mut bundle = ConfigBundle {
            schema_version: 1,
            created_at: Utc::now().to_rfc3339(),
            platform: std::env::consts::OS.to_string(),
            files: entries,
            total_bytes,
            bundle_hash: String::new(),
        };

        bundle.bundle_hash = bundle.compute_hash();
        Ok(bundle)
    }

    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        for file in &self.files {
            hasher.update(file.path.as_bytes());
            hasher.update(file.sha256.as_bytes());
        }
        format!("{:x}", hasher.finalize())
    }

    pub fn to_meta(&self) -> MetaInfo {
        MetaInfo {
            schema_version: self.schema_version,
            updated_at: self.created_at.clone(),
            file_count: self.files.len(),
            total_bytes: self.total_bytes,
            bundle_hash: self.bundle_hash.clone(),
            platform: self.platform.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MetaInfo {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "fileCount")]
    pub file_count: usize,
    #[serde(rename = "totalBytes")]
    pub total_bytes: usize,
    #[serde(rename = "bundleHash")]
    pub bundle_hash: String,
    pub platform: String,
}

pub fn apply_bundle(bundle: &ConfigBundle, target_dir: &Path) -> Result<()> {
    for entry in &bundle.files {
        validate_path(&entry.path)?;
        let full_path = target_dir.join(&entry.path);

        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = STANDARD
            .decode(&entry.content_base64)
            .map_err(|e| OctError::Bundle(format!("base64 decode failed: {}", e)))?;

        let actual_hash = compute_sha256(&content);
        if actual_hash != entry.sha256 {
            return Err(OctError::Bundle(format!(
                "sha256 mismatch for file {}",
                entry.path
            )));
        }

        std::fs::write(&full_path, content)?;
    }
    Ok(())
}

fn make_relative_path(absolute: &Path, discovery: &PathDiscovery) -> Result<String> {
    let candidates: Vec<&PathBuf> = discovery
        .active_config
        .iter()
        .chain(discovery.active_data.iter())
        .collect();

    for base in &candidates {
        if let Ok(rel) = absolute.strip_prefix(base) {
            let rel_str = rel
                .to_str()
                .ok_or_else(|| OctError::PathSecurity("non-UTF8 path".into()))?;
            // Normalize Windows backslashes
            return Ok(rel_str.replace('\\', "/"));
        }
    }

    // Fallback: use the last two path components
    if let Some(name) = absolute.file_name() {
        if let Some(parent) = absolute.parent().and_then(|p| p.file_name()) {
            return Ok(format!("{}/{}", parent.to_string_lossy(), name.to_string_lossy()));
        }
    }

    Err(OctError::PathSecurity(format!(
        "cannot make relative path for {}",
        absolute.display()
    )))
}

fn validate_path(path: &str) -> Result<()> {
    if path.starts_with('/') || path.contains("..") {
        return Err(OctError::PathSecurity(format!(
            "path contains traversal: {}",
            path
        )));
    }
    if path.contains(':') && path.len() > 1 {
        return Err(OctError::PathSecurity(format!(
            "path contains drive letter: {}",
            path
        )));
    }
    if path.contains('\\') {
        return Err(OctError::PathSecurity(format!(
            "path contains backslash: {}",
            path
        )));
    }
    let reserved = ["NUL", "CON", "PRN", "AUX", "COM1", "LPT1"];
    let stem = path
        .split('/')
        .next_back()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or(path);
    if reserved.contains(&stem.to_uppercase().as_str()) {
        return Err(OctError::PathSecurity(format!(
            "path contains reserved name: {}",
            path
        )));
    }
    Ok(())
}

fn compute_sha256(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}
