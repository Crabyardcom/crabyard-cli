use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::project;
use crate::recipe::Recipe;

pub struct Config {
    pub project: PathBuf,
    pub json: bool,
    pub strict: bool,
}

#[derive(Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Pass,
    Warning,
    Error,
}

#[derive(Serialize)]
pub struct Finding {
    pub id: &'static str,
    pub title: &'static str,
    pub severity: Severity,
    pub path: String,
    pub message: String,
    pub action: String,
}

#[derive(Default, Serialize)]
pub struct Summary {
    pub passed: usize,
    pub warnings: usize,
    pub errors: usize,
}

#[derive(Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub project: String,
    pub strict: bool,
    pub passed: bool,
    pub summary: Summary,
    pub findings: Vec<Finding>,
}

impl Report {
    fn add(
        &mut self,
        id: &'static str,
        title: &'static str,
        severity: Severity,
        path: &str,
        message: &str,
        action: &str,
    ) {
        self.findings.push(Finding {
            id,
            title,
            severity,
            path: path.into(),
            message: message.into(),
            action: action.into(),
        });
    }

    fn finish(mut self) -> Self {
        for finding in &self.findings {
            match finding.severity {
                Severity::Pass => self.summary.passed += 1,
                Severity::Warning => self.summary.warnings += 1,
                Severity::Error => self.summary.errors += 1,
            }
        }
        self.passed = self.summary.errors == 0 && (!self.strict || self.summary.warnings == 0);
        self
    }

    fn read(&mut self, root: &Path, relative: &str) -> Option<String> {
        match project::read_text(root, relative) {
            Ok(value) => value,
            Err(_) => {
                self.add(
                    "file.unreadable",
                    "Project files",
                    Severity::Error,
                    relative,
                    "A project file cannot be safely inspected.",
                    "Use a regular UTF-8 file inside the project, smaller than 16 MiB.",
                );
                None
            }
        }
    }

    fn json(&mut self, root: &Path, relative: &str) -> Option<Value> {
        let Some(text) = self.read(root, relative) else {
            self.add(
                "manifest.missing",
                "Project manifests",
                Severity::Error,
                relative,
                "A required project manifest is missing.",
                "Restore the generated manifest before continuing.",
            );
            return None;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) if value.is_object() => Some(value),
            _ => {
                self.add(
                    "manifest.invalid",
                    "Project manifests",
                    Severity::Error,
                    relative,
                    "The manifest must contain a valid JSON object.",
                    "Correct its JSON syntax and rerun crabyard check.",
                );
                None
            }
        }
    }

    fn toml(&mut self, root: &Path, relative: &str) -> Option<toml::Value> {
        let Some(text) = self.read(root, relative) else {
            self.add(
                "manifest.missing",
                "Project manifests",
                Severity::Error,
                relative,
                "A required project manifest is missing.",
                "Restore the generated manifest before continuing.",
            );
            return None;
        };
        match toml::from_str(&text) {
            Ok(value) => Some(value),
            Err(_) => {
                self.add(
                    "manifest.invalid",
                    "Project manifests",
                    Severity::Error,
                    relative,
                    "The manifest is not valid TOML.",
                    "Correct its TOML syntax and rerun crabyard check.",
                );
                None
            }
        }
    }
}

pub fn run(config: Config) -> Result<bool> {
    let report = audit(&config.project, config.strict);
    if config.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("Crabyard project readiness\n");
        for finding in &report.findings {
            let status = match finding.severity {
                Severity::Pass => "PASS",
                Severity::Warning => "WARN",
                Severity::Error => "FAIL",
            };
            println!("[{status}] {} — {}", finding.title, finding.message);
            if !finding.action.is_empty() {
                println!("       {}: {}", finding.path, finding.action);
            }
        }
        println!(
            "\n{} passed, {} warnings, {} errors",
            report.summary.passed, report.summary.warnings, report.summary.errors
        );
        println!(
            "{}",
            if report.passed {
                "Readiness checks passed."
            } else {
                "Resolve the findings above before delivery."
            }
        );
    }
    Ok(report.passed)
}

