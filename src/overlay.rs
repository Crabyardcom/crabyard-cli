use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct ModuleConfig {
    #[serde(default)]
    pub markers: HashMap<String, String>,
    #[serde(default)]
    pub npm_dev_dependencies: HashMap<String, serde_json::Value>,
}

pub fn load_module_config(path: &Path) -> Result<ModuleConfig> {
    if !path.exists() {
        return Ok(ModuleConfig::default());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;
    let config: ModuleConfig = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(config)
}

pub fn apply_markers(content: &str, markers: &HashMap<String, String>) -> String {
    let mut result = Vec::new();
    let mut skip_depth: usize = 0;

    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(marker_name) = extract_marker(trimmed) {
            if let Some(cond_key) = marker_name.strip_prefix("CRABYARD:IF_") {
                let active = markers.get(&format!("CRABYARD:{cond_key}"))
                    .map_or(false, |v| !v.is_empty());
                if !active || skip_depth > 0 {
                    skip_depth += 1;
                }
                continue;
            }
            if let Some(cond_key) = marker_name.strip_prefix("CRABYARD:UNLESS_") {
                let active = markers.get(&format!("CRABYARD:{cond_key}"))
                    .map_or(false, |v| !v.is_empty());
                if active || skip_depth > 0 {
                    skip_depth += 1;
                }
                continue;
            }
            if marker_name == "CRABYARD:ENDIF" {
                if skip_depth > 0 {
                    skip_depth -= 1;
                }
                continue;
            }

            if skip_depth > 0 {
                continue;
            }

            if let Some(replacement) = markers.get(marker_name) {
                if !replacement.is_empty() {
                    let indent = &line[..line.len() - trimmed.len()];
                    let mut first = true;
                    for rep_line in replacement.lines() {
                        if first {
                            result.push(format!("{}{}", indent, rep_line));
                            first = false;
                        } else {
                            result.push(rep_line.to_string());
                        }
                    }
                }
            }
        } else if skip_depth == 0 {
            result.push(line.to_string());
        }
    }
    let mut out = result.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn extract_marker(trimmed: &str) -> Option<&str> {
    for prefix in &["// ", "# "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let rest = rest.trim();
            if rest.starts_with("CRABYARD:") {
                return Some(rest);
            }
        }
    }
    None
}

pub fn merge_package_deps(
    package_json_path: &Path,
    configs: &[&ModuleConfig],
) -> Result<()> {
    if !package_json_path.exists() {
        return Ok(());
    }
    let content = fs::read_to_string(package_json_path)?;
    let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

    for config in configs {
        if !config.npm_dev_dependencies.is_empty() {
            let obj = pkg
                .as_object_mut()
                .context("package.json is not a JSON object")?;
            let dev_deps = obj
                .entry("devDependencies")
                .or_insert_with(|| serde_json::json!({}));
            for (k, v) in &config.npm_dev_dependencies {
                dev_deps[k] = v.clone();
            }
        }
    }

    let output = serde_json::to_string_pretty(&pkg)?;
    fs::write(package_json_path, output)?;
    Ok(())
}
