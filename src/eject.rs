use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use colored::Colorize;
use walkdir::WalkDir;

pub struct Config {
    pub project: Option<PathBuf>,
    pub dry_run: bool,
}

const CRABYARD_FILES: &[&str] = &[
    "manifest.toml",
    "MEMORY.md",
];

const SKIP_DIRS: &[&str] = &[
    "node_modules", "target", ".git", "dist", ".cache",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "ts", "tsx", "js", "jsx", "json", "toml", "yaml", "yml",
    "rs", "css", "html", "md", "svg", "sh", "ps1",
];

pub fn run(config: Config) -> Result<()> {
    let project = config.project.unwrap_or_else(|| PathBuf::from("."));
    let project = fs::canonicalize(&project)
        .with_context(|| format!("Cannot resolve project path: {}", project.display()))?;

    if !project.join("src-tauri").exists() {
        anyhow::bail!(
            "Not a Tauri project. Run this command from the root of a Crabyard project."
        );
    }

    if !project.join("manifest.toml").exists() {
        anyhow::bail!(
            "No manifest.toml found — project may already be ejected."
        );
    }

    println!();
    println!(
        "  {} {}",
        "Ejecting project:".truecolor(161, 161, 170),
        project.display().to_string().truecolor(228, 228, 231)
    );
    println!();

    let mut removed_files = Vec::new();
    let mut cleaned_files = Vec::new();

    for filename in CRABYARD_FILES {
        let path = project.join(filename);
        if path.exists() {
            if config.dry_run {
                println!(
                    "  {} would remove {}",
                    "dry-run:".truecolor(6, 182, 212),
                    filename.truecolor(228, 228, 231)
                );
            } else {
                fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
            }
            removed_files.push(filename.to_string());
        }
    }

    for entry in WalkDir::new(&project)
        .into_iter()
        .filter_entry(|e| {
            !e.file_name()
                .to_str()
                .map(|s| SKIP_DIRS.contains(&s))
                .unwrap_or(false)
        })
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let ext_match = entry
            .path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| TEXT_EXTENSIONS.contains(&e))
            .unwrap_or(false);

        if !ext_match {
            continue;
        }

        let content = match fs::read_to_string(entry.path()) {
            Ok(c) => c,
            Err(_) => continue,
        };

        if !content.contains("CRABYARD:") {
            continue;
        }

        let cleaned = strip_marker_lines(&content);
        if cleaned != content {
            let rel = entry.path().strip_prefix(&project).unwrap_or(entry.path());
            if config.dry_run {
                println!(
                    "  {} would clean markers from {}",
                    "dry-run:".truecolor(6, 182, 212),
                    rel.display().to_string().truecolor(228, 228, 231)
                );
            } else {
                fs::write(entry.path(), &cleaned)
                    .with_context(|| format!("Failed to write {}", entry.path().display()))?;
            }
            cleaned_files.push(rel.display().to_string());
        }
    }

    println!();
    if config.dry_run {
        println!(
            "  {} {} file(s) to remove, {} file(s) to clean",
            "Summary:".truecolor(161, 161, 170),
            removed_files.len(),
            cleaned_files.len()
        );
        println!(
            "  Run without {} to apply changes.",
            "--dry-run".truecolor(228, 228, 231)
        );
    } else {
        println!(
            "  {} Removed {} Crabyard file(s), cleaned markers from {} file(s).",
            "✓".truecolor(34, 197, 94),
            removed_files.len(),
            cleaned_files.len()
        );
        println!(
            "  {}",
            "Project ejected — Crabyard metadata has been removed.".truecolor(113, 113, 122)
        );
    }
    println!();

    Ok(())
}

fn strip_marker_lines(content: &str) -> String {
    content
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            !is_crabyard_marker(trimmed)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_crabyard_marker(trimmed: &str) -> bool {
    for prefix in &["// ", "# "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            if rest.trim().starts_with("CRABYARD:") {
                return true;
            }
        }
    }
    false
}
