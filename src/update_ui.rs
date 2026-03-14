use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use walkdir::WalkDir;

use crate::overlay;
use crate::tokens;
use crate::license;

const UI_OPTIONS: &[&str] = &["shadcn", "daisyui", "tesign", "minimal"];

pub struct Config {
    pub switch: Option<String>,
    pub template: Option<PathBuf>,
    pub license_key: Option<String>,
    pub force: bool,
    pub dry_run: bool,
    pub rollback: bool,
}

struct ProjectManifest {
    current_ui: String,
    previous_ui: Option<String>,
}

pub fn run(config: Config) -> Result<()> {
    let project_dir = std::env::current_dir().context("Cannot determine current directory")?;
    let manifest_path = project_dir.join("manifest.toml");

    if !manifest_path.exists() {
        anyhow::bail!(
            "No manifest.toml found in current directory.\n\
             Run this command from the root of a Crabyard project."
        );
    }

    let manifest = read_manifest(&manifest_path)?;

    let target_ui = if config.rollback {
        match &manifest.previous_ui {
            Some(prev) => {
                println!(
                    "\n  {} rolling back to {}",
                    "Rollback:".truecolor(161, 161, 170).bold(),
                    prev.truecolor(6, 182, 212).bold(),
                );
                prev.clone()
            }
            None => anyhow::bail!("No previous UI framework to roll back to."),
        }
    } else {
        match &config.switch {
            Some(ui) => {
                if !UI_OPTIONS.contains(&ui.as_str()) {
                    anyhow::bail!(
                        "Invalid UI framework '{ui}'. Options: {}",
                        UI_OPTIONS.join(", ")
                    );
                }
                ui.clone()
            }
            None => manifest.current_ui.clone(),
        }
    };

    let switching = target_ui != manifest.current_ui;

    println!();
    if switching {
        println!(
            "  {} {} → {}",
            "Switching UI:".truecolor(161, 161, 170).bold(),
            manifest.current_ui.truecolor(113, 113, 122),
            target_ui.truecolor(6, 182, 212).bold(),
        );
    } else {
        println!(
            "  {} {}",
            "Updating UI:".truecolor(161, 161, 170).bold(),
            target_ui.truecolor(6, 182, 212).bold(),
        );
    }

    let mut template = resolve_template(config.template.clone(), config.license_key.as_deref())?;
    let mut ui_dir = template.join("ui").join(&target_ui);
    if !ui_dir.is_dir() {
        if config.template.is_none() {
            if let Some(key) = config.license_key.as_deref() {
                template = license::refresh_cache(key)?;
                ui_dir = template.join("ui").join(&target_ui);
            }
        }
        if !ui_dir.is_dir() {
            anyhow::bail!(
                "UI overlay '{}' not found in template at {}.\n\
                 Delete {} and re-run to refresh the cache.",
                target_ui,
                ui_dir.display(),
                template.display()
            );
        }
    }

    let changes = compute_changes(&ui_dir, &project_dir)?;

    if changes.is_empty() {
        println!(
            "\n  {} All UI files are already up to date.",
            "✓".truecolor(34, 197, 94).bold()
        );
        return Ok(());
    }

    print_changes(&changes);

    let modified_count = changes.iter().filter(|c| c.kind == ChangeKind::Modified).count();
    if modified_count > 0 && !config.force && !config.dry_run {
        println!(
            "\n  {} {} file(s) have local modifications that will be overwritten.",
            "⚠".truecolor(234, 179, 8).bold(),
            modified_count,
        );
        println!(
            "  Use {} to overwrite, or {} to preview.",
            "--force".truecolor(6, 182, 212),
            "--dry-run".truecolor(6, 182, 212),
        );
        anyhow::bail!("Aborted — use --force to overwrite local changes.");
    }

    if config.dry_run {
        println!(
            "\n  {} Dry run complete — no files were changed.",
            "ℹ".truecolor(6, 182, 212).bold()
        );
        return Ok(());
    }

    if switching {
        remove_old_overlay_files(&template.join("ui").join(&manifest.current_ui), &project_dir)?;
    }

    apply_overlay(&ui_dir, &project_dir)?;

    let ui_config = overlay::load_module_config(&ui_dir.join("module.json"))?;
    let auth_module = manifest_auth(&manifest_path)?;
    let auth_dir = template.join("auth").join(&auth_module);
    let auth_config = overlay::load_module_config(&auth_dir.join("module.json"))?;

    overlay::merge_package_deps(
        &project_dir.join("package.json"),
        &[&auth_config, &ui_config],
    )?;

    if switching {
        set_manifest_previous_ui(&manifest_path, &manifest.current_ui)?;
        update_manifest_ui(&manifest_path, &manifest.current_ui, &target_ui)?;
    }

    let applied = changes.len();
    println!(
        "\n  {} {} file(s) updated.",
        "✓".truecolor(34, 197, 94).bold(),
        applied
    );

    println!(
        "\n  {} Run your package manager's install command to sync dependencies.",
        "→".truecolor(6, 182, 212).bold(),
    );
    println!();

    Ok(())
}

