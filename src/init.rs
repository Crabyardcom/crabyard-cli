use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use colored::Colorize;
use dialoguer::{Confirm, Select};

pub struct Config {
    pub project: Option<PathBuf>,
}

pub fn run(config: Config) -> Result<()> {
    let project_dir = config.project.unwrap_or_else(|| PathBuf::from("."));
    let project_dir = fs::canonicalize(&project_dir)
        .with_context(|| format!("Cannot access {}", project_dir.display()))?;

    let manifest_path = project_dir.join("manifest.toml");
    if manifest_path.exists() {
        bail!("This project is already managed by Crabyard (manifest.toml exists)");
    }

    let tauri_conf = project_dir.join("src-tauri").join("tauri.conf.json");
    if !tauri_conf.exists() {
        bail!(
            "No src-tauri/tauri.conf.json found — run this command from a Tauri v2 project root"
        );
    }

    println!(
        "{} Initializing Crabyard in {}",
        "▸".cyan(),
        project_dir.display()
    );

    let (app_name, bundle_id) = read_tauri_conf(&tauri_conf)?;
    let slug = slug_from_name(&app_name);

    let auth = select_option("Auth module", &["none", "github", "google"])?;
    let ui = select_option("UI framework", &["shadcn", "daisyui", "tesign", "minimal"])?;
    let pm = detect_pm(&project_dir);

    let manifest = format!(
        r#"[template]
version = "1.0.0"

[project]
app_name = "{app_name}"
app_slug = "{slug}"
app_bundle_id = "{bundle_id}"

[modules.auth]
selected = "{auth}"

[modules.ui]
selected = "{ui}"

[modules.package_manager]
selected = "{pm}"
"#
    );

    if !Confirm::new()
        .with_prompt("Write manifest.toml?")
        .default(true)
        .interact()?
    {
        println!("  Aborted.");
        return Ok(());
    }

    fs::write(&manifest_path, &manifest)?;
    println!(
        "  {} Created {}",
        "✓".green(),
        manifest_path.display()
    );
    println!();
    println!(
        "  You can now use {} and {} in this project.",
        "crabyard update-ui".cyan(),
        "crabyard add".cyan()
    );

    Ok(())
}

fn read_tauri_conf(path: &Path) -> Result<(String, String)> {
    let content = fs::read_to_string(path)?;
    let v: serde_json::Value = serde_json::from_str(&content)?;
    let name = v
        .pointer("/productName")
        .or_else(|| v.pointer("/package/productName"))
        .and_then(|v| v.as_str())
        .unwrap_or("my-app")
        .to_string();
    let bundle = v
        .pointer("/identifier")
        .or_else(|| v.pointer("/bundle/identifier"))
        .and_then(|v| v.as_str())
        .unwrap_or("com.example.app")
        .to_string();
    Ok((name, bundle))
}

fn slug_from_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

fn select_option(label: &str, options: &[&str]) -> Result<String> {
    let idx = Select::new()
        .with_prompt(label)
        .items(options)
        .default(0)
        .interact()?;
    Ok(options[idx].to_string())
}

fn detect_pm(dir: &Path) -> String {
    if dir.join("bun.lockb").exists() || dir.join("bun.lock").exists() {
        "bun".into()
    } else if dir.join("pnpm-lock.yaml").exists() {
        "pnpm".into()
    } else if dir.join("yarn.lock").exists() {
        "yarn".into()
    } else {
        "npm".into()
    }
}
