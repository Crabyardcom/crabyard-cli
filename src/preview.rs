use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use walkdir::WalkDir;

use crate::tokens;

pub struct Config {
    pub template: Option<PathBuf>,
    pub auth: String,
    pub ui: String,
    pub license_key: Option<String>,
}

pub fn run(config: Config) -> Result<()> {
    let template = crate::generate::resolve_template_path(
        config.template,
        config.license_key.as_deref(),
    )?;

    let base_dir = template.join("base");
    if !base_dir.is_dir() {
        anyhow::bail!("Template missing 'base/' directory at {}", base_dir.display());
    }

    let auth_dir = template.join("auth").join(&config.auth);
    let ui_dir = template.join("ui").join(&config.ui);

    println!();
    println!(
        "  {} auth={}, ui={}",
        "Preview:".truecolor(161, 161, 170),
        config.auth.truecolor(6, 182, 212).bold(),
        config.ui.truecolor(6, 182, 212).bold()
    );
    println!();

    let mut files: Vec<String> = Vec::new();

    collect_files(&base_dir, &mut files)?;
    if auth_dir.is_dir() {
        collect_files(&auth_dir, &mut files)?;
    }
    if ui_dir.is_dir() {
        collect_files(&ui_dir, &mut files)?;
    }

    files.sort();
    files.dedup();

    let mut dirs_printed: std::collections::HashSet<String> = std::collections::HashSet::new();

    for file in &files {
        let path = Path::new(file);
        if let Some(parent) = path.parent() {
            let dir = parent.display().to_string();
            if !dir.is_empty() && dirs_printed.insert(dir.clone()) {
                println!(
                    "  {}",
                    format!("{}/", dir).truecolor(6, 182, 212)
                );
            }
        }
        let name = path.file_name().map(|f| f.to_string_lossy()).unwrap_or_default();
        let dir_prefix = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let indent = if dir_prefix.is_empty() { "  " } else { "    " };
        println!(
            "{}{}",
            indent,
            name.truecolor(228, 228, 231)
        );
    }

    println!();
    println!(
        "  {} {} file(s) would be generated",
        "Total:".truecolor(161, 161, 170),
        files.len()
    );
    println!();

    Ok(())
}

fn collect_files(source: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in WalkDir::new(source)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let src = entry.path();
        let rel = src.strip_prefix(source)
            .context("Failed to strip prefix")?;

        if tokens::should_skip_path(rel) {
            continue;
        }

        if rel.file_name().map(|f| f == "module.json").unwrap_or(false) {
            continue;
        }

        if !entry.file_type().is_file() {
            continue;
        }

        let rel_str = rel.display().to_string().replace('\\', "/");
        if !rel_str.is_empty() {
            files.push(rel_str);
        }
    }
    Ok(())
}
