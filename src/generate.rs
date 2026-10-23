use std::collections::HashMap;
use std::fs;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use colored::Colorize;
use dialoguer::Input;
use walkdir::WalkDir;

struct CleanupGuard {
    path: PathBuf,
    disarmed: bool,
}

impl CleanupGuard {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if !self.disarmed && self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Reopen stdin from the terminal when piped (e.g. `curl | sh`).
/// On macOS, kqueue only delivers events on the inherited pty slave fd, not on a freshly
/// opened /dev/tty fd — so we prefer duping stdout/stderr before falling back to /dev/tty.
#[cfg(unix)]
fn ensure_stdin_tty() {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        return;
    }
    // Prefer the inherited pty slave fd so that macOS kqueue works.
    if std::io::stdout().is_terminal() {
        unsafe {
            libc::dup2(libc::STDOUT_FILENO, libc::STDIN_FILENO);
        }
        return;
    }
    if std::io::stderr().is_terminal() {
        unsafe {
            libc::dup2(libc::STDERR_FILENO, libc::STDIN_FILENO);
        }
        return;
    }
    // Last resort: open /dev/tty directly (works on Linux, unreliable on macOS).
    if let Ok(tty) = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    {
        use std::os::unix::io::AsRawFd;
        unsafe {
            libc::dup2(tty.as_raw_fd(), libc::STDIN_FILENO);
        }
    }
}

#[cfg(not(unix))]
fn ensure_stdin_tty() {}

use crate::config;
use crate::license;
use crate::overlay;
use crate::recipe::{Preset, Recipe};
use crate::tokens::{self, TokenMap};
use crate::tui::banner;
use crate::tui::generation::{Step, StepStatus, WorkerMsg};
use crate::tui::wizard;

pub struct Config {
    pub app_name: Option<String>,
    pub slug: Option<String>,
    pub bundle_id: Option<String>,
    pub app_version: Option<String>,
    pub author: Option<String>,
    pub description: Option<String>,
    pub auth: Option<String>,
    pub ui: Option<String>,
    pub template: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub yes: bool,
    pub no_git: bool,
    pub no_install: bool,
    pub pm: Option<String>,
    pub license_key: Option<String>,
    pub extras: Vec<String>,
    pub recipe: Option<PathBuf>,
    pub preset: Option<Preset>,
    pub dry_run: bool,
}

