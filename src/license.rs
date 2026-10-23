use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use serde::Deserialize;

const API_BASE: &str = "https://crabyard-api-production.up.railway.app";
const TEMPLATE_VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TEMPLATE_BYTES: u64 = 128 * 1024 * 1024;

fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .redirect(reqwest::redirect::Policy::none())
        .build()?)
}

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
    let resp = client()?
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
    validate_download_url(url)?;
    println!("  Downloading template v{TEMPLATE_VERSION}...");

    let resp = client()?
        .get(url)
        .header("Authorization", format!("Bearer {license_key}"))
        .send()
        .context("Failed to download template")?;

    if !resp.status().is_success() {
        anyhow::bail!("Template download failed: HTTP {}", resp.status());
    }

    let mut bytes = Vec::new();
    resp.take(MAX_DOWNLOAD_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("Failed to read template data")?;
    anyhow::ensure!(
        bytes.len() as u64 <= MAX_DOWNLOAD_BYTES,
        "Template download exceeds 32 MiB"
    );

    let parent = dest.parent().context("Template cache has no parent")?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let staging = parent.join(format!(".download-{}-{nonce}", std::process::id()));
    fs::create_dir(&staging)?;
    let result = extract_template(&bytes, &staging)
        .and_then(|()| fs::rename(&staging, dest).context("Failed to publish template cache"));
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    result?;

    println!("  ✓ Template cached at {}", dest.display());
    Ok(())
}

fn validate_download_url(url: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).context("Invalid template download URL")?;
    let expected = reqwest::Url::parse(API_BASE)?;
    anyhow::ensure!(
        parsed.origin() == expected.origin()
            && parsed.username().is_empty()
            && parsed.password().is_none(),
        "Template download must use the Crabyard API origin"
    );
    Ok(())
}

fn extract_template(bytes: &[u8], dest: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(GzDecoder::new(bytes));
    let mut total = 0_u64;
    for entry in archive.entries().context("Invalid template archive")? {
        let mut entry = entry?;
        let kind = entry.header().entry_type();
        anyhow::ensure!(
            kind.is_file() || kind.is_dir(),
            "Template archive contains a link or unsupported entry"
        );
        let path = entry.path()?.into_owned();
        anyhow::ensure!(
            path.components().all(|c| matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )),
            "Template archive contains an unsafe path"
        );
        total = total
            .checked_add(entry.size())
            .context("Template archive size overflow")?;
        anyhow::ensure!(
            total <= MAX_TEMPLATE_BYTES,
            "Expanded template exceeds 128 MiB"
        );
        anyhow::ensure!(
            entry.unpack_in(dest)?,
            "Template entry escapes cache directory"
        );
    }
    anyhow::ensure!(
        dest.join("base").is_dir() && dest.join("manifest.toml").is_file(),
        "Template archive is missing base/ or manifest.toml"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn license_credentials_stay_on_the_api_origin() {
        assert!(validate_download_url(&format!("{API_BASE}/template/1.6.0")).is_ok());
        for url in [
            "http://crabyard-api-production.up.railway.app/template/1.6.0",
            "https://example.com/template",
            "https://crabyard-api-production.up.railway.app.example.com/template",
            "https://user:password@crabyard-api-production.up.railway.app/template",
        ] {
            assert!(validate_download_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn rejects_archive_links_before_extraction() {
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o644);
        header.set_link_name("../../outside").unwrap();
        header.set_cksum();
        tar.append_data(&mut header, "base/link", std::io::empty())
            .unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        std::io::Write::write_all(&mut gzip, &tar.into_inner().unwrap()).unwrap();
        let bytes = gzip.finish().unwrap();
        let result = extract_template(&bytes, Path::new("unused-link-test"));
        assert!(result.unwrap_err().to_string().contains("link"));
    }
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
