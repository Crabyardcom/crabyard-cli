use std::fs;
use std::path::PathBuf;

use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct UserConfig {
    #[serde(default)]
    pub defaults: Defaults,
}

#[derive(Debug, Deserialize, Default)]
pub struct Defaults {
    pub pm: Option<String>,
    pub auth: Option<String>,
    pub ui: Option<String>,
    pub author: Option<String>,
    pub license_key: Option<String>,
}

fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".crabyardrc"))
}

pub fn load() -> UserConfig {
    let path = match config_path() {
        Some(p) if p.exists() => p,
        _ => return UserConfig::default(),
    };

    let content = match fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return UserConfig::default(),
    };

    toml::from_str(&content).unwrap_or_default()
}
