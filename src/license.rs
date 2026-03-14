use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;

const API_BASE: &str = "https://crabyard-api-production.up.railway.app";
const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct ValidateResponse {
    valid: bool,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    download_url: Option<String>,
}

pub fn validate_and_resolve(license_key: &str) -> Result<PathBuf> {
    let cached = cache_dir().join(TEMPLATE_VERSION);
    if cached.join("base").is_dir() {
        validate_key(license_key)?;
        return Ok(cached);
    }

    let resp = validate_key(license_key)?;

    let download_url = resp
        .download_url
        .unwrap_or_else(|| format!("{API_BASE}/template/{TEMPLATE_VERSION}"));

    download_template(&download_url, license_key, &cached)?;
    Ok(cached)
}

pub fn refresh_cache(license_key: &str) -> Result<PathBuf> {
    let cached = cache_dir().join(TEMPLATE_VERSION);
    if cached.exists() {
        fs::remove_dir_all(&cached)
            .with_context(|| format!("Failed to clear cached template at {}", cached.display()))?;
    }
    validate_and_resolve(license_key)
}

fn validate_key(license_key: &str) -> Result<ValidateResponse> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(format!("{API_BASE}/license/validate"))
        .json(&serde_json::json!({ "key": license_key }))
        .send()
        .context("Failed to reach license server")?;

    if resp.status() == reqwest::StatusCode::UNAUTHORIZED
        || resp.status() == reqwest::StatusCode::FORBIDDEN
    {
        anyhow::bail!("Invalid license key. Purchase one at https://crabyard.dev");
    }

    if !resp.status().is_success() {
        anyhow::bail!(
            "License server returned HTTP {}. Try again later.",
            resp.status()
        );
    }

    let body: ValidateResponse = resp
        .json()
        .context("Invalid response from license server")?;

    if !body.valid {
        let msg = body.message.unwrap_or_default();
        anyhow::bail!("License invalid: {msg}");
    }

    Ok(body)
}

fn download_template(url: &str, license_key: &str, dest: &Path) -> Result<()> {
    println!("  Downloading template v{TEMPLATE_VERSION}...");

    let client = reqwest::blocking::Client::new();
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {license_key}"))
        .send()
        .context("Failed to download template")?;

    if !resp.status().is_success() {
        anyhow::bail!("Template download failed: HTTP {}", resp.status());
    }

    let bytes = resp.bytes().context("Failed to read template data")?;
    let gz = GzDecoder::new(bytes.as_ref());
    let mut archive = tar::Archive::new(gz);

    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create cache dir: {}", dest.display()))?;

    archive
        .unpack(dest)
        .context("Failed to extract template archive")?;

    println!("  ✓ Template cached at {}", dest.display());
    Ok(())
}

fn cache_dir() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".cache"))
                .unwrap_or_else(|| PathBuf::from("."))
        })
        .join("crabyard")
        .join("templates")
}
