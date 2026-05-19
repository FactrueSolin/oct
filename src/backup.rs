use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::path::PathBuf;

use crate::error::{OctError, Result};
use crate::config::oct_backups_dir;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupManifest {
    pub id: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub trigger: String,
    pub files: Vec<BackupFileEntry>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BackupFileEntry {
    #[serde(rename = "originalPath")]
    pub original_path: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    pub sha256: String,
}

pub fn create_backup(files: &[(PathBuf, PathBuf)], trigger: &str) -> Result<BackupManifest> {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let id = format!("{}-{}", timestamp, &uuid::Uuid::new_v4().to_string()[..8]);
    let backup_dir = oct_backups_dir()?.join(&id);
    let files_dir = backup_dir.join("files");
    std::fs::create_dir_all(&files_dir)?;

    let mut manifest_files = Vec::new();

    for (source, base_dir) in files {
        if !source.exists() {
            continue;
        }
        let content = std::fs::read(source)?;
        let sha256 = format!("{:x}", sha2::Sha256::digest(&content));

        let rel = source
            .strip_prefix(base_dir)
            .unwrap_or_else(|_| source.file_name().unwrap_or_default().as_ref());
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let dest = files_dir.join(&rel_str);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&dest, content)?;

        manifest_files.push(BackupFileEntry {
            original_path: source.to_string_lossy().to_string(),
            relative_path: rel_str,
            sha256,
        });
    }

    let manifest = BackupManifest {
        id: id.clone(),
        created_at: Utc::now().to_rfc3339(),
        trigger: trigger.to_string(),
        files: manifest_files,
    };

    let manifest_path = backup_dir.join("manifest.json");
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)?)?;

    Ok(manifest)
}

pub fn list_backups() -> Result<Vec<BackupManifest>> {
    let backups_dir = oct_backups_dir()?;
    if !backups_dir.exists() {
        return Ok(Vec::new());
    }

    let mut backups = Vec::new();
    for entry in std::fs::read_dir(&backups_dir)? {
        let entry = entry?;
        let manifest_path = entry.path().join("manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            let manifest: BackupManifest = serde_json::from_str(&content)?;
            backups.push(manifest);
        }
    }

    backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(backups)
}

pub fn restore_backup(backup_id: &str) -> Result<()> {
    let backups_dir = oct_backups_dir()?;
    let backup_dir = backups_dir.join(backup_id);
    let manifest_path = backup_dir.join("manifest.json");

    if !manifest_path.exists() {
        return Err(OctError::Backup(format!(
            "backup {} not found",
            backup_id
        )));
    }

    let content = std::fs::read_to_string(&manifest_path)?;
    let manifest: BackupManifest = serde_json::from_str(&content)?;

    for entry in &manifest.files {
        let source = backup_dir.join("files").join(&entry.relative_path);
        if !source.exists() {
            eprintln!("warning: backup file missing: {}", entry.relative_path);
            continue;
        }

        // Determine where to restore: try original path, fallback to active config dir
        let dest = PathBuf::from(&entry.original_path);
        let target = if dest.exists() || dest.parent().map(|p| p.exists()).unwrap_or(false) {
            dest
        } else {
            // Fallback: restore to current config dir
            if let Some(config_dir) = dirs::config_dir() {
                config_dir.join("opencode").join(&entry.relative_path)
            } else {
                return Err(OctError::Backup(format!(
                    "cannot determine restore path for {}",
                    entry.relative_path
                )));
            }
        };

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&source, &target)?;
        eprintln!("restored: {}", target.display());
    }

    eprintln!("backup {} restored successfully", backup_id);
    Ok(())
}
