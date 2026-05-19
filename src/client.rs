use reqwest::blocking::Client;
use serde::Deserialize;

use crate::bundle::{ConfigBundle, MetaInfo};
use crate::config::OctConfig;
use crate::error::{OctError, Result};
use crate::token::token_hash;

#[derive(Deserialize, Debug)]
pub struct ServerError {
    pub error: ErrorDetail,
    #[serde(rename = "requestId")]
    pub request_id: String,
}

#[derive(Deserialize, Debug)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
}

pub fn push_config(config: &OctConfig, bundle: &ConfigBundle) -> Result<MetaInfo> {
    let client = Client::new();
    let url = format!("{}/api/v1/config", config.endpoint);
    let _token_hash_hex = token_hash(&config.token);

    let resp = client
        .put(&url)
        .header("Authorization", format!("Bearer {}", config.token))
        .header("Content-Type", "application/json")
        .json(bundle)
        .send()?;

    if resp.status().is_success() {
        let meta: MetaInfo = resp.json()?;
        Ok(meta)
    } else {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ServerError>(&body) {
            Err(OctError::ServerError {
                code: err.error.code,
                message: err.error.message,
            })
        } else {
            Err(OctError::ServerError {
                code: format!("HTTP_{}", status.as_u16()),
                message: body,
            })
        }
    }
}

pub fn pull_config(config: &OctConfig) -> Result<ConfigBundle> {
    let client = Client::new();
    let url = format!("{}/api/v1/config", config.endpoint);

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.token))
        .send()?;

    if resp.status().is_success() {
        let bundle: ConfigBundle = resp.json()?;
        Ok(bundle)
    } else {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        if let Ok(err) = serde_json::from_str::<ServerError>(&body) {
            Err(OctError::ServerError {
                code: err.error.code,
                message: err.error.message,
            })
        } else {
            Err(OctError::ServerError {
                code: format!("HTTP_{}", status.as_u16()),
                message: body,
            })
        }
    }
}

pub fn get_meta(config: &OctConfig) -> Result<MetaInfo> {
    let client = Client::new();
    let url = format!("{}/api/v1/meta", config.endpoint);

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", config.token))
        .send()?;

    if resp.status().is_success() {
        let meta: MetaInfo = resp.json()?;
        Ok(meta)
    } else {
        let status = resp.status();
        let body = resp.text().unwrap_or_default();
        Err(OctError::ServerError {
            code: format!("HTTP_{}", status.as_u16()),
            message: body,
        })
    }
}