#[derive(PartialEq)]
enum ChangeKind {
    Added,
    Modified,
    Unchanged,
}

struct FileChange {
    rel_path: String,
    kind: ChangeKind,
}

fn compute_changes(overlay_dir: &Path, project_dir: &Path) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();

    for entry in WalkDir::new(overlay_dir).into_iter().filter_map(|e| e.ok()) {
        let src = entry.path();
        let rel = src.strip_prefix(overlay_dir)?;

        if tokens::should_skip_path(rel) {
            continue;
        }
        if rel.file_name().map(|f| f == "module.json").unwrap_or(false) {
            continue;
        }
        if entry.file_type().is_dir() {
            continue;
        }

        let dst = project_dir.join(rel);
        let rel_str = rel.to_string_lossy().replace('\\', "/");

        let kind = if !dst.exists() {
            ChangeKind::Added
        } else if tokens::is_binary_path(src) {
            let src_bytes = fs::read(src).unwrap_or_default();
            let dst_bytes = fs::read(&dst).unwrap_or_default();
            if src_bytes == dst_bytes {
                ChangeKind::Unchanged
            } else {
                ChangeKind::Modified
            }
        } else {
            let src_content = fs::read_to_string(src).unwrap_or_default();
            let dst_content = fs::read_to_string(&dst).unwrap_or_default();
            if src_content == dst_content {
                ChangeKind::Unchanged
            } else {
                ChangeKind::Modified
            }
        };

        if kind != ChangeKind::Unchanged {
            changes.push(FileChange {
                rel_path: rel_str,
                kind,
            });
        }
    }

    Ok(changes)
}

fn print_changes(changes: &[FileChange]) {
    println!();
    for change in changes {
        let (icon, style) = match change.kind {
            ChangeKind::Added => ("+", colored::Color::Green),
            ChangeKind::Modified => ("~", colored::Color::Yellow),
            ChangeKind::Unchanged => (" ", colored::Color::White),
        };
        println!("  {} {}", icon.color(style).bold(), change.rel_path.color(style));
    }
}

fn apply_overlay(overlay_dir: &Path, project_dir: &Path) -> Result<()> {
    for entry in WalkDir::new(overlay_dir).into_iter().filter_map(|e| e.ok()) {
        let src = entry.path();
        let rel = src.strip_prefix(overlay_dir)?;

        if tokens::should_skip_path(rel) {
            continue;
        }
        if rel.file_name().map(|f| f == "module.json").unwrap_or(false) {
            continue;
        }

        let dst = project_dir.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst)
                .with_context(|| format!("Failed to create directory: {}", dst.display()))?;
            continue;
        }

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(src, &dst)
            .with_context(|| format!("Failed to copy: {}", src.display()))?;
    }
    Ok(())
}

fn remove_old_overlay_files(old_overlay_dir: &Path, project_dir: &Path) -> Result<()> {
    if !old_overlay_dir.is_dir() {
        return Ok(());
    }

    let mut files_to_remove = Vec::new();

    for entry in WalkDir::new(old_overlay_dir).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry.path().strip_prefix(old_overlay_dir)?;
        if rel.file_name().map(|f| f == "module.json").unwrap_or(false) {
            continue;
        }
        files_to_remove.push(project_dir.join(rel));
    }

    for file in &files_to_remove {
        if file.exists() {
            fs::remove_file(file)
                .with_context(|| format!("Failed to remove: {}", file.display()))?;
        }
    }

    Ok(())
}