pub fn audit(project_path: &Path, strict: bool) -> Report {
    let mut report = Report {
        schema_version: 1,
        project: project_path.display().to_string(),
        strict,
        passed: false,
        summary: Summary::default(),
        findings: Vec::new(),
    };
    let Ok(root) = fs::canonicalize(project_path) else {
        report.add(
            "project.missing",
            "Project directory",
            Severity::Error,
            ".",
            "The project directory cannot be accessed.",
            "Pass a generated project root with --project DIR.",
        );
        return report.finish();
    };
    let recipe = report
        .read(&root, "crabyard.recipe.toml")
        .and_then(|text| toml::from_str::<Recipe>(&text).ok());
    let recipe = match recipe {
        Some(recipe) if recipe.validate().is_ok() => {
            report.add(
                "recipe.valid",
                "Project recipe",
                Severity::Pass,
                "crabyard.recipe.toml",
                "Project choices are valid and reproducible.",
                "",
            );
            Some(recipe)
        }
        _ => {
            report.add(
                "recipe.invalid",
                "Project recipe",
                Severity::Error,
                "crabyard.recipe.toml",
                "The project recipe is missing, invalid, or contains an unsupported choice.",
                "Restore a valid recipe using the supported auth, UI, and package-manager choices.",
            );
            None
        }
    };
    let manifest = report.toml(&root, "manifest.toml");
    let cargo = report.toml(&root, "src-tauri/Cargo.toml");
    let package = report.json(&root, "package.json");
    let tauri = report.json(&root, "src-tauri/tauri.conf.json");
    if let Some(recipe) = &recipe {
        check_identity(
            &mut report,
            recipe,
            manifest.as_ref(),
            cargo.as_ref(),
            package.as_ref(),
            tauri.as_ref(),
        );
    }
    if let (Some(recipe), Some(package)) = (&recipe, &package) {
        check_locks(&mut report, &root, recipe, package, cargo.as_ref());
    }
    check_instructions(&mut report, &root);
    let environment = check_environment(&mut report, &root, recipe.as_ref());
    check_capabilities(&mut report, &root, cargo.as_ref());
    check_delivery(
        &mut report,
        &root,
        recipe.as_ref(),
        package.as_ref(),
        tauri.as_ref(),
        &environment,
    );
    check_gitignore(&mut report, &root);
    report.finish()
}

