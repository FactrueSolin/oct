use rand::Rng;
use regex::Regex;

use crate::error::{OctError, Result};

const MIN_TOKEN_LENGTH: usize = 32;

pub fn generate_token() -> String {
    let chars: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"
        .chars()
        .collect();
    let mut rng = rand::thread_rng();
    let len = 48;
    (0..len)
        .map(|_| chars[rng.gen_range(0..chars.len())])
        .collect()
}

pub fn validate_token(token: &str) -> Result<()> {
    let re = Regex::new(r"^[A-Za-z0-9]{32,}$").unwrap();
    if !re.is_match(token) {
        return Err(OctError::TokenInvalid(
            "token must contain only letters and digits, and be at least 32 characters long".into(),
        ));
    }
    Ok(())
}

pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    format!("{:x}", result)
}
