use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde_json::Value;

use crate::check::{self, Severity};
use crate::project;
use crate::recipe::Recipe;

pub struct Config {
    pub project: PathBuf,
    pub brief: String,
    pub output: PathBuf,
    pub dry_run: bool,
}

pub fn run(config: Config) -> Result<()> {
    run_mode(config, false)
}

pub fn run_silent(config: Config) -> Result<()> {
    run_mode(config, true)
}

fn run_mode(config: Config, quiet: bool) -> Result<()> {
    ensure!(
        !config.brief.trim().is_empty(),
        "Provide a product brief with --brief"
    );
    ensure!(
        config.brief.len() <= 16 * 1024,
        "The product brief must be at most 16 KiB"
    );
    let root = fs::canonicalize(&config.project).context("Cannot access the project directory")?;
    let recipe_text = project::read_text(&root, "crabyard.recipe.toml")?
        .context("No crabyard.recipe.toml found in the project")?;
    let recipe: Recipe = toml::from_str(&recipe_text)
        .map_err(|_| anyhow::anyhow!("The project recipe is invalid"))?;
    recipe
        .validate()
        .context("The project recipe is not supported")?;
    let package = project::read_text(&root, "package.json")?
        .context("No package.json found in the project")?;
    let package: Value =
        serde_json::from_str(&package).map_err(|_| anyhow::anyhow!("package.json is invalid"))?;
    ensure!(
        !config.output.is_absolute()
            && config
                .output
                .components()
                .all(|part| matches!(part, Component::Normal(_) | Component::CurDir)),
        "--output must be a relative path inside the project"
    );
    ensure!(
        config.output.extension().is_some_and(|ext| ext == "md"),
        "--output must name a Markdown (.md) file"
    );
    let output = root.join(&config.output);
    let parent = output.parent().context("Output has no parent directory")?;
    for ancestor in parent.ancestors() {
        if ancestor.exists() {
            ensure!(
                fs::canonicalize(ancestor)?.starts_with(&root),
                "Output directory points outside the project"
            );
            break;
        }
    }
    let report = check::audit(&root, false);
    let document = render(&root, &recipe, &package, &config.brief, &report);
    if config.dry_run {
        print!("{document}");
        return Ok(());
    }
    ensure!(
        output.symlink_metadata().is_err(),
        "Output already exists; choose a new --output path or use --dry-run"
    );
    fs::create_dir_all(parent).context("Cannot create the context output directory")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .context("Cannot create context file without overwriting existing content")?;
    file.write_all(document.as_bytes())
        .context("Cannot write the AI context file")?;
    if !quiet {
        println!("Created {}", config.output.display());
        println!("Open it in your coding assistant, then implement the first acceptance slice.");
    }
    Ok(())
}

