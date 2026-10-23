use std::fs;
use std::path::Path;

use anyhow::{ensure, Context, Result};

pub fn read_text(root: &Path, relative: &str) -> Result<Option<String>> {
    let path = root.join(relative);
    if !path
        .try_exists()
        .with_context(|| format!("Cannot inspect {relative}"))?
    {
        return Ok(None);
    }
    let resolved = fs::canonicalize(&path).with_context(|| format!("Cannot access {relative}"))?;
    ensure!(
        resolved.starts_with(root),
        "{relative} points outside the project"
    );
    let metadata = fs::metadata(&resolved)?;
    ensure!(metadata.is_file(), "{relative} must be a regular file");
    ensure!(
        metadata.len() <= 16 * 1024 * 1024,
        "{relative} exceeds the 16 MiB inspection limit"
    );
    fs::read_to_string(resolved)
        .map(Some)
        .with_context(|| format!("Cannot read {relative} as UTF-8"))
}

pub fn yaml_documents(text: &str) -> Result<Vec<serde_json::Value>> {
    use serde::Deserialize;
    serde_yaml_ng::Deserializer::from_str(text)
        .map(|document| serde_json::Value::deserialize(document).context("Invalid YAML document"))
        .collect()
}
