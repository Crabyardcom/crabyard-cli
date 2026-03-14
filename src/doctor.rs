use std::fmt;
use std::process::{Command, Stdio};

use anyhow::Result;
use colored::Colorize;
use dialoguer::Confirm;

#[derive(Clone, Copy)]
enum Status {
    Ok,
    Warning,
    Missing,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::Ok => write!(f, "✓"),
            Status::Warning => write!(f, "!"),
            Status::Missing => write!(f, "✗"),
        }
    }
}

struct Check {
    name: &'static str,
    status: Status,
    detail: String,
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

pub(crate) fn get_version(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
}

pub(crate) fn parse_semver(s: &str) -> Option<(u32, u32, u32)> {
    let version_part = s.split_whitespace().find(|w| w.contains('.'))?;
    let mut parts = version_part.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts
        .next()
        .and_then(|p| p.split('-').next()?.parse().ok())
        .unwrap_or(0);
    Some((major, minor, patch))
}

fn cmd_exists(cmd: &str) -> bool {
    Command::new(cmd)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn print_action(msg: &str) {
    println!(
        "  {} {}",
        "→".truecolor(6, 182, 212).bold(),
        msg.truecolor(228, 228, 231),
    );
}

fn print_ok(msg: &str) {
    println!(
        "  {} {}",
        "✓".truecolor(34, 197, 94).bold(),
        msg.truecolor(34, 197, 94),
    );
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

fn has_rustup() -> bool {
    Command::new("rustup")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn install_rustup() -> Result<()> {
    print_action("Installing rustup…");

    #[cfg(unix)]
    {
        let ok = Command::new("sh")
            .args([
                "-c",
                "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            anyhow::bail!("Failed to install rustup. Install manually from https://rustup.rs");
        }
        let home = std::env::var("HOME").unwrap_or_default();
        let cargo_bin = format!("{home}/.cargo/bin");
        if let Ok(path) = std::env::var("PATH") {
            std::env::set_var("PATH", format!("{cargo_bin}:{path}"));
        }
    }

    #[cfg(windows)]
    {
        let ok = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Invoke-WebRequest -Uri https://win.rustup.rs/x86_64 -OutFile $env:TEMP\\rustup-init.exe; \
                 & $env:TEMP\\rustup-init.exe -y",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            anyhow::bail!("Failed to install rustup. Install manually from https://rustup.rs");
        }
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let cargo_bin = format!("{userprofile}\\.cargo\\bin");
        if let Ok(path) = std::env::var("PATH") {
            std::env::set_var("PATH", format!("{cargo_bin};{path}"));
        }
    }

    print_ok("rustup installed");
    println!();
    Ok(())
}

fn rustup_update() -> Result<()> {
    print_action("Updating Rust via rustup…");
    println!();

    let ok = Command::new("rustup")
        .arg("update")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    println!();

    if !ok {
        anyhow::bail!("rustup update failed. Update Rust manually and try again.");
    }

    let new_v = get_version("rustc", &["--version"])
        .ok_or_else(|| anyhow::anyhow!("rustc not found after update — check your PATH"))?;

    if parse_semver(&new_v).map_or(true, |(maj, min, _)| !(maj >= 1 && min >= 88)) {
        anyhow::bail!(
            "Rust {new_v} is still below 1.88.0 after update. Check your active toolchain."
        );
    }

    print_ok(&format!("Rust updated to {new_v}"));
    println!();
    Ok(())
}

pub fn ensure_rust_version() -> Result<()> {
    match get_version("rustc", &["--version"]) {
        Some(version)
            if parse_semver(&version).map_or(false, |(maj, min, _)| maj >= 1 && min >= 88) =>
        {
            return Ok(());
        }
        Some(version) => {
            println!(
                "\n  {} {}: {}",
                "!".truecolor(234, 179, 8).bold(),
                "Rust".truecolor(234, 179, 8),
                format!("{version} — requires rustc ≥ 1.88.0").truecolor(113, 113, 122),
            );
            if !has_rustup() {
                install_rustup()?;
            }
            rustup_update()?;
        }
        None => {
            println!(
                "\n  {} {}: {}",
                "!".truecolor(234, 179, 8).bold(),
                "Rust".truecolor(234, 179, 8),
                "not found".truecolor(113, 113, 122),
            );
            if !has_rustup() {
                install_rustup()?;
            }
            rustup_update()?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Package manager
// ---------------------------------------------------------------------------

const PM_ORDER: &[(&str, &str)] = &[
    ("bun", "Bun"),
    ("pnpm", "pnpm"),
    ("yarn", "Yarn"),
    ("npm", "npm"),
];

fn install_bun() -> Result<()> {
    print_action("Installing bun…");

    #[cfg(unix)]
    {
        let ok = Command::new("sh")
            .args(["-c", "curl -fsSL https://bun.sh/install | bash"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            anyhow::bail!("Failed to install bun. Install manually from https://bun.sh");
        }
        let bun_install = std::env::var("BUN_INSTALL")
            .unwrap_or_else(|_| format!("{}/.bun", std::env::var("HOME").unwrap_or_default()));
        let bun_bin = format!("{bun_install}/bin");
        if let Ok(path) = std::env::var("PATH") {
            std::env::set_var("PATH", format!("{bun_bin}:{path}"));
        }
    }

    #[cfg(windows)]
    {
        let ok = Command::new("powershell")
            .args(["-NoProfile", "-Command", "irm bun.sh/install.ps1 | iex"])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok {
            anyhow::bail!("Failed to install bun. Install manually from https://bun.sh");
        }
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let bun_bin = format!("{userprofile}\\.bun\\bin");
        if let Ok(path) = std::env::var("PATH") {
            std::env::set_var("PATH", format!("{bun_bin};{path}"));
        }
    }

    print_ok("bun installed");
    println!();
    Ok(())
}

/// Returns the run-command prefix for the selected PM, e.g. "bun" or "npm run".
pub fn pm_run_prefix(pm: &str) -> &'static str {
    match pm {
        "bun" => "bun",
        "pnpm" => "pnpm",
        "yarn" => "yarn",
        _ => "npm run",
    }
}

/// Returns the command the user should type to run tauri dev.
pub fn pm_tauri_dev(pm: &str) -> &'static str {
    match pm {
        "bun" => "bun run tauri dev",
        "pnpm" => "pnpm tauri dev",
        "yarn" => "yarn tauri dev",
        _ => "npx tauri dev",
    }
}

/// Ensure a package manager is available. Returns the name of the PM to use.
///
/// - If `preferred` is `Some` and already installed → return it.
/// - If `preferred` is `Some("bun")` and missing → auto-install bun.
/// - If `preferred` is `Some` but not bun and missing → bail with instructions.
/// - If `None` → auto-detect first available from bun > pnpm > yarn > npm.
/// - If nothing found → install bun as default.
pub fn ensure_package_manager(preferred: Option<&str>) -> Result<String> {
    if let Some(pm) = preferred {
        if cmd_exists(pm) {
            return Ok(pm.to_string());
        }
        if pm == "bun" {
            install_bun()?;
            if cmd_exists("bun") {
                return Ok("bun".into());
            }
            anyhow::bail!("bun installation succeeded but binary not found in PATH");
        }
        anyhow::bail!(
            "{pm} not found. Install it first:\n  \
             pnpm → npm i -g pnpm\n  \
             yarn → npm i -g yarn\n  \
             npm  → install Node.js from https://nodejs.org"
        );
    }

    for (name, _) in PM_ORDER {
        if cmd_exists(name) {
            return Ok(name.to_string());
        }
    }

    install_bun()?;
    if cmd_exists("bun") {
        return Ok("bun".into());
    }
    anyhow::bail!("No package manager found and bun auto-install failed");
}

// ---------------------------------------------------------------------------
// macOS — Xcode Command Line Tools
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn has_xcode_clt() -> bool {
    Command::new("xcode-select")
        .arg("-p")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn check_xcode_clt() -> Check {
    if has_xcode_clt() {
        Check {
            name: "Xcode CLT",
            status: Status::Ok,
            detail: "installed".into(),
        }
    } else {
        Check {
            name: "Xcode CLT",
            status: Status::Missing,
            detail: "not found — run: xcode-select --install".into(),
        }
    }
}

#[cfg(target_os = "macos")]
pub fn ensure_xcode_clt() -> Result<()> {
    if has_xcode_clt() {
        return Ok(());
    }

    println!(
        "\n  {} {}: {}",
        "!".truecolor(234, 179, 8).bold(),
        "Xcode CLT".truecolor(234, 179, 8),
        "not found".truecolor(113, 113, 122),
    );
    print_action("Installing Xcode Command Line Tools…");
    println!("    A system dialog may appear — click \"Install\" to continue.\n");

    Command::new("xcode-select")
        .arg("--install")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok();

    // xcode-select --install launches an async macOS installer dialog.
    // Poll until the tools appear or we hit a timeout.
    let poll_interval = std::time::Duration::from_secs(5);
    let max_wait = std::time::Duration::from_secs(600); // 10 min
    let start = std::time::Instant::now();

    while !has_xcode_clt() {
        if start.elapsed() > max_wait {
            anyhow::bail!(
                "Timed out waiting for Xcode CLT installation. \
                 Complete the install manually, then run `crabyard new` again."
            );
        }
        println!(
            "    {} waiting for Xcode CLT installer…",
            "⏳".truecolor(113, 113, 122)
        );
        std::thread::sleep(poll_interval);
    }

    print_ok("Xcode Command Line Tools installed");
    println!();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn ensure_xcode_clt() -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Windows — MSVC Build Tools
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn has_msvc() -> bool {
    // Check via vswhere (ships with VS 2017+)
    let vswhere_paths = [
        r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe",
        r"C:\Program Files\Microsoft Visual Studio\Installer\vswhere.exe",
    ];

    for path in &vswhere_paths {
        if std::path::Path::new(path).exists() {
            let ok = Command::new(path)
                .args([
                    "-products",
                    "*",
                    "-latest",
                    "-requires",
                    "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                    "-property",
                    "installationPath",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| String::from_utf8(o.stdout).ok())
                .map(|s| !s.trim().is_empty())
                .unwrap_or(false);
            if ok {
                return true;
            }
        }
    }

    // Fallback: check if cl.exe is reachable
    Command::new("where")
        .arg("cl")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn check_msvc() -> Check {
    if has_msvc() {
        Check {
            name: "MSVC Build Tools",
            status: Status::Ok,
            detail: "installed".into(),
        }
    } else {
        Check {
            name: "MSVC Build Tools",
            status: Status::Missing,
            detail: "not found — install Visual Studio Build Tools with C++ workload".into(),
        }
    }
}

#[cfg(target_os = "windows")]
pub fn ensure_msvc() -> Result<()> {
    if has_msvc() {
        return Ok(());
    }

    println!(
        "\n  {} {}: {}",
        "!".truecolor(234, 179, 8).bold(),
        "MSVC Build Tools".truecolor(234, 179, 8),
        "not found".truecolor(113, 113, 122),
    );

    // Try winget first
    if cmd_exists("winget") {
        print_action("Installing Visual Studio Build Tools via winget…");
        println!("    This may take several minutes.\n");

        let ok = Command::new("winget")
            .args([
                "install",
                "Microsoft.VisualStudio.2022.BuildTools",
                "--override",
                "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --add Microsoft.VisualStudio.Component.VC.Tools.x86.x64 --add Microsoft.VisualStudio.Component.Windows11SDK.22621",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false);

        if ok && has_msvc() {
            print_ok("MSVC Build Tools installed");
            println!();
            return Ok(());
        }
    }

    // Fallback: direct download
    print_action("Downloading Visual Studio Build Tools installer…");
    let ok = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$url = 'https://aka.ms/vs/17/release/vs_BuildTools.exe'; $out = \"$env:TEMP\\vs_BuildTools.exe\"; Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing; Start-Process -FilePath $out -ArgumentList '--wait','--passive','--add','Microsoft.VisualStudio.Workload.VCTools','--add','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','--add','Microsoft.VisualStudio.Component.Windows11SDK.22621' -Wait",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok || !has_msvc() {
        anyhow::bail!(
            "MSVC Build Tools installation failed.\n  \
             Install manually: https://visualstudio.microsoft.com/visual-cpp-build-tools/\n  \
             Select the \"Desktop development with C++\" workload."
        );
    }

    print_ok("MSVC Build Tools installed");
    println!();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_msvc() -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Windows — WebView2
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn has_webview2() -> bool {
    let key =
        r"SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey(key)
        .is_ok()
}

#[cfg(target_os = "windows")]
fn check_webview2() -> Check {
    if has_webview2() {
        Check {
            name: "WebView2",
            status: Status::Ok,
            detail: "installed".into(),
        }
    } else {
        Check {
            name: "WebView2",
            status: Status::Missing,
            detail: "not found — will be auto-installed".into(),
        }
    }
}

#[cfg(target_os = "windows")]
pub fn ensure_webview2() -> Result<()> {
    if has_webview2() {
        return Ok(());
    }

    println!(
        "\n  {} {}: {}",
        "!".truecolor(234, 179, 8).bold(),
        "WebView2".truecolor(234, 179, 8),
        "not found".truecolor(113, 113, 122),
    );
    print_action("Installing WebView2 runtime…");

    let ok = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "$url = 'https://go.microsoft.com/fwlink/p/?LinkId=2124703'; $out = \"$env:TEMP\\MicrosoftEdgeWebview2Setup.exe\"; Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing; $proc = Start-Process -FilePath $out -ArgumentList '/silent','/install' -PassThru; if (!$proc.WaitForExit(900000)) { $proc.Kill(); exit 1 }; exit $proc.ExitCode",
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !ok || !has_webview2() {
        anyhow::bail!(
            "WebView2 installation failed.\n  \
             Download manually: https://developer.microsoft.com/en-us/microsoft-edge/webview2/"
        );
    }

    print_ok("WebView2 runtime installed");
    println!();
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_webview2() -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Linux — system libraries
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
const LINUX_LIBS: &[&str] = &[
    "webkit2gtk-4.1",
    "javascriptcoregtk-4.1",
    "gtk+-3.0",
    "libsoup-3.0",
];

#[cfg(target_os = "linux")]
fn has_cmd(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn find_missing_linux_libs() -> Vec<&'static str> {
    // If pkg-config itself is missing, install deps unconditionally
    // (pkg-config will be installed alongside the dev libraries)
    if !has_cmd("pkg-config") && !has_cmd("pkgconf") {
        return LINUX_LIBS.to_vec();
    }
    let pc = if has_cmd("pkg-config") {
        "pkg-config"
    } else {
        "pkgconf"
    };
    LINUX_LIBS
        .iter()
        .copied()
        .filter(|lib| {
            !Command::new(pc)
                .args(["--exists", lib])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        })
        .collect()
}

#[cfg(target_os = "linux")]
enum Distro {
    Debian,
    Fedora,
    Arch,
    Suse,
    Void,
    Alpine,
    Unknown,
}

#[cfg(target_os = "linux")]
fn detect_distro() -> Distro {
    if has_cmd("apt-get") {
        Distro::Debian
    } else if has_cmd("dnf") {
        Distro::Fedora
    } else if has_cmd("pacman") {
        Distro::Arch
    } else if has_cmd("zypper") {
        Distro::Suse
    } else if has_cmd("xbps-install") {
        Distro::Void
    } else if has_cmd("apk") {
        Distro::Alpine
    } else {
        Distro::Unknown
    }
}

#[cfg(target_os = "linux")]
fn linux_install_cmd(distro: &Distro) -> Option<(&'static str, &'static [&'static str])> {
    match distro {
        Distro::Debian => Some((
            "apt-get",
            &[
                "install", "-y", "pkg-config", "curl",
                "libwebkit2gtk-4.1-dev", "libjavascriptcoregtk-4.1-dev",
                "libgtk-3-dev", "libsoup-3.0-dev", "libssl-dev",
                "libayatana-appindicator3-dev",
            ],
        )),
        Distro::Fedora => Some((
            "dnf",
            &[
                "install", "-y", "pkg-config", "curl",
                "webkit2gtk4.1-devel", "javascriptcoregtk4.1-devel",
                "gtk3-devel", "libsoup3-devel", "openssl-devel",
                "libappindicator-gtk3-devel",
            ],
        )),
        Distro::Arch => Some((
            "pacman",
            &[
                "-S", "--needed", "--noconfirm", "pkgconf", "curl",
                "webkit2gtk-4.1", "gtk3", "libsoup3", "openssl",
                "libayatana-appindicator",
            ],
        )),
        Distro::Suse => Some((
            "zypper",
            &[
                "install", "-y", "pkg-config", "curl",
                "webkit2gtk3-soup2-devel", "gtk3-devel",
                "libsoup-devel", "libopenssl-devel",
                "libappindicator3-devel",
            ],
        )),
        Distro::Void => Some((
            "xbps-install",
            &[
                "-Sy", "pkg-config", "curl",
                "webkit2gtk-devel", "gtk+3-devel",
                "libsoup3-devel", "openssl-devel",
                "libayatana-appindicator-devel",
            ],
        )),
        Distro::Alpine => Some((
            "apk",
            &[
                "add", "pkgconf", "curl",
                "webkit2gtk-dev", "gtk+3.0-dev",
                "libsoup3-dev", "openssl-dev",
            ],
        )),
        Distro::Unknown => None,
    }
}

#[cfg(target_os = "linux")]
fn linux_install_hint(distro: &Distro) -> String {
    match distro {
        Distro::Debian => "sudo apt install pkg-config libwebkit2gtk-4.1-dev libjavascriptcoregtk-4.1-dev libgtk-3-dev libsoup-3.0-dev libssl-dev libayatana-appindicator3-dev".into(),
        Distro::Fedora => "sudo dnf install pkg-config webkit2gtk4.1-devel javascriptcoregtk4.1-devel gtk3-devel libsoup3-devel openssl-devel libappindicator-gtk3-devel".into(),
        Distro::Arch => "sudo pacman -S pkgconf webkit2gtk-4.1 gtk3 libsoup3 openssl libayatana-appindicator".into(),
        Distro::Suse => "sudo zypper install pkg-config webkit2gtk3-soup2-devel gtk3-devel libsoup-devel libopenssl-devel libappindicator3-devel".into(),
        Distro::Void => "sudo xbps-install -Sy pkg-config webkit2gtk-devel gtk+3-devel libsoup3-devel openssl-devel libayatana-appindicator-devel".into(),
        Distro::Alpine => "sudo apk add pkgconf webkit2gtk-dev gtk+3.0-dev libsoup3-dev openssl-dev".into(),
        Distro::Unknown => "see https://v2.tauri.app/start/prerequisites/#linux".into(),
    }
}

#[cfg(target_os = "linux")]
fn check_linux_deps() -> Check {
    let missing = find_missing_linux_libs();
    if missing.is_empty() {
        Check {
            name: "Linux deps",
            status: Status::Ok,
            detail: "webkit2gtk-4.1, javascriptcoregtk-4.1, gtk+-3.0, libsoup-3.0 found".into(),
        }
    } else {
        let distro = detect_distro();
        Check {
            name: "Linux deps",
            status: Status::Missing,
            detail: format!(
                "missing: {} — {}",
                missing.join(", "),
                linux_install_hint(&distro)
            ),
        }
    }
}

#[cfg(target_os = "linux")]
fn install_linux_deps() -> Result<()> {
    let distro = detect_distro();
    let (pm, args) = match linux_install_cmd(&distro) {
        Some(cmd) => cmd,
        None => anyhow::bail!(
            "Could not detect your package manager. Install the Tauri system libraries manually:\n  \
             see https://v2.tauri.app/start/prerequisites/#linux"
        ),
    };

    print_action("Installing system libraries via sudo…");
    println!();

    let ok = Command::new("sudo")
        .arg(pm)
        .args(args)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    println!();

    if !ok {
        anyhow::bail!(
            "Package installation failed. Install manually:\n  {}",
            linux_install_hint(&distro)
        );
    }

    let still_missing = find_missing_linux_libs();
    if !still_missing.is_empty() {
        anyhow::bail!(
            "Still missing after install: {}. Install manually:\n  {}",
            still_missing.join(", "),
            linux_install_hint(&distro)
        );
    }

    print_ok("System libraries installed");
    println!();
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn ensure_linux_deps() -> Result<()> {
    let missing = find_missing_linux_libs();
    if missing.is_empty() {
        return Ok(());
    }

    let distro = detect_distro();

    println!(
        "\n  {} {}: {}",
        "!".truecolor(234, 179, 8).bold(),
        "Linux deps".truecolor(234, 179, 8),
        format!("missing: {}", missing.join(", ")).truecolor(113, 113, 122),
    );

    if linux_install_cmd(&distro).is_none() {
        println!(
            "\n  {} {}\n",
            "→".truecolor(6, 182, 212).bold(),
            "see https://v2.tauri.app/start/prerequisites/#linux".truecolor(228, 228, 231),
        );
        anyhow::bail!(
            "Could not detect your package manager. Install the missing libraries manually, then run `crabyard new` again."
        );
    }

    let should_install = Confirm::new()
        .with_prompt("    Install system libraries now? (requires sudo)")
        .default(true)
        .interact()
        .unwrap_or(false);

    if !should_install {
        println!(
            "\n  {} {}\n",
            "→".truecolor(6, 182, 212).bold(),
            linux_install_hint(&distro).truecolor(228, 228, 231),
        );
        anyhow::bail!(
            "Install the missing system libraries listed above, then run `crabyard new` again."
        );
    }

    println!();
    install_linux_deps()
}

#[cfg(not(target_os = "linux"))]
pub fn ensure_linux_deps() -> Result<()> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Doctor — check & report
// ---------------------------------------------------------------------------

fn check_rust() -> Check {
    let version = match get_version("rustc", &["--version"]) {
        Some(v) => v,
        None => {
            return Check {
                name: "Rust",
                status: Status::Missing,
                detail: "not found — install from https://rustup.rs".into(),
            };
        }
    };

    if parse_semver(&version).map_or(false, |(maj, min, _)| maj >= 1 && min >= 88) {
        return Check {
            name: "Rust",
            status: Status::Ok,
            detail: version,
        };
    }

    let has_rustup = Command::new("rustup")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if !has_rustup {
        return Check {
            name: "Rust",
            status: Status::Warning,
            detail: format!(
                "{version} — requires rustc ≥ 1.88.0, install rustup from https://rustup.rs"
            ),
        };
    }

    println!(
        "\n  {} {}: {}",
        "!".truecolor(234, 179, 8).bold(),
        "Rust".truecolor(234, 179, 8),
        format!("{version} — requires rustc ≥ 1.88.0").truecolor(113, 113, 122),
    );

    let should_update = Confirm::new()
        .with_prompt("    Run `rustup update` now?")
        .default(true)
        .interact()
        .unwrap_or(false);

    if !should_update {
        return Check {
            name: "Rust",
            status: Status::Warning,
            detail: format!("{version} — requires rustc ≥ 1.88.0, run: rustup update"),
        };
    }

    println!();
    let update_ok = Command::new("rustup")
        .arg("update")
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    println!();

    if !update_ok {
        return Check {
            name: "Rust",
            status: Status::Warning,
            detail: format!("{version} — rustup update failed, update manually"),
        };
    }

    match get_version("rustc", &["--version"]) {
        Some(new_v)
            if parse_semver(&new_v).map_or(false, |(maj, min, _)| maj >= 1 && min >= 88) =>
        {
            Check {
                name: "Rust",
                status: Status::Ok,
                detail: format!("{new_v} (updated)"),
            }
        }
        Some(new_v) => Check {
            name: "Rust",
            status: Status::Warning,
            detail: format!("{new_v} — still below 1.88.0, check your active toolchain"),
        },
        None => Check {
            name: "Rust",
            status: Status::Warning,
            detail: "rustc not found after update — check your PATH".into(),
        },
    }
}

fn check_cargo() -> Check {
    match get_version("cargo", &["--version"]) {
        Some(v) => Check {
            name: "Cargo",
            status: Status::Ok,
            detail: v,
        },
        None => Check {
            name: "Cargo",
            status: Status::Missing,
            detail: "not found — install Rust from https://rustup.rs".into(),
        },
    }
}

fn check_node() -> Check {
    match get_version("node", &["--version"]) {
        Some(v) => Check {
            name: "Node.js",
            status: Status::Ok,
            detail: v,
        },
        None => Check {
            name: "Node.js",
            status: Status::Warning,
            detail: "not found — optional if using Bun".into(),
        },
    }
}

fn check_package_manager() -> Check {
    for (name, label) in PM_ORDER {
        if let Some(v) = get_version(name, &["--version"]) {
            return Check {
                name: "Package manager",
                status: Status::Ok,
                detail: format!("{label} {v}"),
            };
        }
    }
    Check {
        name: "Package manager",
        status: Status::Missing,
        detail: "none found (bun/pnpm/yarn/npm) — install Bun or Node.js".into(),
    }
}

fn check_git() -> Check {
    match get_version("git", &["--version"]) {
        Some(v) => Check {
            name: "Git",
            status: Status::Ok,
            detail: v,
        },
        None => Check {
            name: "Git",
            status: Status::Warning,
            detail: "not found — optional, needed for `git init` during project creation".into(),
        },
    }
}

fn check_tauri_cli() -> Check {
    if let Some(v) = get_version("cargo-tauri", &["--version"]) {
        return Check {
            name: "Tauri CLI",
            status: Status::Ok,
            detail: v,
        };
    }
    if let Some(v) = get_version("cargo", &["tauri", "--version"]) {
        return Check {
            name: "Tauri CLI",
            status: Status::Ok,
            detail: v,
        };
    }
    Check {
        name: "Tauri CLI",
        status: Status::Warning,
        detail: "not found — install with: cargo install tauri-cli --locked".into(),
    }
}

pub fn run() -> Result<()> {
    println!();
    crate::tui::banner::print_inline_banner();
    crate::tui::banner::print_inline_separator();
    println!(
        "  {}",
        format!("Doctor v{}", env!("GIT_VERSION")).truecolor(161, 161, 170)
    );
    println!();

    let mut checks = vec![
        check_rust(),
        check_cargo(),
        check_node(),
        check_package_manager(),
        check_git(),
        check_tauri_cli(),
    ];

    #[cfg(target_os = "macos")]
    checks.push(check_xcode_clt());

    #[cfg(target_os = "linux")]
    checks.push(check_linux_deps());

    #[cfg(target_os = "windows")]
    {
        checks.push(check_msvc());
        checks.push(check_webview2());
    }

    let mut issues = 0u32;

    for c in &checks {
        match c.status {
            Status::Ok => {
                println!(
                    "  {} {}: {}",
                    "✓".truecolor(34, 197, 94).bold(),
                    c.name.truecolor(228, 228, 231),
                    c.detail.truecolor(113, 113, 122)
                );
            }
            Status::Warning => {
                println!(
                    "  {} {}: {}",
                    "!".truecolor(234, 179, 8).bold(),
                    c.name.truecolor(234, 179, 8),
                    c.detail.truecolor(113, 113, 122)
                );
            }
            Status::Missing => {
                println!(
                    "  {} {}: {}",
                    "✗".truecolor(239, 68, 68).bold(),
                    c.name.truecolor(239, 68, 68),
                    c.detail.truecolor(239, 68, 68)
                );
                issues += 1;
            }
        }
    }

    println!();
    crate::tui::banner::print_inline_separator();
    if issues == 0 {
        println!(
            "  {}",
            "🦀 All checks passed. You're ready to build!"
                .truecolor(34, 197, 94)
                .bold()
        );
    } else {
        println!(
            "  {}",
            format!(
                "{} issue{} found. Install missing dependencies before running `crabyard new`.",
                issues,
                if issues == 1 { "" } else { "s" }
            )
            .truecolor(239, 68, 68)
            .bold()
        );
    }
    crate::tui::banner::print_inline_separator();
    println!();

    Ok(())
}