fn check_identity(
    report: &mut Report,
    recipe: &Recipe,
    manifest: Option<&toml::Value>,
    cargo: Option<&toml::Value>,
    package: Option<&Value>,
    tauri: Option<&Value>,
) {
    let checks = [
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("modules"))
                .and_then(|v| v.get("auth"))
                .and_then(|v| v.get("selected"))
                .and_then(toml::Value::as_str),
            recipe.auth.as_str(),
        ),
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("modules"))
                .and_then(|v| v.get("ui"))
                .and_then(|v| v.get("selected"))
                .and_then(toml::Value::as_str),
            recipe.ui.as_str(),
        ),
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("modules"))
                .and_then(|v| v.get("package_manager"))
                .and_then(|v| v.get("selected"))
                .and_then(toml::Value::as_str),
            recipe.pm.as_str(),
        ),
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("project"))
                .and_then(|v| v.get("app_slug"))
                .and_then(toml::Value::as_str),
            recipe.slug.as_str(),
        ),
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("project"))
                .and_then(|v| v.get("app_bundle_id"))
                .and_then(toml::Value::as_str),
            recipe.bundle_id.as_str(),
        ),
        (
            "manifest.toml",
            manifest
                .and_then(|v| v.get("project"))
                .and_then(|v| v.get("app_version"))
                .and_then(toml::Value::as_str),
            recipe.app_version.as_str(),
        ),
        (
            "src-tauri/Cargo.toml",
            cargo
                .and_then(|v| v.get("package"))
                .and_then(|v| v.get("name"))
                .and_then(toml::Value::as_str),
            recipe.slug.as_str(),
        ),
        (
            "src-tauri/Cargo.toml",
            cargo
                .and_then(|v| v.get("package"))
                .and_then(|v| v.get("version"))
                .and_then(toml::Value::as_str),
            recipe.app_version.as_str(),
        ),
        (
            "package.json",
            package.and_then(|v| v.get("name")).and_then(Value::as_str),
            recipe.slug.as_str(),
        ),
        (
            "package.json",
            package
                .and_then(|v| v.get("version"))
                .and_then(Value::as_str),
            recipe.app_version.as_str(),
        ),
        (
            "src-tauri/tauri.conf.json",
            tauri
                .and_then(|v| v.get("productName"))
                .and_then(Value::as_str),
            recipe.app_name.as_str(),
        ),
        (
            "src-tauri/tauri.conf.json",
            tauri
                .and_then(|v| v.get("identifier"))
                .and_then(Value::as_str),
            recipe.bundle_id.as_str(),
        ),
        (
            "src-tauri/tauri.conf.json",
            tauri.and_then(|v| v.get("version")).and_then(Value::as_str),
            recipe.app_version.as_str(),
        ),
    ];
    if checks
        .iter()
        .any(|(_, actual, expected)| *actual != Some(expected))
    {
        report.add(
            "identity.mismatch",
            "Application identity",
            Severity::Error,
            "crabyard.recipe.toml",
            "Recipe, frontend, Rust, Tauri, and generator manifests disagree.",
            "Synchronize app name, slug, bundle ID, and version across the project manifests.",
        );
    } else {
        report.add(
            "identity.consistent",
            "Application identity",
            Severity::Pass,
            "crabyard.recipe.toml",
            "Application identity agrees across all manifests.",
            "",
        );
    }
    if recipe.bundle_id.starts_with("com.example.")
        || matches!(recipe.app_name.as_str(), "My App" | "Example" | "App")
    {
        report.add(
            "identity.placeholder",
            "Delivery identity",
            Severity::Warning,
            "crabyard.recipe.toml",
            "The app still uses a development identity.",
            "Choose your product name and an owned reverse-DNS bundle ID before distribution.",
        );
    }
}

