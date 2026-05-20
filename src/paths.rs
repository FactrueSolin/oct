use serde::{Deserialize, Serialize};
use std::collections::HashSet;
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

        // Legacy home-relative locations must use PathBuf::join; string concatenation
        // produces broken Windows paths such as C:\Users\Daifukuopencode.
        if let Some(home) = dirs::home_dir() {
            for (label, p) in legacy_home_dirs(&home) {
                let exists = p.exists();
                if exists && active_config.is_none() {
                    active_config = Some(p.clone());
                }
                paths.push(DiscoveredPath {
                    label: label.into(),
                    path: p,
                    exists,
                    kind: PathKind::LegacyHome,
                });
            }
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
        let mut seen = HashSet::new();
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

        files.retain(|file| seen.insert(file.clone()));
        files.sort();

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

fn legacy_home_dirs(home: &Path) -> [(&'static str, PathBuf); 2] {
    [
        ("Legacy ~/.opencode", home.join(".opencode")),
        ("Legacy ~/opencode", home.join("opencode")),
    ]
}

fn scan_config_dir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for file_name in ROOT_CONFIG_FILES {
        push_if_file(dir.join(file_name), files);
    }

    for subdir in WHITELISTED_CONFIG_DIRS {
        scan_whitelisted_subdir(&dir.join(subdir), files)?;
    }

    Ok(())
}

const ROOT_CONFIG_FILES: &[&str] = &[
    "opencode.json",
    "opencode.jsonc",
    "package.json",
    "package-lock.json",
];

const WHITELISTED_CONFIG_DIRS: &[&str] = &["agents", "commands", "plugins", "plugin"];

const SAFE_CONFIG_EXTENSIONS: &[&str] = &["json", "jsonc", "toml", "ts", "js", "yaml", "yml", "md"];

const EXCLUDED_DIRS: &[&str] = &[
    ".cache",
    ".git",
    ".hg",
    ".next",
    ".svn",
    ".wrangler",
    "build",
    "cache",
    "coverage",
    "dist",
    "lib",
    "node_modules",
    "out",
    "target",
    "tmp",
    "vendor",
];

const EXCLUDED_FILES: &[&str] = &[
    "auth.json",
    "bun.lockb",
    "mcp-auth.json",
    "package-lock.json",
    "package.json",
    "pnpm-lock.yaml",
    "yarn.lock",
];

const EXCLUDED_PREFIXES: &[&str] = &["opencode.db", "opencode-"];

fn push_if_file(path: PathBuf, files: &mut Vec<PathBuf>) {
    if path.is_file() {
        files.push(path);
    }
}

fn scan_whitelisted_subdir(dir: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(dir)
        .max_depth(3)
        .into_iter()
        .filter_entry(|e| !is_excluded_dir(e.path()))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let lower_file_name = file_name.to_ascii_lowercase();
        if EXCLUDED_FILES.contains(&lower_file_name.as_str()) {
            continue;
        }
        if EXCLUDED_PREFIXES
            .iter()
            .any(|p| lower_file_name.starts_with(p))
        {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !SAFE_CONFIG_EXTENSIONS.contains(&ext) {
            continue;
        }

        files.push(path.to_path_buf());
    }

    Ok(())
}

fn is_excluded_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|name| EXCLUDED_DIRS.contains(&name.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn scan_uses_whitelist_and_skips_dependencies() {
        let root = temp_dir("whitelist");
        std::fs::create_dir_all(root.join("plugins").join("node_modules").join("dep")).unwrap();
        std::fs::create_dir_all(root.join("plugins").join("dist")).unwrap();
        std::fs::create_dir_all(root.join("plugins").join("nested-plugin")).unwrap();
        std::fs::create_dir_all(root.join("node_modules").join("top_dep")).unwrap();
        std::fs::create_dir_all(root.join("unknown")).unwrap();

        std::fs::write(root.join("opencode.json"), "{}").unwrap();
        std::fs::write(root.join("package.json"), "{}").unwrap();
        std::fs::write(root.join("package-lock.json"), "{}").unwrap();
        std::fs::write(
            root.join("plugins").join("graphify.js"),
            "export default {}",
        )
        .unwrap();
        std::fs::create_dir_all(root.join("agents")).unwrap();
        std::fs::write(root.join("agents").join("coder.md"), "agent").unwrap();
        std::fs::write(
            root.join("plugins")
                .join("nested-plugin")
                .join("package-lock.json"),
            "{}",
        )
        .unwrap();
        std::fs::write(
            root.join("plugins")
                .join("node_modules")
                .join("dep")
                .join("index.js"),
            "dependency",
        )
        .unwrap();
        std::fs::write(root.join("plugins").join("dist").join("bundle.js"), "built").unwrap();
        std::fs::write(
            root.join("node_modules").join("top_dep").join("index.js"),
            "dependency",
        )
        .unwrap();
        std::fs::write(root.join("unknown").join("extra.js"), "unknown").unwrap();

        let mut files = Vec::new();
        scan_config_dir(&root, &mut files).unwrap();
        let mut rel_files = files
            .iter()
            .map(|p| {
                p.strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect::<Vec<_>>();
        rel_files.sort();

        assert_eq!(
            rel_files,
            vec![
                "agents/coder.md",
                "opencode.json",
                "package-lock.json",
                "package.json",
                "plugins/graphify.js"
            ]
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn legacy_home_path_uses_path_join() {
        let dirs = legacy_home_dirs(Path::new("/home/factrue"));
        assert_eq!(dirs[0].1, PathBuf::from("/home/factrue").join(".opencode"));
        assert_eq!(dirs[1].1, PathBuf::from("/home/factrue").join("opencode"));
    }

    #[cfg(windows)]
    #[test]
    fn legacy_home_path_keeps_windows_separator() {
        let dirs = legacy_home_dirs(Path::new(r"C:\Users\Daifuku"));
        assert_eq!(dirs[0].1, PathBuf::from(r"C:\Users\Daifuku\.opencode"));
        assert_eq!(dirs[1].1, PathBuf::from(r"C:\Users\Daifuku\opencode"));
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("oct-{}-{}", prefix, unique))
    }
}