fn read_manifest(path: &Path) -> Result<ProjectManifest> {
    let content = fs::read_to_string(path).context("Failed to read manifest.toml")?;
    let ui = extract_toml_value(&content, "modules.ui", "selected")
        .context("manifest.toml missing [modules.ui] selected")?;
    let previous_ui = extract_toml_value(&content, "modules.ui", "previous");
    Ok(ProjectManifest { current_ui: ui, previous_ui })
}

fn set_manifest_previous_ui(path: &Path, previous: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let key_line = format!("previous = \"{}\"", previous);

    if let Some(existing) = extract_toml_value(&content, "modules.ui", "previous") {
        let old = format!("previous = \"{}\"", existing);
        let updated = content.replace(&old, &key_line);
        fs::write(path, updated)?;
    } else {
        let section = "[modules.ui]";
        if let Some(pos) = content.find(section) {
            let insert_pos = pos + section.len();
            let mut new_content = String::with_capacity(content.len() + key_line.len() + 2);
            new_content.push_str(&content[..insert_pos]);
            new_content.push('\n');
            new_content.push_str(&key_line);
            new_content.push_str(&content[insert_pos..]);
            fs::write(path, new_content)?;
        }
    }
    Ok(())
}

fn manifest_auth(path: &Path) -> Result<String> {
    let content = fs::read_to_string(path).context("Failed to read manifest.toml")?;
    Ok(extract_toml_value(&content, "modules.auth", "selected")
        .unwrap_or_else(|| "none".into()))
}

fn extract_toml_value(content: &str, section: &str, key: &str) -> Option<String> {
    let section_header = format!("[{}]", section);
    let mut in_section = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed == section_header;
            continue;
        }
        if in_section {
            if let Some(rest) = trimmed.strip_prefix(key) {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let val = rest.trim().trim_matches('"');
                    return Some(val.to_string());
                }
            }
        }
    }
    None
}

fn update_manifest_ui(path: &Path, old_ui: &str, new_ui: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;

    // Targeted replace within [modules.ui] section to avoid touching [modules.auth]
    let section_marker = "[modules.ui]";
    if let Some(section_pos) = content.find(section_marker) {
        let after_section = &content[section_pos + section_marker.len()..];
        let old_line = format!("selected = \"{}\"", old_ui);
        if let Some(key_offset) = after_section.find(&old_line) {
            let abs_pos = section_pos + section_marker.len() + key_offset;
            let mut new_content = String::with_capacity(content.len());
            new_content.push_str(&content[..abs_pos]);
            new_content.push_str(&format!("selected = \"{}\"", new_ui));
            new_content.push_str(&content[abs_pos + old_line.len()..]);
            fs::write(path, new_content)?;
            return Ok(());
        }
    }

    let updated = content.replace(
        &format!("selected = \"{}\"", old_ui),
        &format!("selected = \"{}\"", new_ui),
    );
    fs::write(path, updated)?;
    Ok(())
}

fn resolve_template(explicit: Option<PathBuf>, license_key: Option<&str>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p);
        }
        anyhow::bail!("Template directory does not exist: {}", p.display());
    }

    if let Some(key) = license_key {
        return license::validate_and_resolve(key);
    }

    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok();
    if let Some(home) = home {
        let default = PathBuf::from(home).join(".crabyard").join("templates");
        if default.exists() {
            return Ok(default);
        }
    }

    // Check cache directory for any existing template
    let cache = dirs::cache_dir()
        .unwrap_or_else(|| {
            dirs::home_dir()
                .map(|h| h.join(".cache"))
                .unwrap_or_else(|| PathBuf::from("."))
        })
        .join("crabyard")
        .join("templates");

    if cache.is_dir() {
        // Use the latest cached version
        let mut versions: Vec<_> = fs::read_dir(&cache)?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join("base").is_dir())
            .collect();
        versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
        if let Some(latest) = versions.first() {
            return Ok(latest.path());
        }
    }

    anyhow::bail!(
        "No template found.\n\
         Options:\n  \
         1. Set CRABYARD_LICENSE_KEY (downloads latest template)\n  \
         2. Use --template /path/to/scaffold\n  \
         3. Run 'crabyard new' first to cache a template"
    )
}