fn dependency_groups(
    package: &Value,
) -> impl Iterator<Item = (&'static str, BTreeMap<String, String>)> + '_ {
    ["dependencies", "devDependencies", "optionalDependencies"]
        .into_iter()
        .map(|group| {
            let deps = package
                .get(group)
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .filter_map(|(name, version)| {
                    version
                        .as_str()
                        .map(|version| (name.clone(), version.to_string()))
                })
                .collect();
            (group, deps)
        })
}

fn check_locks(
    report: &mut Report,
    root: &Path,
    recipe: &Recipe,
    package: &Value,
    cargo: Option<&toml::Value>,
) {
    if ["dependencies", "devDependencies", "optionalDependencies"]
        .iter()
        .any(|group| {
            package.get(*group).is_some_and(|value| {
                !value.as_object().is_some_and(|entries| {
                    entries
                        .values()
                        .all(|value| value.as_str().is_some_and(|text| !text.is_empty()))
                })
            })
        })
    {
        report.add(
            "dependencies.invalid",
            "Dependency declarations",
            Severity::Error,
            "package.json",
            "Dependency groups must map package names to nonempty version strings.",
            "Correct the dependency declarations before regenerating the lockfile.",
        );
    }
    let expected = match recipe.pm.as_str() {
        "npm" => "package-lock.json",
        "pnpm" => "pnpm-lock.yaml",
        "yarn" => "yarn.lock",
        _ => "bun.lock",
    };
    let locks: Vec<_> = [
        "package-lock.json",
        "pnpm-lock.yaml",
        "yarn.lock",
        "bun.lock",
        "bun.lockb",
    ]
    .into_iter()
    .filter(|name| root.join(name).is_file())
    .collect();
    if locks
        .iter()
        .any(|name| *name != expected && !(recipe.pm == "bun" && *name == "bun.lockb"))
    {
        report.add("lock.manager-conflict", "Package manager", Severity::Warning, "crabyard.recipe.toml", "Lockfiles from a different package manager are present.", "Keep the selected package manager and its lockfile consistent; remove obsolete lockfiles after verifying migration.");
    }
    if let Some(text) = report.read(root, expected) {
        let result = match recipe.pm.as_str() {
            "npm" => serde_json::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.pointer("/packages/").cloned())
                .map(|locked| {
                    dependency_groups(package).all(|(group, actual)| {
                        let expected: BTreeMap<String, String> = locked
                            .get(group)
                            .and_then(Value::as_object)
                            .into_iter()
                            .flatten()
                            .filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.into()))
                            })
                            .collect();
                        actual == expected
                    })
                }),
            "bun" => json5::from_str::<Value>(&text)
                .ok()
                .and_then(|v| v.pointer("/workspaces/").cloned())
                .map(|locked| {
                    dependency_groups(package).all(|(group, actual)| {
                        let expected: BTreeMap<String, String> = locked
                            .get(group)
                            .and_then(Value::as_object)
                            .into_iter()
                            .flatten()
                            .filter_map(|(key, value)| {
                                value.as_str().map(|value| (key.clone(), value.into()))
                            })
                            .collect();
                        actual == expected
                    })
                }),
            "pnpm" => project::yaml_documents(&text)
                .ok()
                .and_then(|documents| documents.into_iter().last())
                .and_then(|v| v.pointer("/importers/.").cloned())
                .map(|locked| {
                    dependency_groups(package).all(|(group, actual)| {
                        let expected: BTreeMap<String, String> = locked
                            .get(group)
                            .and_then(Value::as_object)
                            .into_iter()
                            .flatten()
                            .filter_map(|(key, value)| {
                                value
                                    .get("specifier")
                                    .and_then(Value::as_str)
                                    .map(|value| (key.clone(), value.into()))
                            })
                            .collect();
                        actual == expected
                    })
                }),
            "yarn" => {
                let selectors: Vec<_> = text
                    .lines()
                    .filter(|line| {
                        !line.starts_with(char::is_whitespace)
                            && !line.starts_with('#')
                            && line.ends_with(':')
                    })
                    .collect();
                Some(dependency_groups(package).flat_map(|(_, deps)| deps).all(
                    |(name, version)| {
                        selectors.iter().any(|line| {
                            line.trim_end_matches(':').split(", ").any(|selector| {
                                let selector = selector.trim_matches(['\'', '"']);
                                selector == format!("{name}@{version}")
                                    || selector == format!("{name}@npm:{version}")
                            })
                        })
                    },
                ))
            }
            _ => None,
        };
        match result {
            Some(true) => report.add("lock.frontend", "Frontend lockfile", Severity::Pass, expected, "Direct dependency declarations match the selected lockfile.", ""),
            Some(false) => report.add("lock.stale", "Frontend lockfile", Severity::Error, expected, "Direct dependencies changed without updating the lockfile.", "Run the selected package manager's install command and commit its updated lockfile."),
            None => report.add("lock.unverified", "Frontend lockfile", Severity::Warning, expected, "The lockfile format could not be verified.", "Regenerate a current text lockfile with the selected package manager; validate it with a frozen install in CI."),
        }
    } else {
        report.add(
            "lock.missing",
            "Frontend lockfile",
            Severity::Warning,
            expected,
            "A current text lockfile has not been generated.",
            "Install dependencies with the selected package manager and commit its lockfile.",
        );
    }
    if let Some(lock) = report.read(root, "src-tauri/Cargo.lock") {
        let parsed = toml::from_str::<toml::Value>(&lock).ok();
        let package_name = cargo
            .and_then(|v| v.get("package"))
            .and_then(|v| v.get("name"))
            .and_then(toml::Value::as_str);
        let locked_package = parsed
            .as_ref()
            .and_then(|v| v.get("package"))
            .and_then(toml::Value::as_array)
            .and_then(|packages| {
                packages.iter().find(|package| {
                    package.get("name").and_then(toml::Value::as_str) == package_name
                        && package.get("version").and_then(toml::Value::as_str)
                            == Some(recipe.app_version.as_str())
                })
            });
        let mut declared = BTreeSet::new();
        if let Some(cargo) = cargo {
            collect_cargo_dependencies(cargo, &mut declared);
            if let Some(targets) = cargo.get("target").and_then(toml::Value::as_table) {
                for target in targets.values() {
                    collect_cargo_dependencies(target, &mut declared);
                }
            }
        }
        let locked: BTreeSet<_> = locked_package
            .and_then(|package| package.get("dependencies"))
            .and_then(toml::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(toml::Value::as_str)
            .filter_map(|value| value.split_whitespace().next())
            .map(str::to_string)
            .collect();
        let consistent = locked_package.is_some() && declared.is_subset(&locked);
        if consistent {
            report.add(
                "lock.rust",
                "Rust lockfile",
                Severity::Pass,
                "src-tauri/Cargo.lock",
                "The Rust lockfile includes the app identity and declared direct dependencies.",
                "",
            );
        } else {
            report.add("lock.rust-stale", "Rust lockfile", Severity::Error, "src-tauri/Cargo.lock", "The Rust lockfile is invalid or does not match the app identity or dependencies.", "Run cargo generate-lockfile --manifest-path src-tauri/Cargo.toml and commit the result.");
        }
    } else {
        report.add("lock.rust-missing", "Rust lockfile", Severity::Warning, "src-tauri/Cargo.lock", "Rust dependency resolution is not checked in.", "Run cargo generate-lockfile --manifest-path src-tauri/Cargo.toml and commit Cargo.lock.");
    }
}