fn render(
    root: &Path,
    recipe: &Recipe,
    package: &Value,
    brief: &str,
    report: &check::Report,
) -> String {
    let app_name = redact(&recipe.app_name);
    let mut output = format!("# {app_name}: AI implementation context\n\n## Product brief\n\n{}\n\n## Project contract\n\n- App: {app_name}\n- Package: `{}`\n- Bundle identifier: `{}`\n- Version: `{}`\n- Frontend: React + TypeScript, `{}` UI\n- Desktop: Rust + Tauri v2\n- Authentication: `{}`\n- Package manager: `{}`\n- Selected features: {}\n\n",
        redact(brief.trim()), recipe.slug, recipe.bundle_id, recipe.app_version, recipe.ui, recipe.auth, recipe.pm,
        if recipe.extras.is_empty() { "base application".into() } else { recipe.extras.join(", ") });
    output.push_str("## Read these project files first\n\n");
    for path in [
        "AGENTS.md",
        "docs/PROJECT.md",
        "docs/tasks/FEATURE.md",
        "docs/HANDOFF.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".github/copilot-instructions.md",
        "crabyard.recipe.toml",
        "package.json",
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json",
    ] {
        if root.join(path).is_file() {
            output.push_str(&format!("- `{path}`\n"));
        }
    }
    output.push_str("\n## Existing foundations\n\n");
    for (path, purpose) in [
        ("src/components", "application views and reusable UI"),
        (
            "src/hooks",
            "typed frontend behavior and native integration",
        ),
        (
            "src-tauri/src/commands",
            "native IPC commands; validate untrusted inputs here",
        ),
        (
            "src-tauri/src/models",
            "shared domain data and persisted settings",
        ),
        (
            "src-tauri/src/auth",
            "native authentication and secure credential storage",
        ),
        ("src-tauri/capabilities", "explicit native permissions"),
        ("src/test", "frontend behavior tests"),
    ] {
        if root.join(path).is_dir() {
            output.push_str(&format!("- `{path}`: {purpose}.\n"));
        }
    }
    output.push_str("\nDirect frontend dependencies: ");
    let names: Vec<_> = package
        .get("dependencies")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(name, _)| safe_package_name(name))
        .map(|(name, _)| format!("`{name}`"))
        .collect();
    output.push_str(&names.join(", "));
    output.push_str(". Versions and private registry details stay in package.json.\n\n");
    output.push_str("## First implementation slice\n\n1. Read the project instructions and translate the brief into a small set of user-visible acceptance criteria. Record unknown product decisions before changing architecture.\n2. Reuse the existing views, typed IPC boundary, settings, and persistence. Add one complete user flow with loading, empty, success, and error states.\n3. Keep secrets out of frontend environment variables. Validate native command arguments, keep capabilities narrow, and handle persistence failures explicitly.\n4. Add behavior tests for the acceptance criteria, run the verification commands below, and update docs/HANDOFF.md with decisions and remaining work.\n\n");
    match recipe.auth.as_str() {
        "github" => output.push_str("Authentication needs the public GITHUB_CLIENT_ID configured locally. Never request or embed a desktop client secret.\n\n"),
        "google" => output.push_str("Authentication needs the public GOOGLE_CLIENT_ID configured locally. Keep PKCE and native secure credential storage. Never embed a desktop client secret.\n\n"),
        _ => {}
    }
    output.push_str("## Verification commands\n\n```sh\n");
    for script in ["check", "test", "build"] {
        if package
            .get("scripts")
            .and_then(Value::as_object)
            .is_some_and(|scripts| scripts.contains_key(script))
        {
            let prefix = if recipe.pm == "npm" || recipe.pm == "bun" {
                format!("{} run", recipe.pm)
            } else {
                recipe.pm.clone()
            };
            output.push_str(&format!("{prefix} {script}\n"));
        }
    }
    output.push_str("cargo test --manifest-path src-tauri/Cargo.toml\ncrabyard check --strict\n```\n\nThese commands are a verification plan; their success has not been assumed.\n\n## Configuration findings to resolve\n\n");
    let findings: Vec<_> = report
        .findings
        .iter()
        .filter(|finding| finding.severity != Severity::Pass)
        .collect();
    if findings.is_empty() {
        output.push_str("The configuration audit found no current blockers. Run the behavior tests and desktop build before delivery.\n");
    } else {
        for finding in findings {
            let level = if finding.severity == Severity::Error {
                "ERROR"
            } else {
                "WARN"
            };
            output.push_str(&format!(
                "- [{level}] `{}` / `{}`: {} {}\n",
                finding.id, finding.path, finding.message, finding.action
            ));
        }
    }
    output.push_str("\nThis context includes selected project metadata and file references. Environment files, credential stores, signing material, source-file bodies, and dependency URL values are excluded.\n");
    output
}

fn safe_package_name(name: &str) -> bool {
    name.len() <= 214
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"@/._-".contains(&byte))
}

fn redact(text: &str) -> String {
    let mut in_private_key = false;
    text.lines()
        .map(|line| {
            if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
                in_private_key = true;
                return "[private key omitted]".into();
            }
            if in_private_key {
                if line.contains("-----END") {
                    in_private_key = false;
                }
                return String::new();
            }
            let upper = line.to_uppercase();
            if let Some((key, _)) = line.split_once('=').or_else(|| line.split_once(':')) {
                let key = key.to_uppercase();
                if [
                    "SECRET",
                    "PASSWORD",
                    "PRIVATE_KEY",
                    "ACCESS_TOKEN",
                    "REFRESH_TOKEN",
                    "API_KEY",
                    "API-KEY",
                    "LICENSE_KEY",
                    "TOKEN",
                    "AUTHORIZATION",
                    "COOKIE",
                ]
                .iter()
                .any(|word| key.contains(word))
                {
                    return "[sensitive assignment omitted]".into();
                }
            }
            if upper.contains("BEARER ") {
                return "[authorization value omitted]".into();
            }
            line.split_whitespace()
                .map(|word| {
                    if [
                        "sk-",
                        "ghp_",
                        "gho_",
                        "github_pat_",
                        "AKIA",
                        "xoxb-",
                        "xoxp-",
                    ]
                    .iter()
                    .any(|prefix| word.contains(prefix))
                    {
                        "[credential omitted]".to_string()
                    } else if reqwest::Url::parse(
                        word.trim_matches(['(', ')', '[', ']', '<', '>', '\'', '"', '`', ',']),
                    )
                    .is_ok_and(|url| {
                        !url.username().is_empty()
                            || url.password().is_some()
                            || url.query_pairs().any(|(key, _)| {
                                ["token", "secret", "key", "password"]
                                    .iter()
                                    .any(|term| key.to_lowercase().contains(term))
                            })
                    }) {
                        "[authenticated URL omitted]".to_string()
                    } else {
                        word.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
