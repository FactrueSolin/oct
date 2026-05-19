use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{OctError, Result};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OctConfig {
    pub token: String,
    pub endpoint: String,
}

impl OctConfig {
    pub fn new(token: String, endpoint: String) -> Self {
        Self { token, endpoint }
    }

    pub fn load() -> Result<Self> {
        let path = oct_data_dir()?.join("config.toml");
        if !path.exists() {
            return Err(OctError::NotInitialized);
        }
        let content = std::fs::read_to_string(&path)?;
        let config: OctConfig = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let dir = oct_data_dir()?;
        std::fs::create_dir_all(&dir)?;
        let path = dir.join("config.toml");
        let content = toml::to_string_pretty(self).map_err(|e| OctError::TomlSer(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}

pub fn oct_data_dir() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .ok_or_else(|| OctError::Config("cannot determine config directory".into()))?
        .join("oct");
    Ok(dir)
}

pub fn oct_backups_dir() -> Result<PathBuf> {
    Ok(oct_data_dir()?.join("backups"))
}