fn collect_cargo_dependencies(value: &toml::Value, output: &mut BTreeSet<String>) {
    for group in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if let Some(dependencies) = value.get(group).and_then(toml::Value::as_table) {
            for (name, dependency) in dependencies {
                output.insert(
                    dependency
                        .get("package")
                        .and_then(toml::Value::as_str)
                        .unwrap_or(name)
                        .to_string(),
                );
            }
        }
    }
}

fn check_instructions(report: &mut Report, root: &Path) {
    for path in [
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".github/copilot-instructions.md",
        "docs/PROJECT.md",
        "docs/tasks/FEATURE.md",
        "docs/HANDOFF.md",
    ] {
        if report
            .read(root, path)
            .is_some_and(|text| !text.trim().is_empty())
        {
            report.add(
                "instructions.present",
                "AI project guidance",
                Severity::Pass,
                path,
                &format!("{path} is available to coding assistants."),
                "",
            );
        } else {
            report.add("instructions.missing", "AI project guidance", if path == "AGENTS.md" { Severity::Error } else { Severity::Warning }, path, "A coding-assistant instruction or product-brief file is missing.", "Restore the generated guidance and tailor it to your product's acceptance criteria.");
        }
    }
    let cursor = root.join(".cursor/rules");
    let present = fs::read_dir(cursor)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path().extension().is_some_and(|ext| ext == "mdc"));
    if !present {
        report.add(
            "instructions.cursor",
            "Cursor guidance",
            Severity::Warning,
            ".cursor/rules",
            "No scoped Cursor instruction file was found.",
            "Restore the generated .mdc project rules.",
        );
    }
}

pub fn environment_values(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("export ").unwrap_or(line.trim());
            if line.starts_with('#') {
                return None;
            }
            let (key, value) = line.split_once('=')?;
            let value = value.split_once(" #").map_or(value, |(value, _)| value);
            Some((
                key.trim().into(),
                value.trim().trim_matches(['\'', '"']).into(),
            ))
        })
        .collect()
}