pub fn run(mut config: Config) -> Result<()> {
    let generated_at = unix_timestamp()?;
    if let Some(path) = &config.recipe {
        Recipe::read(path)?.apply(&mut config);
    }
    if let Some(preset) = config.preset {
        let (auth, ui, extras) = preset.defaults();
        config.auth = config.auth.or_else(|| Some(auth.into()));
        config.ui = config.ui.or_else(|| Some(ui.into()));
        if config.extras.is_empty() && config.recipe.is_none() {
            config.extras = extras.iter().map(|extra| (*extra).into()).collect();
        }
    }

    let rc = config::load();
    config.pm = config.pm.or(rc.defaults.pm);
    config.auth = config.auth.or(rc.defaults.auth);
    config.ui = config.ui.or(rc.defaults.ui);
    config.author = config.author.or(rc.defaults.author);
    config.license_key = config.license_key.or(rc.defaults.license_key);

    let non_interactive = config.yes
        || config.dry_run
        || config.recipe.is_some()
        || config.preset.is_some()
        || (config.app_name.is_some()
            && config.auth.is_some()
            && config.ui.is_some()
            && config.pm.is_some());
    Recipe::resolve(&config).validate()?;
    if !non_interactive {
        ensure_stdin_tty();
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            anyhow::bail!("Interactive generation needs a terminal. Use --yes, --recipe, or --preset for headless generation.");
        }
        let result = wizard::run_wizard(
            config.pm.as_deref(),
            config.auth.as_deref(),
            config.ui.as_deref(),
            &config.extras,
            config.app_name.as_deref(),
            config.slug.as_deref(),
            config.bundle_id.as_deref(),
            config.app_version.as_deref(),
            config.author.as_deref(),
            config.description.as_deref(),
        )?;

        let wiz = match result {
            Some(w) => w,
            None => {
                println!("  Cancelled.");
                return Ok(());
            }
        };

        config.pm = Some(wiz.pm);
        config.auth = Some(wiz.auth);
        config.ui = Some(wiz.ui);
        config.extras = wiz.extras;
        config.app_name = Some(wiz.app_name);
        config.slug = Some(wiz.slug).filter(|s| !s.is_empty());
        config.bundle_id = Some(wiz.bundle_id).filter(|s| !s.is_empty());
        config.app_version = Some(wiz.version);
        config.author = Some(wiz.author);
        config.description = Some(wiz.description);
    }

    let mut recipe = Recipe::resolve(&config);
    let mut seen = std::collections::HashSet::new();
    recipe.extras.retain(|extra| seen.insert(extra.clone()));
    recipe.validate()?;
    let output = config
        .output
        .clone()
        .unwrap_or_else(|| PathBuf::from(&recipe.slug));
    if output.symlink_metadata().is_ok() {
        anyhow::bail!(
            "Output directory '{}' already exists. Choose a different name or remove it first.",
            output.display()
        );
    }

    if config.dry_run {
        if let Some(template) = &config.template {
            validate_template(template, &recipe.auth, &recipe.ui)?;
            validate_output_location(template, &output)?;
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "recipe": recipe,
                "output": output,
                "template": config.template,
                "initialize_git": !config.no_git,
                "install_dependencies": !config.no_install,
                "host_setup": false,
                "generated_at": generated_at,
            }))?
        );
        return Ok(());
    }

    let mut template = resolve_template(config.template.clone(), config.license_key.as_deref())?;
    if (!template.join("auth").join(&recipe.auth).is_dir()
        || !template.join("ui").join(&recipe.ui).is_dir())
        && config.template.is_none()
    {
        if let Some(key) = config.license_key.as_deref() {
            template = license::refresh_cache(key)?;
        }
    }
    validate_template(&template, &recipe.auth, &recipe.ui)?;
    validate_output_location(&template, &output)?;
    if !non_interactive && !config.no_install {
        crate::doctor::ensure_rust_version()?;
        crate::doctor::ensure_linux_deps()?;
        crate::doctor::ensure_xcode_clt()?;
        crate::doctor::ensure_msvc()?;
        crate::doctor::ensure_webview2()?;
        crate::doctor::ensure_package_manager(Some(&recipe.pm))?;
    }

    let oauth_client_id = if non_interactive {
        None
    } else {
        collect_oauth_client_id(&config, &recipe.auth)?
    };
    let token_map = build_token_map(&recipe, generated_at);
    let slug = recipe.slug.clone();
    let auth_module = recipe.auth.clone();
    let ui_module = recipe.ui.clone();
    let pm_resolved = recipe.pm.clone();
    let extras = recipe.extras.clone();
    let base_dir = template.join("base");
    let auth_config =
        overlay::load_module_config(&template.join("auth").join(&auth_module).join("module.json"))?;
    let ui_config =
        overlay::load_module_config(&template.join("ui").join(&ui_module).join("module.json"))?;

    println!();
    banner::print_inline_banner();
    banner::print_inline_separator();

    println!(
        "\n  {} auth={}, ui={}, pm={}, extras={}",
        "Modules:".truecolor(161, 161, 170),
        auth_module.truecolor(6, 182, 212).bold(),
        ui_module.truecolor(6, 182, 212).bold(),
        pm_resolved.truecolor(6, 182, 212).bold(),
        extras.len().to_string().truecolor(6, 182, 212).bold()
    );
    println!();

    let no_git = config.no_git;
    let no_install = config.no_install;
    let has_extras = !extras.is_empty();

    let mut steps = vec![
        Step {
            label: "Copy base template".into(),
            status: StepStatus::Running,
        },
        Step {
            label: format!("Apply auth/{auth_module} overlay"),
            status: StepStatus::Pending,
        },
        Step {
            label: format!("Apply ui/{ui_module} overlay"),
            status: StepStatus::Pending,
        },
        Step {
            label: "Process tokens & markers".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Merge npm dependencies".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Apply extras & plugins".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Write env & manifest".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Initialize git repository".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Install frontend dependencies".into(),
            status: StepStatus::Pending,
        },
        Step {
            label: "Generate app icons".into(),
            status: StepStatus::Pending,
        },
    ];

    if !has_extras {
        steps[5].status = StepStatus::Skipped;
    }
    if no_git {
        steps[7].status = StepStatus::Skipped;
    }
    if no_install {
        steps[8].status = StepStatus::Skipped;
        steps[9].status = StepStatus::Skipped;
    }

    let install_ok = Arc::new(AtomicBool::new(!no_install));
    let install_ok_c = install_ok.clone();
    let hook_errors = Arc::new(Mutex::new(Vec::new()));
    let hook_errors_c = hook_errors.clone();

    let auth_module_c = auth_module.clone();
    let ui_module_c = ui_module.clone();
    let output_c = output.clone();
    let token_map_c = token_map.clone();
    let oauth_id_c = oauth_client_id.clone();
    let pm_c = pm_resolved.clone();
    let extras_c = extras.clone();

    if let Some(parent) = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&output)
        .with_context(|| format!("Failed to create output directory {}", output.display()))?;
    let mut cleanup_guard = CleanupGuard::new(output.clone());

    crate::tui::generation::run_generation(&slug, steps, non_interactive, move |tx| {
        let result = (|| -> anyhow::Result<()> {
            copy_overlay(&base_dir, &output_c)?;
            tx.send(WorkerMsg::StepDone(0)).ok();

            let auth_dir = template.join("auth").join(&auth_module_c);
            if !auth_dir.is_dir() {
                anyhow::bail!(
                    "Auth module '{}' not found in template at {}.\n\
                 Delete {} and re-run to refresh the cache.",
                    auth_module_c,
                    auth_dir.display(),
                    template.display()
                );
            }
            copy_overlay(&auth_dir, &output_c)?;
            tx.send(WorkerMsg::StepDone(1)).ok();

            let ui_dir = template.join("ui").join(&ui_module_c);
            if !ui_dir.is_dir() {
                anyhow::bail!(
                    "UI module '{}' not found in template at {}.\n\
                 Delete {} and re-run to refresh the cache.",
                    ui_module_c,
                    ui_dir.display(),
                    template.display()
                );
            }
            copy_overlay(&ui_dir, &output_c)?;
            tx.send(WorkerMsg::StepDone(2)).ok();

            let mut all_markers = HashMap::new();
            for (k, v) in &auth_config.markers {
                all_markers.insert(k.clone(), v.clone());
            }
            for (k, v) in &ui_config.markers {
                all_markers.insert(k.clone(), v.clone());
            }

            process_output(&output_c, &all_markers, &token_map_c)?;
            tx.send(WorkerMsg::StepDone(3)).ok();

            overlay::merge_package_deps(
                &output_c.join("package.json"),
                &[&auth_config, &ui_config],
            )?;
            tx.send(WorkerMsg::StepDone(4)).ok();

            if !extras_c.is_empty() {
                for extra in &extras_c {
                    let add_config = crate::add::Config {
                        feature: extra.clone(),
                        project: Some(output_c.clone()),
                        dry_run: false,
                    };
                    crate::add::run_silent(add_config)?;
                }
                tx.send(WorkerMsg::StepDone(5)).ok();
            }

            write_env_file(&output_c, &auth_module_c, oauth_id_c.as_deref())?;
            write_manifest(&output_c, &token_map_c, &auth_module_c, &ui_module_c)?;
            recipe.write(&output_c)?;
            tx.send(WorkerMsg::StepDone(6)).ok();

            if !no_git {
                match crate::hooks::git_init(&output_c) {
                    Ok(()) => tx.send(WorkerMsg::StepDone(7)).ok(),
                    Err(e) => {
                        hook_errors_c.lock().unwrap().push(e.to_string());
                        tx.send(WorkerMsg::StepFailed(7, e.to_string())).ok()
                    }
                };
            }

            if !no_install {
                match crate::hooks::install_deps(&output_c, &pm_c) {
                    Ok(()) => tx.send(WorkerMsg::StepDone(8)).ok(),
                    Err(e) => {
                        install_ok_c.store(false, Ordering::Relaxed);
                        hook_errors_c.lock().unwrap().push(e.to_string());
                        tx.send(WorkerMsg::StepFailed(8, e.to_string())).ok()
                    }
                };

                if install_ok_c.load(Ordering::Relaxed) {
                    match crate::hooks::generate_icons(&output_c) {
                        Ok(()) => tx.send(WorkerMsg::StepDone(9)).ok(),
                        Err(e) => {
                            hook_errors_c.lock().unwrap().push(e.to_string());
                            tx.send(WorkerMsg::StepFailed(9, e.to_string())).ok()
                        }
                    };
                }
            }

            tx.send(WorkerMsg::AllDone).ok();
            Ok(())
        })();

        result
    })?;

    cleanup_guard.disarm();
    if non_interactive && !hook_errors.lock().unwrap().is_empty() {
        anyhow::bail!("Project generated at {}, but setup failed: {}. The scaffold is preserved; install dependencies and retry the failed command.",
            output.display(), hook_errors.lock().unwrap().join("; "));
    }

    let needs_oauth =
        matches!(auth_module.as_str(), "github" | "google") && oauth_client_id.is_none();

    println!();
    banner::print_inline_separator();
    println!(
        " {} {}",
        "Project ready at".truecolor(34, 197, 94).bold(),
        output.display()
    );
    println!();
    println!(" {}", "Next steps:".truecolor(228, 228, 231).bold());
    println!(
        "   {}",
        format!("cd \"{}\"", output.display()).truecolor(161, 161, 170)
    );
    if needs_oauth {
        match auth_module.as_str() {
            "github" => {
                println!("   {}", "# Set up GitHub OAuth:".truecolor(161, 161, 170));
                println!(
                    "   {}",
                    "#   1. Go to https://github.com/settings/developers".truecolor(113, 113, 122)
                );
                println!(
                    "   {}",
                    "#   2. Create an OAuth App (callback URL can be blank)"
                        .truecolor(113, 113, 122)
                );
                println!(
                    "   {}",
                    "#   3. Copy Client ID into .env as GITHUB_CLIENT_ID".truecolor(113, 113, 122)
                );
            }
            "google" => {
                println!("   {}", "# Set up Google OAuth:".truecolor(161, 161, 170));
                println!(
                    "   {}",
                    "#   1. Go to https://console.cloud.google.com/apis/credentials"
                        .truecolor(113, 113, 122)
                );
                println!(
                    "   {}",
                    "#   2. Create a Desktop app credential".truecolor(113, 113, 122)
                );
                println!(
                    "   {}",
                    "#   3. Copy Client ID into .env as GOOGLE_CLIENT_ID".truecolor(113, 113, 122)
                );
            }
            _ => {}
        }
    }
    if !install_ok.load(Ordering::Relaxed) {
        println!(
            "   {}",
            format!("{pm_resolved} install")
                .truecolor(6, 182, 212)
                .bold()
        );
    }
    let dev_cmd = crate::doctor::pm_tauri_dev(&pm_resolved);
    println!("   {}", dev_cmd.truecolor(6, 182, 212).bold());
    banner::print_inline_separator();
    println!();

    Ok(())
}

