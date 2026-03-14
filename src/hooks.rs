use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};

/// Initialize a git repository in `dir` and create an initial commit.
pub fn git_init(dir: &Path) -> Result<()> {
    run_cmd("git", &["init"], dir, "git init")?;
    run_cmd("git", &["add", "-A"], dir, "git add")?;
    run_cmd(
        "git",
        &["commit", "-m", "chore: initial Crabyard scaffold"],
        dir,
        "git commit",
    )?;
    Ok(())
}

/// Install frontend dependencies using the specified package manager.
pub fn install_deps(dir: &Path, pm: &str) -> Result<()> {
    run_cmd(pm, &["install"], dir, &format!("{pm} install"))
}

fn run_cmd(program: &str, args: &[&str], dir: &Path, label: &str) -> Result<()> {
    let output = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .with_context(|| format!("Failed to run `{label}` — is `{program}` installed?"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().last().unwrap_or("").trim();
        if detail.is_empty() {
            anyhow::bail!("`{label}` exited with {}", output.status);
        }
        anyhow::bail!("`{label}` failed: {detail}");
    }
    Ok(())
}