fn configured(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.to_lowercase().starts_with("your_")
        && !value.contains("{{")
        && !value.contains("${")
}

fn check_environment(
    report: &mut Report,
    root: &Path,
    recipe: Option<&Recipe>,
) -> BTreeMap<String, String> {
    let mut effective = BTreeMap::new();
    for path in [
        ".env",
        ".env.local",
        ".env.production",
        ".env.production.local",
    ] {
        if let Some(text) = report.read(root, path) {
            let values = environment_values(&text);
            if values.iter().any(|(name, value)| {
                name.starts_with("VITE_")
                    && [
                        "SECRET",
                        "PASSWORD",
                        "PRIVATE_KEY",
                        "ACCESS_TOKEN",
                        "REFRESH_TOKEN",
                    ]
                    .iter()
                    .any(|word| name.contains(word))
                    && configured(value)
            }) {
                report.add("env.exposed-secret", "Browser environment", Severity::Error, path, "A browser-exposed environment variable appears to contain a secret.", "Remove secrets from VITE_ variables; store credentials in native secure storage or server-side environment variables.");
            }
            effective.extend(values);
        }
    }
    let key = match recipe.map(|recipe| recipe.auth.as_str()) {
        Some("github") => "GITHUB_CLIENT_ID",
        Some("google") => "GOOGLE_CLIENT_ID",
        _ => return effective,
    };
    if effective.get(key).is_some_and(|value| {
        configured(value)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    }) {
        report.add(
            "oauth.configured",
            "OAuth public configuration",
            Severity::Pass,
            ".env",
            "The selected OAuth provider has a public client ID configured locally.",
            "",
        );
    } else {
        report.add(
            "oauth.missing",
            "OAuth public configuration",
            Severity::Warning,
            ".env",
            "The selected authentication provider still needs its public client ID.",
            &format!(
                "Set {key} in your local environment. Desktop apps must not embed a client secret."
            ),
        );
    }
    effective
}

fn check_capabilities(report: &mut Report, root: &Path, cargo: Option<&toml::Value>) {
    let entries = fs::read_dir(root.join("src-tauri/capabilities")).ok();
    let mut paths: Vec<_> = entries
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
        .map(|entry| entry.path())
        .collect();
    paths.sort();
    if paths.is_empty() {
        report.add(
            "capabilities.missing",
            "Tauri capabilities",
            Severity::Error,
            "src-tauri/capabilities",
            "No desktop capability definition was found.",
            "Restore a capability file scoped to the intended app windows and plugin permissions.",
        );
    }
    for path in paths {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let Some(capability) = report.json(root, &relative) else {
            continue;
        };
        if !["windows", "webviews"].iter().any(|field| {
            capability
                .get(field)
                .and_then(Value::as_array)
                .is_some_and(|values| !values.is_empty())
        }) {
            report.add(
                "capabilities.windows",
                "Tauri capability scope",
                Severity::Error,
                &relative,
                "The capability is not assigned to any app window or webview.",
                "Scope permissions to the intended window or webview labels.",
            );
        }
        let Some(permissions) = capability.get("permissions").and_then(Value::as_array) else {
            report.add(
                "capabilities.invalid",
                "Tauri capabilities",
                Severity::Error,
                &relative,
                "A capability file has no permissions array.",
                "Declare explicit plugin permissions in a permissions array.",
            );
            continue;
        };
        for permission in permissions {
            let name = permission
                .as_str()
                .or_else(|| permission.get("identifier").and_then(Value::as_str))
                .unwrap_or("");
            if name.is_empty() || name.contains('*') {
                report.add(
                    "capabilities.wildcard",
                    "Tauri capabilities",
                    Severity::Error,
                    &relative,
                    "A permission identifier is empty or uses a wildcard.",
                    "Use explicit Tauri v2 permission identifiers and narrow allow scopes.",
                );
            }
            if let Some((namespace, _)) = name.split_once(':') {
                if namespace != "core"
                    && cargo
                        .and_then(|value| value.get("dependencies"))
                        .and_then(|deps| deps.get(format!("tauri-plugin-{namespace}")))
                        .is_none()
                {
                    report.add("capabilities.plugin-missing", "Tauri capabilities", Severity::Error, &relative, "A capability references a plugin absent from Rust dependencies.", "Add the corresponding plugin or remove its obsolete capability permissions.");
                }
            }
        }
        if capability
            .pointer("/remote/urls")
            .and_then(Value::as_array)
            .is_some_and(|urls| !urls.is_empty())
        {
            report.add("capabilities.remote", "Remote IPC access", Severity::Warning, &relative, "Remote URLs can access native capabilities.", "Review every remote origin and grant only the minimum native permissions it needs.");
        }
    }
}