fn collect_oauth_client_id(config: &Config, auth_module: &str) -> Result<Option<String>> {
    let (env_name, setup_url, help) = match auth_module {
        "github" => (
            "GITHUB_CLIENT_ID",
            "https://github.com/settings/developers",
            "Create a GitHub OAuth App → copy the Client ID",
        ),
        "google" => (
            "GOOGLE_CLIENT_ID",
            "https://console.cloud.google.com/apis/credentials",
            "Create a Desktop app credential → copy the Client ID",
        ),
        _ => return Ok(None),
    };

    if config.yes {
        return Ok(None);
    }

    println!();
    println!("  OAuth setup — {env_name}");
    println!("  {setup_url}");
    println!("  {help}");
    println!();

    let input: String = Input::new()
        .with_prompt(format!("{env_name} (press Enter to skip)"))
        .default(String::new())
        .show_default(false)
        .interact_text()
        .context("Prompt cancelled")?;

    let trimmed = input.trim().to_string();
    if trimmed.is_empty() || trimmed.starts_with("your_") {
        println!("  → Skipped. Set {env_name} in .env before running the app.");
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

fn build_token_map(recipe: &Recipe, generated_at: String) -> TokenMap {
    let mut map = HashMap::new();
    map.insert("APP_NAME".into(), recipe.app_name.clone());
    map.insert("APP_SLUG".into(), recipe.slug.clone());
    map.insert("APP_SLUG_SNAKE".into(), tokens::to_snake(&recipe.slug));
    map.insert("APP_BUNDLE_ID".into(), recipe.bundle_id.clone());
    map.insert("APP_VERSION".into(), recipe.app_version.clone());
    map.insert("APP_DESCRIPTION".into(), recipe.description.clone());
    map.insert("APP_AUTHOR".into(), recipe.author.clone());
    map.insert("AUTH_MODULE".into(), recipe.auth.clone());
    map.insert("UI_MODULE".into(), recipe.ui.clone());
    map.insert("PACKAGE_MANAGER".into(), recipe.pm.clone());
    map.insert(
        "PM_RUN".into(),
        crate::doctor::pm_run_prefix(&recipe.pm).into(),
    );
    map.insert(
        "PM_TAURI_DEV".into(),
        crate::doctor::pm_tauri_dev(&recipe.pm).into(),
    );
    map.insert("CRABYARD_VERSION".into(), env!("GIT_VERSION").into());
    map.insert("GENERATED_AT".into(), generated_at);

    map
}

fn validate_template(template: &Path, auth: &str, ui: &str) -> Result<()> {
    for module in [
        PathBuf::from("base"),
        PathBuf::from("auth").join(auth),
        PathBuf::from("ui").join(ui),
    ] {
        let path = template.join(module);
        if !path.is_dir() {
            anyhow::bail!("Template module not found: {}", path.display());
        }
        overlay::load_module_config(&path.join("module.json"))?;
        for entry in WalkDir::new(&path).into_iter().filter_entry(|entry| {
            entry
                .path()
                .strip_prefix(&path)
                .map_or(true, |rel| !tokens::should_skip_path(rel))
        }) {
            let entry =
                entry.with_context(|| format!("Cannot read template {}", path.display()))?;
            if entry.file_type().is_symlink() {
                anyhow::bail!(
                    "Template contains a symbolic link: {}",
                    entry.path().display()
                );
            }
        }
    }
    Ok(())
}

fn validate_output_location(template: &Path, output: &Path) -> Result<()> {
    let template = fs::canonicalize(template)?;
    let absolute = std::env::current_dir()?.join(output);
    for ancestor in absolute.ancestors() {
        if let Ok(mut resolved) = fs::canonicalize(ancestor) {
            for component in absolute.strip_prefix(ancestor)?.components() {
                match component {
                    std::path::Component::ParentDir => {
                        resolved.pop();
                    }
                    std::path::Component::CurDir => {}
                    _ => resolved.push(component.as_os_str()),
                }
            }
            if resolved.starts_with(&template) {
                anyhow::bail!("Output directory must be outside the template directory");
            }
            return Ok(());
        }
    }
    anyhow::bail!("Cannot resolve output directory {}", output.display())
}

/// Copy an overlay directory to output, overwriting existing files.
/// Skips module.json (consumed separately by the marker/deps system).
fn copy_overlay(source: &Path, output: &Path) -> Result<()> {
    for entry in WalkDir::new(source).into_iter().filter_entry(|entry| {
        entry
            .path()
            .strip_prefix(source)
            .map_or(true, |rel| !tokens::should_skip_path(rel))
    }) {
        let entry = entry.with_context(|| format!("Cannot read template {}", source.display()))?;
        let src = entry.path();
        let rel = src.strip_prefix(source)?;

        if tokens::should_skip_path(rel) {
            continue;
        }

        if rel
            .file_name()
            .map(|f| f == "module.json" || f == ".sync-state.json")
            .unwrap_or(false)
        {
            continue;
        }

        let dst = output.join(rel);

        if entry.file_type().is_dir() {
            fs::create_dir_all(&dst)
                .with_context(|| format!("Failed to create directory: {}", dst.display()))?;
            continue;
        }

        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::copy(src, &dst).with_context(|| format!("Failed to copy: {}", src.display()))?;
    }
    Ok(())
}

/// Walk all files in the output directory, applying markers and token replacement.
fn process_output(
    output: &Path,
    markers: &HashMap<String, String>,
    token_map: &TokenMap,
) -> Result<usize> {
    let mut count = 0usize;

    for entry in WalkDir::new(output) {
        let entry =
            entry.with_context(|| format!("Cannot read generated project {}", output.display()))?;
        let path = entry.path();
        if !entry.file_type().is_file() || tokens::is_binary_path(path) {
            continue;
        }

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let processed = overlay::apply_markers(&content, markers);
        let processed = tokens::replace_for_path(&processed, token_map, path);

        if processed != content {
            fs::write(path, &processed)
                .with_context(|| format!("Failed to write: {}", path.display()))?;
            count += 1;
        }
    }

    Ok(count)
}

fn write_env_file(output: &Path, auth_module: &str, client_id: Option<&str>) -> Result<()> {
    let example = output.join(".env.example");
    let env = output.join(".env");
    if example.exists() && !env.exists() {
        fs::copy(&example, &env).context("Failed to create .env from .env.example")?;

        if let Some(id) = client_id {
            let content = fs::read_to_string(&env)?;
            let content = match auth_module {
                "github" => content.replace(
                    "GITHUB_CLIENT_ID=your_github_client_id_here",
                    &format!("GITHUB_CLIENT_ID={id}"),
                ),
                "google" => content.replace(
                    "GOOGLE_CLIENT_ID=your_google_client_id_here",
                    &format!("GOOGLE_CLIENT_ID={id}"),
                ),
                _ => content,
            };
            fs::write(&env, content)?;
        }
    }
    Ok(())
}

fn write_manifest(output: &Path, token_map: &TokenMap, auth: &str, ui: &str) -> Result<()> {
    let manifest = output.join("manifest.toml");
    if manifest.exists() {
        let content = fs::read_to_string(&manifest)?;
        let content = content
            .replace("{{AUTH_MODULE}}", auth)
            .replace("{{UI_MODULE}}", ui);
        let content = tokens::replace(&content, token_map);
        fs::write(&manifest, content)?;
    }
    Ok(())
}

pub fn resolve_template_path(
    explicit: Option<PathBuf>,
    license_key: Option<&str>,
) -> Result<PathBuf> {
    resolve_template(explicit, license_key)
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

    anyhow::bail!(
        "No template directory found.\n\
         Options:\n  \
         1. Set CRABYARD_LICENSE_KEY env var (downloads from crabyard.dev)\n  \
         2. Set CRABYARD_TEMPLATE env var: export CRABYARD_TEMPLATE=/path/to/template\n  \
         3. Use --template flag: crabyard new --template /path/to/template"
    )
}

fn unix_timestamp() -> Result<String> {
    use std::time::{SystemTime, UNIX_EPOCH};
    match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => {
            return Ok(value
                .parse::<u64>()
                .context("SOURCE_DATE_EPOCH must be a non-negative Unix timestamp")?
                .to_string())
        }
        Err(std::env::VarError::NotPresent) => {}
        Err(error) => return Err(error).context("Invalid SOURCE_DATE_EPOCH"),
    }
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into()))
}
