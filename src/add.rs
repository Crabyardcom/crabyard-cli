use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;

pub struct Config {
    pub feature: String,
    pub project: Option<PathBuf>,
    pub dry_run: bool,
}

struct FeatureInfo {
    name: &'static str,
    description: &'static str,
    npm_deps: &'static [(&'static str, &'static str)],
    npm_dev_deps: &'static [(&'static str, &'static str)],
    cargo_deps: &'static [(&'static str, &'static str)],
    tauri_plugins: &'static [&'static str],
    capabilities: &'static [&'static str],
}

const FEATURES: &[FeatureInfo] = &[
    FeatureInfo {
        name: "notifications",
        description: "System notifications via tauri-plugin-notification",
        npm_deps: &[("@tauri-apps/plugin-notification", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-notification", "\"2\"")],
        tauri_plugins: &["tauri_plugin_notification"],
        capabilities: &["notification:default"],
    },
    FeatureInfo {
        name: "clipboard",
        description: "System clipboard read/write via tauri-plugin-clipboard-manager",
        npm_deps: &[("@tauri-apps/plugin-clipboard-manager", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-clipboard-manager", "\"2\"")],
        tauri_plugins: &["tauri_plugin_clipboard_manager"],
        capabilities: &["clipboard-manager:allow-read-text", "clipboard-manager:allow-write-text"],
    },
    FeatureInfo {
        name: "global-shortcut",
        description: "Global keyboard shortcuts via tauri-plugin-global-shortcut",
        npm_deps: &[("@tauri-apps/plugin-global-shortcut", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-global-shortcut", "\"2\"")],
        tauri_plugins: &["tauri_plugin_global_shortcut::Builder::new().build()"],
        capabilities: &["global-shortcut:default"],
    },
    FeatureInfo {
        name: "autostart",
        description: "Launch app at system startup via tauri-plugin-autostart",
        npm_deps: &[("@tauri-apps/plugin-autostart", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-autostart", "\"2\"")],
        tauri_plugins: &["tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None)"],
        capabilities: &["autostart:default"],
    },
    FeatureInfo {
        name: "log",
        description: "Structured logging via tauri-plugin-log",
        npm_deps: &[("@tauri-apps/plugin-log", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-log", "{ version = \"2\", features = [\"colored\"] }")],
        tauri_plugins: &["tauri_plugin_log::Builder::new().build()"],
        capabilities: &["log:default"],
    },
    FeatureInfo {
        name: "sql",
        description: "SQLite database via tauri-plugin-sql",
        npm_deps: &[("@tauri-apps/plugin-sql", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-sql", "{ version = \"2\", features = [\"sqlite\"] }")],
        tauri_plugins: &["tauri_plugin_sql::Builder::new().build()"],
        capabilities: &["sql:default"],
    },
    FeatureInfo {
        name: "fs",
        description: "Filesystem access via tauri-plugin-fs",
        npm_deps: &[("@tauri-apps/plugin-fs", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-fs", "\"2\"")],
        tauri_plugins: &["tauri_plugin_fs"],
        capabilities: &["fs:default"],
    },
    FeatureInfo {
        name: "shell",
        description: "Execute shell commands via tauri-plugin-shell",
        npm_deps: &[("@tauri-apps/plugin-shell", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-shell", "\"2\"")],
        tauri_plugins: &["tauri_plugin_shell"],
        capabilities: &["shell:default"],
    },
    FeatureInfo {
        name: "http",
        description: "HTTP client via tauri-plugin-http",
        npm_deps: &[("@tauri-apps/plugin-http", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-http", "\"2\"")],
        tauri_plugins: &["tauri_plugin_http"],
        capabilities: &["http:default"],
    },
    FeatureInfo {
        name: "deep-link",
        description: "Custom URL protocol handler via tauri-plugin-deep-link",
        npm_deps: &[("@tauri-apps/plugin-deep-link", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-deep-link", "\"2\"")],
        tauri_plugins: &["tauri_plugin_deep_link"],
        capabilities: &["deep-link:default"],
    },
    FeatureInfo {
        name: "tray",
        description: "System tray icon with context menu via tray-icon",
        npm_deps: &[],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &["tauri::tray::TrayIconBuilder::new().build(app)?;return Ok(())"],
        capabilities: &[],
    },
    FeatureInfo {
        name: "updater",
        description: "Auto-updater via tauri-plugin-updater",
        npm_deps: &[("@tauri-apps/plugin-updater", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-updater", "\"2\"")],
        tauri_plugins: &["tauri_plugin_updater"],
        capabilities: &["updater:default"],
    },
    FeatureInfo {
        name: "store",
        description: "Persistent key-value store via tauri-plugin-store",
        npm_deps: &[("@tauri-apps/plugin-store", "~2")],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-store", "\"2\"")],
        tauri_plugins: &["tauri_plugin_store"],
        capabilities: &["store:default"],
    },
    FeatureInfo {
        name: "cmdk",
        description: "Command palette component (Ctrl/Cmd+K)",
        npm_deps: &[("cmdk", "^1")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "i18n",
        description: "Internationalization with react-i18next",
        npm_deps: &[("react-i18next", "^15"), ("i18next", "^24")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "multi-window",
        description: "Multi-window support with WebviewWindow API",
        npm_deps: &[],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[
            "core:window:allow-create",
            "core:window:allow-close",
            "core:window:allow-set-title",
            "core:window:allow-set-size",
            "core:window:allow-center",
            "core:window:allow-show",
            "core:window:allow-hide",
            "core:webview:allow-create-webview-window",
        ],
    },
    FeatureInfo {
        name: "tanstack-query",
        description: "Data fetching and caching with TanStack Query",
        npm_deps: &[("@tanstack/react-query", "^5")],
        npm_dev_deps: &[("@tanstack/react-query-devtools", "^5")],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "framer-motion",
        description: "Animations and transitions with Motion",
        npm_deps: &[("motion", "^12")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "react-hook-form",
        description: "Performant form management with React Hook Form + Zod",
        npm_deps: &[("react-hook-form", "^7"), ("@hookform/resolvers", "^5"), ("zod", "^3")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "zod",
        description: "Runtime schema validation with Zod",
        npm_deps: &[("zod", "^3")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "tanstack-router",
        description: "Type-safe routing with TanStack Router",
        npm_deps: &[("@tanstack/react-router", "^1"), ("@tanstack/router-devtools", "^1")],
        npm_dev_deps: &[("@tanstack/router-plugin", "^1")],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "date-fns",
        description: "Modern date utility library",
        npm_deps: &[("date-fns", "^4")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "sentry",
        description: "Error tracking and crash reporting with Sentry",
        npm_deps: &[("@sentry/react", "^9")],
        npm_dev_deps: &[],
        cargo_deps: &[],
        tauri_plugins: &[],
        capabilities: &[],
    },
    FeatureInfo {
        name: "window-state",
        description: "Persist and restore window size/position",
        npm_deps: &[],
        npm_dev_deps: &[],
        cargo_deps: &[("tauri-plugin-window-state", "\"2\"")],
        tauri_plugins: &["tauri_plugin_window_state::Builder::default().build()"],
        capabilities: &["window-state:default"],
    },
];

pub fn run(config: Config) -> Result<()> {
    if config.feature == "list" {
        print_available_features();
        return Ok(());
    }

    let feature = FEATURES
        .iter()
        .find(|f| f.name == config.feature)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown feature '{}'. Run `crabyard add list` to see available features.",
                config.feature
            )
        })?;

    let project = config
        .project
        .unwrap_or_else(|| PathBuf::from("."));

    if !project.join("package.json").exists() || !project.join("src-tauri").exists() {
        anyhow::bail!(
            "Not a Crabyard project. Run this command from a Crabyard project directory."
        );
    }

    println!();
    println!(
        "  {} {}",
        "Adding feature:".truecolor(161, 161, 170),
        feature.name.truecolor(6, 182, 212).bold()
    );
    println!("  {}", feature.description.truecolor(113, 113, 122));
    println!();

    if config.dry_run {
        println!("  {} (dry run — no files modified)", "Preview:".truecolor(161, 161, 170));
        println!();
        print_plan(feature);
        return Ok(());
    }

    add_npm_deps(&project, feature)?;
    add_cargo_deps(&project, feature)?;
    add_tauri_plugin(&project, feature)?;
    add_capabilities(&project, feature)?;

    println!();
    println!(
        "  {} {} added",
        "✓".truecolor(34, 197, 94).bold(),
        feature.name.truecolor(6, 182, 212).bold()
    );
    println!();

    if let Some(first_dep) = feature.npm_deps.first() {
        println!(
            "  {} Run your package manager's install command, then use the plugin:",
            "Next:".truecolor(161, 161, 170)
        );
        println!(
            "  {}",
            format!("  import {{ ... }} from \"{}\"", first_dep.0)
                .truecolor(228, 228, 231)
        );
    } else {
        println!(
            "  {} Feature configured. Restart your dev server to apply changes.",
            "Next:".truecolor(161, 161, 170)
        );
    }
    println!();

    Ok(())
}

fn print_available_features() {
    println!();
    println!("  {} ", "Available features:".truecolor(161, 161, 170).bold());
    println!();
    for f in FEATURES {
        println!(
            "    {}  {}",
            f.name.truecolor(6, 182, 212).bold(),
            f.description.truecolor(113, 113, 122)
        );
    }
    println!();
    println!(
        "  Usage: {}",
        "crabyard add <feature>".truecolor(228, 228, 231)
    );
    println!();
}

fn print_plan(feature: &FeatureInfo) {
    if !feature.npm_deps.is_empty() || !feature.npm_dev_deps.is_empty() {
        println!("  npm dependencies:");
        for (name, ver) in feature.npm_deps {
            println!("    + {} {}", name, ver);
        }
        for (name, ver) in feature.npm_dev_deps {
            println!("    + {} {} (dev)", name, ver);
        }
    }
    if !feature.cargo_deps.is_empty() {
        println!("  Cargo dependencies:");
        for (name, ver) in feature.cargo_deps {
            println!("    + {} = {}", name, ver);
        }
    }
    if !feature.tauri_plugins.is_empty() {
        println!("  Tauri plugins (lib.rs):");
        for p in feature.tauri_plugins {
            println!("    + .plugin({}::init())", p);
        }
    }
    if !feature.capabilities.is_empty() {
        println!("  Capabilities:");
        for c in feature.capabilities {
            println!("    + {}", c);
        }
    }
    println!();
}

pub fn run_silent(config: Config) -> Result<()> {
    let feature = FEATURES
        .iter()
        .find(|f| f.name == config.feature)
        .ok_or_else(|| {
            anyhow::anyhow!("Unknown feature '{}'", config.feature)
        })?;

    let project = config.project.unwrap_or_else(|| PathBuf::from("."));

    add_npm_deps_silent(&project, feature)?;
    add_cargo_deps_silent(&project, feature)?;
    add_tauri_plugin_silent(&project, feature)?;
    add_capabilities_silent(&project, feature)?;

    Ok(())
}

fn add_npm_deps_silent(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.npm_deps.is_empty() && feature.npm_dev_deps.is_empty() {
        return Ok(());
    }
    let pkg_path = project.join("package.json");
    let content = fs::read_to_string(&pkg_path).context("Failed to read package.json")?;
    let mut pkg: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse package.json")?;
    let obj = pkg.as_object_mut().unwrap();
    if !feature.npm_deps.is_empty() {
        let deps = obj.entry("dependencies").or_insert_with(|| serde_json::json!({}));
        for (name, ver) in feature.npm_deps {
            deps[*name] = serde_json::Value::String(ver.to_string());
        }
    }
    if !feature.npm_dev_deps.is_empty() {
        let deps = obj.entry("devDependencies").or_insert_with(|| serde_json::json!({}));
        for (name, ver) in feature.npm_dev_deps {
            deps[*name] = serde_json::Value::String(ver.to_string());
        }
    }
    let output = serde_json::to_string_pretty(&pkg)?;
    fs::write(&pkg_path, output).context("Failed to write package.json")?;
    Ok(())
}

fn find_deps_insert_pos(content: &str) -> usize {
    if let Some(marker_pos) = content.find("# CRABYARD:CARGO_DEPS") {
        return marker_pos;
    }
    let search_from = if let Some(pos) = content.find("\n[dependencies]") {
        pos + "\n[dependencies]".len()
    } else if content.starts_with("[dependencies]") {
        "[dependencies]".len()
    } else {
        return content.len();
    };
    if let Some(next) = content[search_from..].find("\n[") {
        return search_from + next + 1;
    }
    content.len()
}

fn ensure_newline_at(content: &mut String, pos: usize) {
    if pos > 0 && content.as_bytes().get(pos - 1) != Some(&b'\n') {
        content.insert(pos, '\n');
    }
}

fn add_cargo_deps_silent(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.cargo_deps.is_empty() {
        return Ok(());
    }
    let cargo_path = project.join("src-tauri").join("Cargo.toml");
    let mut content = fs::read_to_string(&cargo_path).context("Failed to read Cargo.toml")?;
    for (name, ver) in feature.cargo_deps {
        if content.contains(&format!("{name} =")) || content.contains(&format!("{name}=")) {
            continue;
        }
        let dep_line = format!("{} = {}\n", name, ver);
        let mut insert_pos = find_deps_insert_pos(&content);
        ensure_newline_at(&mut content, insert_pos);
        insert_pos = find_deps_insert_pos(&content);
        content.insert_str(insert_pos, &dep_line);
    }
    fs::write(&cargo_path, content).context("Failed to write Cargo.toml")?;
    Ok(())
}

fn add_tauri_plugin_silent(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.tauri_plugins.is_empty() {
        return Ok(());
    }
    let lib_path = project.join("src-tauri").join("src").join("lib.rs");
    let mut content = fs::read_to_string(&lib_path).context("Failed to read lib.rs")?;
    for plugin in feature.tauri_plugins {
        let init_call = if plugin.contains("::init(") || plugin.contains("::new(") || plugin.contains("::Builder") {
            format!(".plugin({})", plugin)
        } else {
            format!(".plugin({}::init())", plugin)
        };
        if content.contains(plugin) {
            continue;
        }
        if let Some(pos) = content.find(".setup(") {
            content.insert_str(pos, &format!("{}\n        ", init_call));
        }
    }
    fs::write(&lib_path, content).context("Failed to write lib.rs")?;
    Ok(())
}

fn add_capabilities_silent(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.capabilities.is_empty() {
        return Ok(());
    }
    let cap_path = project.join("src-tauri").join("capabilities").join("default.json");
    let content = fs::read_to_string(&cap_path).context("Failed to read capabilities/default.json")?;
    let mut cap: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse capabilities/default.json")?;
    let permissions = cap
        .get_mut("permissions")
        .and_then(|p| p.as_array_mut())
        .context("Missing permissions array in default.json")?;
    for perm in feature.capabilities {
        let val = serde_json::Value::String(perm.to_string());
        if !permissions.contains(&val) {
            permissions.push(val);
        }
    }
    let output = serde_json::to_string_pretty(&cap)?;
    fs::write(&cap_path, output).context("Failed to write capabilities/default.json")?;
    Ok(())
}

fn add_npm_deps(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.npm_deps.is_empty() && feature.npm_dev_deps.is_empty() {
        return Ok(());
    }
    let pkg_path = project.join("package.json");
    let content = fs::read_to_string(&pkg_path).context("Failed to read package.json")?;
    let mut pkg: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse package.json")?;

    let obj = pkg.as_object_mut().unwrap();

    if !feature.npm_deps.is_empty() {
        let deps = obj
            .entry("dependencies")
            .or_insert_with(|| serde_json::json!({}));
        for (name, ver) in feature.npm_deps {
            deps[*name] = serde_json::Value::String(ver.to_string());
        }
    }

    if !feature.npm_dev_deps.is_empty() {
        let deps = obj
            .entry("devDependencies")
            .or_insert_with(|| serde_json::json!({}));
        for (name, ver) in feature.npm_dev_deps {
            deps[*name] = serde_json::Value::String(ver.to_string());
        }
    }

    let output = serde_json::to_string_pretty(&pkg)?;
    fs::write(&pkg_path, output).context("Failed to write package.json")?;
    println!("    {} package.json", "✓".truecolor(34, 197, 94));
    Ok(())
}

fn add_cargo_deps(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.cargo_deps.is_empty() {
        return Ok(());
    }

    let cargo_path = project.join("src-tauri").join("Cargo.toml");
    let mut content =
        fs::read_to_string(&cargo_path).context("Failed to read Cargo.toml")?;

    for (name, ver) in feature.cargo_deps {
        if content.contains(&format!("{name} =")) || content.contains(&format!("{name}=")) {
            println!(
                "    {} {} already in Cargo.toml",
                "–".truecolor(161, 161, 170),
                name
            );
            continue;
        }

        let dep_line = format!("{} = {}\n", name, ver);
        let mut insert_pos = find_deps_insert_pos(&content);
        ensure_newline_at(&mut content, insert_pos);
        insert_pos = find_deps_insert_pos(&content);
        content.insert_str(insert_pos, &dep_line);
    }

    fs::write(&cargo_path, content).context("Failed to write Cargo.toml")?;
    println!("    {} Cargo.toml", "✓".truecolor(34, 197, 94));
    Ok(())
}

fn add_tauri_plugin(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.tauri_plugins.is_empty() {
        return Ok(());
    }

    let lib_path = project.join("src-tauri").join("src").join("lib.rs");
    let mut content =
        fs::read_to_string(&lib_path).context("Failed to read lib.rs")?;

    for plugin in feature.tauri_plugins {
        let init_call = if plugin.contains("::init(") || plugin.contains("::new(") || plugin.contains("::Builder") {
            format!(".plugin({})", plugin)
        } else {
            format!(".plugin({}::init())", plugin)
        };

        if content.contains(plugin) {
            println!(
                "    {} {} already in lib.rs",
                "–".truecolor(161, 161, 170),
                plugin
            );
            continue;
        }

        // Insert plugin registration before .setup(
        if let Some(pos) = content.find(".setup(") {
            content.insert_str(pos, &format!("{}\n        ", init_call));
        }
    }

    fs::write(&lib_path, content).context("Failed to write lib.rs")?;
    println!("    {} lib.rs", "✓".truecolor(34, 197, 94));
    Ok(())
}

fn add_capabilities(project: &Path, feature: &FeatureInfo) -> Result<()> {
    if feature.capabilities.is_empty() {
        return Ok(());
    }

    let cap_path = project
        .join("src-tauri")
        .join("capabilities")
        .join("default.json");
    let content =
        fs::read_to_string(&cap_path).context("Failed to read capabilities/default.json")?;
    let mut cap: serde_json::Value =
        serde_json::from_str(&content).context("Failed to parse capabilities/default.json")?;

    let permissions = cap
        .get_mut("permissions")
        .and_then(|p| p.as_array_mut())
        .context("Missing permissions array in default.json")?;

    for perm in feature.capabilities {
        let val = serde_json::Value::String(perm.to_string());
        if !permissions.contains(&val) {
            permissions.push(val);
        }
    }

    let output = serde_json::to_string_pretty(&cap)?;
    fs::write(&cap_path, output).context("Failed to write capabilities/default.json")?;
    println!("    {} capabilities/default.json", "✓".truecolor(34, 197, 94));
    Ok(())
}