fn check_delivery(
    report: &mut Report,
    root: &Path,
    recipe: Option<&Recipe>,
    package: Option<&Value>,
    tauri: Option<&Value>,
    environment: &BTreeMap<String, String>,
) {
    if let Some(package) = package {
        let scripts = package.get("scripts").and_then(Value::as_object);
        if ["check", "test", "build"]
            .iter()
            .all(|name| scripts.is_some_and(|scripts| scripts.contains_key(*name)))
        {
            report.add(
                "quality.commands",
                "Verification commands",
                Severity::Pass,
                "package.json",
                "Combined checks, tests, and build commands are available.",
                "",
            );
        } else {
            report.add(
                "quality.commands-missing",
                "Verification commands",
                Severity::Warning,
                "package.json",
                "The project is missing a combined check, test, or build script.",
                "Define check, test, and build scripts and run them in CI.",
            );
        }
    }
    if let Some(tauri) = tauri {
        if !tauri.pointer("/app/security/csp").is_some_and(|csp| {
            csp.as_str().is_some_and(|text| !text.trim().is_empty())
                || csp
                    .as_object()
                    .is_some_and(|directives| !directives.is_empty())
        }) {
            report.add(
                "security.csp",
                "Webview policy",
                Severity::Error,
                "src-tauri/tauri.conf.json",
                "The production webview has no content security policy.",
                "Declare a restrictive production CSP; keep development origins in devCsp.",
            );
        }
        let updater = tauri.pointer("/plugins/updater");
        let endpoints = updater
            .and_then(|v| v.get("endpoints"))
            .and_then(Value::as_array);
        let enabled = environment
            .get("VITE_UPDATER_ENABLED")
            .is_some_and(|value| value == "true")
            || tauri
                .pointer("/bundle/createUpdaterArtifacts")
                .is_some_and(|value| value == true || value == "v1Compatible")
            || endpoints.is_some_and(|endpoints| !endpoints.is_empty())
            || recipe.is_some_and(|recipe| recipe.extras.iter().any(|extra| extra == "updater"));
        if enabled {
            let valid_endpoints = endpoints.is_some_and(|urls| {
                !urls.is_empty()
                    && urls.iter().all(|value| {
                        value
                            .as_str()
                            .and_then(|url| reqwest::Url::parse(url).ok())
                            .is_some_and(|url| {
                                url.scheme() == "https"
                                    && url.username().is_empty()
                                    && url.password().is_none()
                            })
                    })
            });
            let pubkey = updater
                .and_then(|value| value.get("pubkey"))
                .and_then(Value::as_str)
                .is_some_and(configured);
            let artifacts = tauri
                .pointer("/bundle/createUpdaterArtifacts")
                .is_some_and(|value| value == true || value == "v1Compatible");
            if !valid_endpoints || !pubkey || !artifacts {
                report.add("updater.incomplete", "Signed updates", Severity::Warning, "src-tauri/tauri.conf.json", "Updater delivery is incomplete.", "Set HTTPS update endpoints and the public signing key, then enable createUpdaterArtifacts. Keep the private signing key in CI secrets.");
            } else {
                report.add(
                    "updater.configured",
                    "Signed updates",
                    Severity::Pass,
                    "src-tauri/tauri.conf.json",
                    "HTTPS endpoints, public key, and updater artifacts are configured.",
                    "",
                );
            }
        } else {
            report.add(
                "updater.disabled",
                "Signed updates",
                Severity::Pass,
                "src-tauri/tauri.conf.json",
                "Automatic updates are explicitly disabled until configured.",
                "",
            );
        }
    }
    for (path, id, title) in [
        (
            ".github/workflows/ci.yml",
            "ci.missing",
            "Continuous integration",
        ),
        (
            ".github/workflows/release.yml",
            "release.missing",
            "Release automation",
        ),
    ] {
        match report.read(root, path) {
            Some(text) => match project::yaml_documents(&text) {
                Ok(documents) if documents.len() == 1 && documents[0].get("jobs").and_then(Value::as_object).is_some_and(|jobs| !jobs.is_empty()) => {
                    report.add("workflow.valid", title, Severity::Pass, path, "A valid workflow with jobs is present.", "");
                    if id == "release.missing" && !text.contains("TAURI_SIGNING_PRIVATE_KEY") {
                        report.add("release.signing", "Release signing", Severity::Warning, path, "The release workflow does not reference the updater signing secret.", "Reference secrets.TAURI_SIGNING_PRIVATE_KEY when building signed update artifacts.");
                    }
                }
                _ => report.add("workflow.invalid", title, Severity::Error, path, "The workflow is malformed or has no jobs.", "Correct the YAML and define verification or release jobs."),
            },
            None => report.add(id, title, Severity::Warning, path, "Delivery automation has not been configured.", "Restore the generated workflow and configure its required repository secrets and release settings."),
        }
    }
}

