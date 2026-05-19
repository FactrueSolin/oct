use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::{OctError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPath {
    pub label: String,
    pub path: PathBuf,
    pub exists: bool,
    pub kind: PathKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PathKind {
    UserConfig,
    UserData,
    LegacyHome,
    ManagedConfig,
    EnvOverride,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathDiscovery {
    pub paths: Vec<DiscoveredPath>,
    pub active_config: Option<PathBuf>,
    pub active_data: Option<PathBuf>,
}

impl PathDiscovery {
    pub fn run() -> Result<Self> {
        let mut paths = Vec::new();
        let mut active_config = None;
        let mut active_data = None;

        // Environment variable overrides take highest priority
        if let Ok(dir) = std::env::var("OPENCODE_CONFIG_DIR") {
            let p = PathBuf::from(&dir);
            let exists = p.exists();
            if exists && active_config.is_none() {
                active_config = Some(p.clone());
            }
            paths.push(DiscoveredPath {
                label: "OPENCODE_CONFIG_DIR".into(),
                path: p,
                exists,
                kind: PathKind::EnvOverride,
            });
        }

        if let Ok(dir) = std::env::var("OPENCODE_DATA_DIR") {
            let p = PathBuf::from(&dir);
            let exists = p.exists();
            if exists && active_data.is_none() {
                active_data = Some(p.clone());
            }
            paths.push(DiscoveredPath {
                label: "OPENCODE_DATA_DIR".into(),
                path: p,
                exists,
                kind: PathKind::EnvOverride,
            });
        }

        // XDG-based paths (opencode source uses xdg-basedir)
        if let Some(config_dir) = dirs::config_dir() {
            let p = config_dir.join("opencode");
            let exists = p.exists();
            if exists && active_config.is_none() {
                active_config = Some(p.clone());
            }
            paths.push(DiscoveredPath {
                label: "XDG config (~/.config/opencode)".into(),
                path: p,
                exists,
                kind: PathKind::UserConfig,
            });
        }

        if let Some(data_dir) = dirs::data_dir() {
            let p = data_dir.join("opencode");
            let exists = p.exists();
            if exists && active_data.is_none() {
                active_data = Some(p.clone());
            }
            paths.push(DiscoveredPath {
                label: "XDG data (~/.local/share/opencode)".into(),
                path: p,
                exists,
                kind: PathKind::UserData,
            });
        }

        // Legacy ~/.opencode
        if let Some(home) = dirs::home_dir() {
            let p = home.join(".opencode");
            let exists = p.exists();
            if exists && active_config.is_none() {
                active_config = Some(p.clone());
            }
            paths.push(DiscoveredPath {
                label: "Legacy ~/.opencode".into(),
                path: p,
                exists,
                kind: PathKind::LegacyHome,
            });
        }

        // Managed config (system-wide)
        let managed = managed_config_dir();
        if let Some(p) = managed {
            let exists = p.exists();
            paths.push(DiscoveredPath {
                label: "Managed config".into(),
                path: p.clone(),
                exists,
                kind: PathKind::ManagedConfig,
            });
        }

        Ok(PathDiscovery {
            paths,
            active_config,
            active_data,
        })
    }

    pub fn config_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let dirs_to_scan = self
            .active_config
            .iter()
            .chain(self.active_data.iter())
            .cloned()
            .collect::<Vec<_>>();

        for dir in &dirs_to_scan {
            if !dir.exists() {
                continue;
            }
            scan_config_dir(dir, &mut files)?;
        }

        if files.is_empty() {
            return Err(OctError::NoConfigFound);
        }

        Ok(files)
    }
}

fn managed_config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        Some(PathBuf::from("/Library/Application Support/opencode"))
    } else if cfg!(target_os = "windows") {
        std::env::var("ProgramData")
            .ok()
            .map(|p| PathBuf::from(p).join("opencode"))
    } else {
        Some(PathBuf::from("/etc/opencode"))
    }
}

fn scan_config_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let safe_extensions = ["json", "jsonc", "toml", "ts", "js", "yaml", "yml"];
    let safe_dirs = [
        "agents", "commands", "modes", "plugins", "skills", "tools", "themes",
    ];
    let excluded_files = ["auth.json", "mcp-auth.json"];
    let excluded_patterns = ["opencode.db", "opencode-"];

    for entry in walkdir::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Exclude sensitive files
        if excluded_files.contains(&file_name) {
            continue;
        }
        if excluded_patterns.iter().any(|p| file_name.starts_with(p)) {
            continue;
        }

        // Check extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        if !safe_extensions.contains(&ext) {
            continue;
        }

        files.push(path.to_path_buf());
    }

    // Also scan safe subdirectories
    for safe_dir in &safe_dirs {
        let sub = dir.join(safe_dir);
        if sub.exists() {
            scan_config_dir(&sub, files)?;
        }
    }

    Ok(())
}
