use colored::Colorize;

struct PluginEntry {
    name: &'static str,
    crate_name: &'static str,
    npm_name: &'static str,
    description: &'static str,
    add_feature: Option<&'static str>,
}

const PLUGINS: &[PluginEntry] = &[
    PluginEntry {
        name: "Autostart",
        crate_name: "tauri-plugin-autostart",
        npm_name: "@tauri-apps/plugin-autostart",
        description: "Launch app at system startup",
        add_feature: Some("autostart"),
    },
    PluginEntry {
        name: "Clipboard Manager",
        crate_name: "tauri-plugin-clipboard-manager",
        npm_name: "@tauri-apps/plugin-clipboard-manager",
        description: "Read and write to the system clipboard",
        add_feature: Some("clipboard"),
    },
    PluginEntry {
        name: "Deep Link",
        crate_name: "tauri-plugin-deep-link",
        npm_name: "@tauri-apps/plugin-deep-link",
        description: "Custom URL protocol handler (myapp://)",
        add_feature: Some("deep-link"),
    },
    PluginEntry {
        name: "Dialog",
        crate_name: "tauri-plugin-dialog",
        npm_name: "@tauri-apps/plugin-dialog",
        description: "Native file/message dialogs (included by default)",
        add_feature: None,
    },
    PluginEntry {
        name: "Filesystem",
        crate_name: "tauri-plugin-fs",
        npm_name: "@tauri-apps/plugin-fs",
        description: "Read/write files outside the webview sandbox",
        add_feature: Some("fs"),
    },
    PluginEntry {
        name: "Global Shortcut",
        crate_name: "tauri-plugin-global-shortcut",
        npm_name: "@tauri-apps/plugin-global-shortcut",
        description: "Register global keyboard shortcuts",
        add_feature: Some("global-shortcut"),
    },
    PluginEntry {
        name: "HTTP",
        crate_name: "tauri-plugin-http",
        npm_name: "@tauri-apps/plugin-http",
        description: "Make HTTP requests from the frontend",
        add_feature: Some("http"),
    },
    PluginEntry {
        name: "Log",
        crate_name: "tauri-plugin-log",
        npm_name: "@tauri-apps/plugin-log",
        description: "Structured logging with file rotation",
        add_feature: Some("log"),
    },
    PluginEntry {
        name: "Notification",
        crate_name: "tauri-plugin-notification",
        npm_name: "@tauri-apps/plugin-notification",
        description: "System notifications",
        add_feature: Some("notifications"),
    },
    PluginEntry {
        name: "Opener",
        crate_name: "tauri-plugin-opener",
        npm_name: "@tauri-apps/plugin-opener",
        description: "Open URLs and files with the default app (included by default)",
        add_feature: None,
    },
    PluginEntry {
        name: "OS",
        crate_name: "tauri-plugin-os",
        npm_name: "@tauri-apps/plugin-os",
        description: "Read OS information (included by default)",
        add_feature: None,
    },
    PluginEntry {
        name: "Process",
        crate_name: "tauri-plugin-process",
        npm_name: "@tauri-apps/plugin-process",
        description: "Read/control the current process (included by default)",
        add_feature: None,
    },
    PluginEntry {
        name: "Shell",
        crate_name: "tauri-plugin-shell",
        npm_name: "@tauri-apps/plugin-shell",
        description: "Execute shell commands and manage child processes",
        add_feature: Some("shell"),
    },
    PluginEntry {
        name: "SQL",
        crate_name: "tauri-plugin-sql",
        npm_name: "@tauri-apps/plugin-sql",
        description: "SQLite/MySQL/Postgres database access",
        add_feature: Some("sql"),
    },
    PluginEntry {
        name: "Store",
        crate_name: "tauri-plugin-store",
        npm_name: "@tauri-apps/plugin-store",
        description: "Persistent key-value store (included by default)",
        add_feature: Some("store"),
    },
    PluginEntry {
        name: "Updater",
        crate_name: "tauri-plugin-updater",
        npm_name: "@tauri-apps/plugin-updater",
        description: "Auto-update your app (included by default)",
        add_feature: Some("updater"),
    },
    PluginEntry {
        name: "WebSocket",
        crate_name: "tauri-plugin-websocket",
        npm_name: "@tauri-apps/plugin-websocket",
        description: "WebSocket client for real-time communication",
        add_feature: None,
    },
    PluginEntry {
        name: "Window State",
        crate_name: "tauri-plugin-window-state",
        npm_name: "@tauri-apps/plugin-window-state",
        description: "Persist and restore window size/position",
        add_feature: None,
    },
    PluginEntry {
        name: "Barcode Scanner",
        crate_name: "tauri-plugin-barcode-scanner",
        npm_name: "@tauri-apps/plugin-barcode-scanner",
        description: "Scan barcodes and QR codes (mobile)",
        add_feature: None,
    },
    PluginEntry {
        name: "Biometric",
        crate_name: "tauri-plugin-biometric",
        npm_name: "@tauri-apps/plugin-biometric",
        description: "Biometric authentication (fingerprint, face)",
        add_feature: None,
    },
    PluginEntry {
        name: "NFC",
        crate_name: "tauri-plugin-nfc",
        npm_name: "@tauri-apps/plugin-nfc",
        description: "Near-field communication (mobile)",
        add_feature: None,
    },
];

pub fn run(filter: Option<&str>) {
    println!();
    println!(
        "  {} {}",
        "Tauri v2 Plugins".truecolor(161, 161, 170).bold(),
        format!("({} plugins)", PLUGINS.len()).truecolor(113, 113, 122)
    );
    println!();

    for p in PLUGINS {
        if let Some(q) = filter {
            let q = q.to_lowercase();
            if !p.name.to_lowercase().contains(&q)
                && !p.crate_name.to_lowercase().contains(&q)
                && !p.description.to_lowercase().contains(&q)
            {
                continue;
            }
        }

        print!(
            "  {} {}",
            p.name.truecolor(6, 182, 212).bold(),
            format!("— {}", p.description).truecolor(113, 113, 122),
        );

        if let Some(feat) = p.add_feature {
            print!(
                "  {}",
                format!("[crabyard add {}]", feat).truecolor(34, 197, 94),
            );
        }
        println!();

        println!(
            "    {} {} | {}",
            "crates:".truecolor(113, 113, 122),
            p.crate_name.truecolor(228, 228, 231),
            p.npm_name.truecolor(228, 228, 231),
        );
    }

    println!();
    println!(
        "  {} {}",
        "Tip:".truecolor(161, 161, 170),
        "Use `crabyard add <feature>` for one-command installation of supported plugins."
            .truecolor(113, 113, 122)
    );
    println!();
}
