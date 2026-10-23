use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

struct GeneratedProject(PathBuf);

impl Drop for GeneratedProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_crabyard"))
        .args(args)
        .env("PATH", "")
        .env_remove("CRABYARD_TEMPLATE")
        .env_remove("CRABYARD_LICENSE_KEY")
        .env("SOURCE_DATE_EPOCH", "1700000000")
        .output()
        .unwrap()
}

fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect();
    files.sort_by(|left, right| left.0.cmp(&right.0));
    files
}

#[test]
#[ignore = "requires the pinned real template checkout in CRABYARD_TEST_TEMPLATE"]
fn real_template_generates_auditable_projects_and_secret_free_ai_context() {
    let template = std::env::var("CRABYARD_TEST_TEMPLATE")
        .expect("Set CRABYARD_TEST_TEMPLATE to the supported template checkout");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project = GeneratedProject(
        std::env::temp_dir().join(format!("crabyard-template-{}-{nonce}", std::process::id())),
    );
    let path = project.0.to_str().unwrap();
    let result = cli(&[
        "new",
        "Fieldnotes",
        "--slug",
        "fieldnotes",
        "--bundle-id",
        "com.crabyard.fieldnotes",
        "--preset",
        "local-first",
        "--pm",
        "npm",
        "--template",
        &template,
        "--output",
        path,
        "--brief",
        "Build a local-first notebook with search and Markdown export.",
        "--no-git",
        "--no-install",
    ]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(project.0.join("docs/AI_CONTEXT.md").is_file());
    let before = snapshot(&project.0);
    let checked = cli(&["check", "--project", path, "--json"]);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&checked.stdout).unwrap();
    assert_eq!(report["summary"]["errors"], 0);
    assert_eq!(
        report["summary"]["warnings"], 2,
        "Only uninstalled lockfiles should require action: {report}"
    );
    let strict = cli(&["check", "--project", path, "--json", "--strict"]);
    assert_eq!(strict.status.code(), Some(1));
    let preview = cli(&[
        "context",
        "--project",
        path,
        "--brief",
        "Add notebook tagging.\nAPI_TOKEN=do-not-export-this",
        "--dry-run",
    ]);
    assert!(preview.status.success());
    let context = String::from_utf8(preview.stdout).unwrap();
    assert!(context.contains("Add notebook tagging."));
    assert!(!context.contains("do-not-export-this"));
    assert!(context.contains("docs/HANDOFF.md"));
    assert_eq!(snapshot(&project.0), before);
}