fn check_gitignore(report: &mut Report, root: &Path) {
    let Some(text) = report.read(root, ".gitignore") else {
        report.add(
            "gitignore.missing",
            "Secret file protection",
            Severity::Error,
            ".gitignore",
            "The project has no .gitignore.",
            "Ignore local environment files and private signing keys while retaining .env.example.",
        );
        return;
    };
    let mut builder = ignore::gitignore::GitignoreBuilder::new(root);
    for line in text.lines() {
        if builder.add_line(None, line).is_err() {
            report.add(
                "gitignore.invalid",
                "Secret file protection",
                Severity::Error,
                ".gitignore",
                "A gitignore pattern cannot be parsed.",
                "Correct the gitignore glob syntax.",
            );
            return;
        }
    }
    let Ok(matcher) = builder.build() else {
        return;
    };
    let ignored = |path: &str| {
        matcher
            .matched_path_or_any_parents(root.join(path), false)
            .is_ignore()
    };
    if [
        ".env",
        ".env.local",
        ".env.production",
        ".env.staging",
        ".env.production.local",
    ]
    .iter()
    .all(|path| ignored(path))
        && !ignored(".env.example")
    {
        report.add(
            "gitignore.secrets",
            "Secret file protection",
            Severity::Pass,
            ".gitignore",
            "Local environment files are ignored and the public example remains trackable.",
            "",
        );
    } else {
        report.add(
            "gitignore.env",
            "Secret file protection",
            Severity::Error,
            ".gitignore",
            "Environment files could be committed, or the public example is hidden.",
            "Ignore .env and .env.*; add !.env.example after those patterns.",
        );
    }
    if !["signing.key", "signing.p12"]
        .iter()
        .all(|path| ignored(path))
    {
        report.add(
            "gitignore.signing",
            "Private signing keys",
            Severity::Warning,
            ".gitignore",
            "Private signing key files are not excluded by default.",
            "Ignore *.key and *.p12, and store production signing material in CI secrets.",
        );
    }
}
