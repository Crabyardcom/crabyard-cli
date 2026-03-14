use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;
use walkdir::WalkDir;

use crate::tokens;

pub struct Config {
    pub template: Option<PathBuf>,
    pub license_key: Option<String>,
    pub force: bool,
    pub dry_run: bool,
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

    let manifest_content = fs::read_to_string(&manifest_path).context("Failed to read manifest.toml")?;
    let project_version = extract_toml_value(&manifest_content, "template", "version")
        .unwrap_or_else(|| "unknown".into());
    let auth_module = extract_toml_value(&manifest_content, "modules.auth", "selected")
        .unwrap_or_else(|| "none".into());
    let ui_module = extract_toml_value(&manifest_content, "modules.ui", "selected")
        .unwrap_or_else(|| "shadcn".into());

    let template = crate::generate::resolve_template_path(
        config.template,
        config.license_key.as_deref(),
    )?;

    let template_manifest = template.join("base").join("manifest.toml");
    let template_version = if template_manifest.exists() {
        let content = fs::read_to_string(&template_manifest)?;
        extract_toml_value(&content, "template", "version").unwrap_or_else(|| "unknown".into())
    } else {
        "unknown".into()
    };

    println!();
    println!(
        "  {} project={}, template={}",
        "Upgrade check:".truecolor(161, 161, 170).bold(),
        project_version.truecolor(113, 113, 122),
        template_version.truecolor(6, 182, 212).bold(),
    );
    println!(
        "  auth={}, ui={}",
        auth_module.truecolor(6, 182, 212),
        ui_module.truecolor(6, 182, 212),
    );
    println!();

    let base_dir = template.join("base");
    let auth_dir = template.join("auth").join(&auth_module);
    let ui_dir = template.join("ui").join(&ui_module);

    let mut diffs = Vec::new();
    collect_diffs(&base_dir, &project_dir, &mut diffs)?;
    if auth_dir.is_dir() {
        collect_diffs(&auth_dir, &project_dir, &mut diffs)?;
    }
    if ui_dir.is_dir() {
        collect_diffs(&ui_dir, &project_dir, &mut diffs)?;
    }

    let outdated: Vec<_> = diffs.iter().filter(|d| d.kind != DiffKind::Unchanged).collect();

    if outdated.is_empty() {
        println!(
            "  {} All files are up to date with the template.",
            "✓".truecolor(34, 197, 94).bold()
        );
        println!();
        return Ok(());
    }

    for d in &outdated {
        let (icon, color) = match d.kind {
            DiffKind::Added => ("+", colored::Color::Green),
            DiffKind::Modified => ("~", colored::Color::Yellow),
            DiffKind::Missing => ("!", colored::Color::Red),
            DiffKind::Unchanged => (" ", colored::Color::White),
        };
        println!("  {} {}", icon.color(color).bold(), d.rel_path.color(color));
    }

    let added = outdated.iter().filter(|d| d.kind == DiffKind::Added).count();
    let modified = outdated.iter().filter(|d| d.kind == DiffKind::Modified).count();
    let missing = outdated.iter().filter(|d| d.kind == DiffKind::Missing).count();

    println!();
    println!(
        "  {} {} new, {} modified, {} missing from template",
        "Summary:".truecolor(161, 161, 170),
        added, modified, missing,
    );

    if config.dry_run {
        println!(
            "\n  {} Dry run — no files modified.",
            "ℹ".truecolor(6, 182, 212).bold()
        );
        println!();
        return Ok(());
    }

    if modified > 0 && !config.force {
        println!(
            "\n  {} {} file(s) have local modifications that will be overwritten.",
            "⚠".truecolor(234, 179, 8).bold(),
            modified,
        );
        println!(
            "  Use {} to overwrite, or {} to preview.",
            "--force".truecolor(6, 182, 212),
            "--dry-run".truecolor(6, 182, 212),
        );
        anyhow::bail!("Aborted — use --force to overwrite local changes.");
    }

    let mut applied = 0u32;
    for d in &outdated {
        if d.kind == DiffKind::Missing {
            continue;
        }
        let dst = project_dir.join(&d.rel_path);
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&d.template_path, &dst)
            .with_context(|| format!("Failed to copy: {}", d.rel_path))?;
        applied += 1;
    }

    if project_version != template_version {
        update_template_version(&manifest_path, &project_version, &template_version)?;
    }

    println!(
        "\n  {} {} file(s) updated to template v{}.",
        "✓".truecolor(34, 197, 94).bold(),
        applied,
        template_version,
    );
    println!(
        "\n  {} Run your package manager's install command to sync dependencies.",
        "→".truecolor(6, 182, 212).bold(),
    );
    println!();

    Ok(())
}

#[derive(PartialEq)]
enum DiffKind {
    Added,
    Modified,
    Missing,
    Unchanged,
}

struct FileDiff {
    rel_path: String,
    template_path: PathBuf,
    kind: DiffKind,
}

fn collect_diffs(
    overlay_dir: &Path,
    project_dir: &Path,
    diffs: &mut Vec<FileDiff>,
) -> Result<()> {
    for entry in WalkDir::new(overlay_dir).into_iter().filter_map(|e| e.ok()) {
        let src = entry.path();
        let rel = src.strip_prefix(overlay_dir)?;

        if tokens::should_skip_path(rel) || rel.as_os_str().is_empty() {
            continue;
        }
        if rel.file_name().map(|f| f == "module.json").unwrap_or(false) {
            continue;
        }
        if !entry.file_type().is_file() {
            continue;
        }

        let rel_str = rel.display().to_string().replace('\\', "/");
        if rel_str == "manifest.toml" {
            continue;
        }

        let dst = project_dir.join(rel);

        let kind = if !dst.exists() {
            DiffKind::Added
        } else if tokens::is_binary_path(src) {
            let src_bytes = fs::read(src).unwrap_or_default();
            let dst_bytes = fs::read(&dst).unwrap_or_default();
            if src_bytes == dst_bytes { DiffKind::Unchanged } else { DiffKind::Modified }
        } else {
            let src_content = fs::read_to_string(src).unwrap_or_default();
            let dst_content = fs::read_to_string(&dst).unwrap_or_default();
            if src_content == dst_content { DiffKind::Unchanged } else { DiffKind::Modified }
        };

        diffs.push(FileDiff {
            rel_path: rel_str,
            template_path: src.to_path_buf(),
            kind,
        });
    }
    Ok(())
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
                    return Some(rest.trim().trim_matches('"').to_string());
                }
            }
        }
    }
    None
}

fn update_template_version(path: &Path, old: &str, new: &str) -> Result<()> {
    let content = fs::read_to_string(path)?;
    let updated = content.replace(
        &format!("version = \"{}\"", old),
        &format!("version = \"{}\"", new),
    );
    fs::write(path, updated)?;
    Ok(())
}
