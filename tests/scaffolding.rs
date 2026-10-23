use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEST: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "crabyard-cli-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn template(&self) -> PathBuf {
        let template = self.path("template");
        for (path, content) in [
            ("base/package.json", r#"{"name":"{{APP_SLUG}}","version":"{{APP_VERSION}}","description":"{{APP_DESCRIPTION}}","scripts":{"check":"verify","test":"test","build":"build"},"dependencies":{"react":"^19"}}"#),
            ("base/manifest.toml", "[project]\napp_name = \"{{APP_NAME}}\"\nauthor = \"{{APP_AUTHOR}}\"\napp_slug = \"{{APP_SLUG}}\"\napp_bundle_id = \"{{APP_BUNDLE_ID}}\"\napp_version = \"{{APP_VERSION}}\"\n[modules.auth]\nselected = \"{{AUTH_MODULE}}\"\n[modules.ui]\nselected = \"{{UI_MODULE}}\"\n[modules.package_manager]\nselected = \"{{PACKAGE_MANAGER}}\"\n"),
            ("base/src-tauri/Cargo.toml", "[package]\nname = \"{{APP_SLUG}}\"\nversion = \"{{APP_VERSION}}\"\n[dependencies]\ntauri = { version = \"2\", features = [\"custom-protocol\"] }\n"),
            ("base/src-tauri/tauri.conf.json", r#"{"productName":"{{APP_NAME}}","identifier":"{{APP_BUNDLE_ID}}","version":"{{APP_VERSION}}","app":{"security":{"csp":"default-src 'self'"}},"bundle":{"createUpdaterArtifacts":false}}"#),
            ("base/src-tauri/src/lib.rs", "fn main() { tauri::Builder::default().setup(|app| { Ok(()) }); }\n"),
            ("base/src-tauri/capabilities/default.json", r#"{"windows":["main"],"permissions":["core:default"]}"#),
            ("base/index.html", "<title>{{APP_NAME}}</title>\n"),
            ("base/src/title.ts", "const title = \"{{APP_NAME}}\";\n"),
            ("auth/none/.env.example", "VITE_APP_NAME={{APP_NAME}}\n"),
            ("ui/minimal/module.json", "{}"),
            ("ui/shadcn/module.json", "{}"),
            ("auth/github/.env.example", "GITHUB_CLIENT_ID=your_github_client_id_here\n"),
            ("base/.gitignore", ".env\n.env.*\n!.env.example\n*.key\n*.p12\n"),
            ("base/AGENTS.md", "# Project instructions\nUse acceptance criteria and verify changes.\n"),
            ("base/CLAUDE.md", "Read AGENTS.md.\n"),
            ("base/GEMINI.md", "Read AGENTS.md.\n"),
            ("base/.github/copilot-instructions.md", "Read AGENTS.md.\n"),
            ("base/.cursor/rules/project.mdc", "Read AGENTS.md.\n"),
            ("base/docs/PROJECT.md", "# Product brief\nDefine the user's problem and acceptance criteria.\n"),
            ("base/docs/tasks/FEATURE.md", "# Feature acceptance criteria\n"),
            ("base/docs/HANDOFF.md", "# Implementation handoff\n"),
            ("base/.github/workflows/ci.yml", "name: CI\non: push\njobs:\n  test:\n    runs-on: ubuntu-latest\n    steps:\n      - run: npm run check\n"),
            ("base/.github/workflows/release.yml", "name: Release\non: workflow_dispatch\njobs:\n  release:\n    runs-on: ubuntu-latest\n    env:\n      TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}\n    steps:\n      - run: npm run build\n"),
        ] {
            write(&template.join(path), content);
        }
        template
    }

    fn cli(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_crabyard"))
            .args(args)
            .current_dir(&self.0)
            .env("PATH", "")
            .env("SOURCE_DATE_EPOCH", "1700000000")
            .env_remove("CRABYARD_TEMPLATE")
            .env_remove("CRABYARD_LICENSE_KEY")
            .output()
            .unwrap()
    }

    fn generate_project(&self, pm: &str) -> PathBuf {
        self.template();
        assert_success(&self.cli(&[
            "new",
            "Field Notes",
            "--slug",
            "field-notes",
            "--bundle-id",
            "com.crabyard.fieldnotes",
            "--preset",
            "minimal",
            "--pm",
            pm,
            "--template",
            "template",
            "--output",
            "project",
            "--no-git",
            "--no-install",
        ]));
        self.path("project")
    }

    fn lock_project(&self, pm: &str) {
        let root = self.path("project");
        let package: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
        let root_package = serde_json::json!({"name":package["name"], "version":package["version"], "dependencies":package["dependencies"]});
        let (name, text) = match pm {
            "npm" => ("package-lock.json", serde_json::to_string_pretty(&serde_json::json!({"lockfileVersion":3,"packages":{"":root_package}})).unwrap()),
            "bun" => ("bun.lock", format!("// Bun text lockfile\n{}\n", serde_json::to_string_pretty(&serde_json::json!({"lockfileVersion":1,"workspaces":{"":root_package},"packages":{}})).unwrap())),
            "pnpm" => {
                let deps: serde_json::Map<_, _> = package["dependencies"].as_object().unwrap().iter().map(|(name, version)| (name.clone(), serde_json::json!({"specifier":version,"version":"19.0.0"}))).collect();
                let document = serde_yaml_ng::to_string(&serde_json::json!({"lockfileVersion":"9.0","importers":{".":{"dependencies":deps}}})).unwrap();
                ("pnpm-lock.yaml", format!("---\nlockfileVersion: '9.0'\nconfigDependencies: {{}}\n---\n{document}"))
            }
            "yarn" => ("yarn.lock", package["dependencies"].as_object().unwrap().iter().map(|(name, version)| format!("\"{}@{}\":\n  version \"19.0.0\"\n", name, version.as_str().unwrap())).collect()),
            _ => unreachable!(),
        };
        write(&root.join(name), &text);
        write(
            &root.join("src-tauri/Cargo.lock"),
            "version = 4\n[[package]]\nname = \"field-notes\"\nversion = \"0.1.0\"\ndependencies = [\"tauri\"]\n",
        );
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn assert_success(result: &Output) {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn snapshot(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry.path().strip_prefix(root).unwrap().to_path_buf(),
                fs::read(entry.path()).unwrap(),
            )
        })
        .collect()
}

fn has_finding(report: &serde_json::Value, id: &str) -> bool {
    report["findings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|finding| finding["id"] == id)
}

#[test]
fn presets_are_machine_readable() {
    let workspace = Workspace::new();
    let result = workspace.cli(&["presets", "--json"]);
    assert_success(&result);
    let presets: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(presets.as_array().unwrap().len(), 3);
    assert_eq!(presets[2]["name"], "local-first");
    assert!(presets[2]["extras"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("sql")));
}

#[test]
fn dry_run_resolves_preset_and_overrides_without_side_effects() {
    let workspace = Workspace::new();
    let result = workspace.cli(&[
        "new",
        "Workspace",
        "--preset",
        "local-first",
        "--auth",
        "github",
        "--pm",
        "pnpm",
        "--dry-run",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&result);
    let plan: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(plan["recipe"]["slug"], "workspace");
    assert_eq!(plan["recipe"]["auth"], "github");
    assert_eq!(plan["recipe"]["pm"], "pnpm");
    assert_eq!(plan["host_setup"], false);
    assert_eq!(plan["install_dependencies"], false);
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
}

#[test]
fn rejects_invalid_inputs_before_template_resolution() {
    for (option, value, expected) in [
        ("--slug", "../escape", "Slug must"),
        ("--slug", "123-app", "Slug must"),
        ("--bundle-id", "invalid", "Bundle ID"),
        ("--auth", "../github", "Auth module"),
        ("--ui", "unknown", "UI framework"),
        ("--pm", "npm && bad", "Package manager"),
        ("--app-version", "next", "semantic versioning"),
        ("--extras", "made-up", "Unknown feature"),
        ("--description", "{{APP_NAME}}", "template tokens"),
    ] {
        let workspace = Workspace::new();
        let result = workspace.cli(&[
            "new",
            "App",
            "--yes",
            option,
            value,
            "--template",
            "missing",
        ]);
        assert!(!result.status.success());
        assert!(
            String::from_utf8_lossy(&result.stderr).contains(expected),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
    }
}

#[test]
fn headless_generation_needs_no_host_tooling_and_escapes_metadata() {
    let workspace = Workspace::new();
    workspace.template();
    let name = "A \"quoted\" & <desktop>";
    let description = "A \\\"description\"";
    let result = workspace.cli(&[
        "new",
        name,
        "--slug",
        "quoted-app",
        "--author",
        "A \"Person\"",
        "--description",
        description,
        "--preset",
        "minimal",
        "--pm",
        "npm",
        "--template",
        "template",
        "--output",
        "output",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&result);
    let output = workspace.path("output");
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(output.join("package.json")).unwrap()).unwrap();
    assert_eq!(package["description"], description);
    let tauri: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output.join("src-tauri/tauri.conf.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(tauri["productName"], name);
    let manifest: toml::Value =
        toml::from_str(&fs::read_to_string(output.join("manifest.toml")).unwrap()).unwrap();
    assert_eq!(manifest["project"]["app_name"].as_str(), Some(name));
    assert_eq!(manifest["project"]["author"].as_str(), Some("A \"Person\""));
    assert!(fs::read_to_string(output.join("index.html"))
        .unwrap()
        .contains("A &quot;quoted&quot; &amp; &lt;desktop&gt;"));
    assert_eq!(
        fs::read_to_string(output.join(".env")).unwrap(),
        format!("VITE_APP_NAME={}\n", serde_json::to_string(name).unwrap())
    );
    assert!(output.join("crabyard.recipe.toml").is_file());
    assert!(!output.join(".git").exists());
}

#[test]
fn recipe_replay_preserves_choices_and_accepts_cli_overrides() {
    let workspace = Workspace::new();
    workspace.template();
    let initial = workspace.cli(&[
        "new",
        "Original",
        "--preset",
        "local-first",
        "--pm",
        "npm",
        "--app-version",
        "3.4.5",
        "--author",
        "Example",
        "--template",
        "template",
        "--output",
        "original",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&initial);
    let result = workspace.cli(&[
        "new",
        "Renamed",
        "--recipe",
        "original/crabyard.recipe.toml",
        "--app-version",
        "2.0.1",
        "--dry-run",
        "--output",
        "replayed",
    ]);
    assert_success(&result);
    let plan: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(plan["recipe"]["app_name"], "Renamed");
    assert_eq!(plan["recipe"]["slug"], "original");
    assert_eq!(plan["recipe"]["app_version"], "2.0.1");
    assert_eq!(plan["recipe"]["pm"], "npm");
    assert_eq!(plan["recipe"]["author"], "Example");
    assert!(plan["recipe"]["extras"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("sql")));
    let package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(workspace.path("original/package.json")).unwrap())
            .unwrap();
    assert_eq!(package["dependencies"]["@tauri-apps/plugin-sql"], "~2");
    assert!(!workspace.path("replayed").exists());
    let replay = workspace.cli(&[
        "new",
        "--recipe",
        "original/crabyard.recipe.toml",
        "--template",
        "template",
        "--output",
        "replayed",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&replay);
    let original = workspace.path("original");
    for entry in walkdir::WalkDir::new(&original) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let relative = entry.path().strip_prefix(&original).unwrap();
            assert_eq!(
                fs::read(entry.path()).unwrap(),
                fs::read(workspace.path("replayed").join(relative)).unwrap(),
                "{}",
                relative.display()
            );
        }
    }
}

#[test]
fn output_cannot_recurse_into_template() {
    let workspace = Workspace::new();
    workspace.template();
    for output in [
        "template/base/generated",
        "missing/../template/base/generated",
    ] {
        let result = workspace.cli(&[
            "new",
            "App",
            "--preset",
            "minimal",
            "--template",
            "template",
            "--output",
            output,
            "--no-git",
            "--no-install",
        ]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains("outside the template directory"));
        assert!(!workspace.path("template/base/generated").exists());
        assert!(!workspace.path("missing").exists());
    }
}

#[test]
fn invalid_source_date_epoch_fails_before_side_effects() {
    let workspace = Workspace::new();
    let result = Command::new(env!("CARGO_BIN_EXE_crabyard"))
        .args(["new", "App", "--yes", "--template", "missing"])
        .current_dir(&workspace.0)
        .env("SOURCE_DATE_EPOCH", "invalid")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("SOURCE_DATE_EPOCH"));
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
}

#[test]
fn tray_and_updater_generate_valid_rust_and_are_idempotent() {
    let workspace = Workspace::new();
    workspace.template();
    let result = workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "output",
        "--extras",
        "tray,updater",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&result);
    let lib_path = workspace.path("output/src-tauri/src/lib.rs");
    let source = fs::read_to_string(&lib_path).unwrap();
    syn::parse_file(&source).unwrap();
    assert!(!source.contains(".plugin(tauri::tray"));
    assert!(source.contains("tauri_plugin_updater::Builder::new().build()"));
    assert!(!source.contains("tauri_plugin_updater::init()"));
    let cargo_path = workspace.path("output/src-tauri/Cargo.toml");
    let cargo_source = fs::read_to_string(&cargo_path).unwrap();
    let cargo: toml::Value = toml::from_str(&cargo_source).unwrap();
    let features = cargo["dependencies"]["tauri"]["features"]
        .as_array()
        .unwrap();
    assert_eq!(
        features,
        &[
            toml::Value::String("custom-protocol".into()),
            toml::Value::String("tray-icon".into())
        ]
    );
    let permissions_path = workspace.path("output/src-tauri/capabilities/default.json");
    let permissions = fs::read_to_string(&permissions_path).unwrap();
    for feature in ["tray", "updater"] {
        assert_success(&workspace.cli(&["add", feature, "--project", "output"]));
    }
    assert_eq!(source, fs::read_to_string(lib_path).unwrap());
    assert_eq!(cargo_source, fs::read_to_string(cargo_path).unwrap());
    assert_eq!(permissions, fs::read_to_string(permissions_path).unwrap());
}

#[test]
fn added_features_are_saved_in_recipe_for_replay() {
    let workspace = Workspace::new();
    workspace.template();
    assert_success(&workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "output",
        "--no-git",
        "--no-install",
    ]));
    assert_success(&workspace.cli(&["add", "tray", "--project", "output", "--dry-run"]));
    let recipe_path = workspace.path("output/crabyard.recipe.toml");
    let recipe: toml::Value = toml::from_str(&fs::read_to_string(&recipe_path).unwrap()).unwrap();
    assert!(recipe["extras"].as_array().unwrap().is_empty());
    assert_success(&workspace.cli(&["add", "tray", "--project", "output"]));
    assert_success(&workspace.cli(&["add", "updater", "--project", "output"]));
    let recipe: toml::Value = toml::from_str(&fs::read_to_string(recipe_path).unwrap()).unwrap();
    assert_eq!(
        recipe["extras"].as_array().unwrap(),
        &[
            toml::Value::String("tray".into()),
            toml::Value::String("updater".into())
        ]
    );
}

#[test]
fn readiness_is_read_only_and_strict_gates_uninstalled_projects() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    let before = snapshot(&project);
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    assert_success(&result);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["summary"]["errors"], 0);
    assert!(has_finding(&report, "lock.missing"));
    assert!(has_finding(&report, "lock.rust-missing"));
    let strict = workspace.cli(&["check", "--project", "project", "--json", "--strict"]);
    assert_eq!(strict.status.code(), Some(1));
    let strict: serde_json::Value = serde_json::from_slice(&strict.stdout).unwrap();
    assert_eq!(strict["passed"], false);
    assert_eq!(snapshot(&project), before);
    let human = workspace.cli(&["check", "--project", "project"]);
    assert_success(&human);
    assert!(String::from_utf8_lossy(&human.stdout).contains("Frontend lockfile"));
}

#[test]
fn readiness_accepts_current_lockfiles_and_detects_changed_specs_for_all_package_managers() {
    for pm in ["npm", "bun", "pnpm", "yarn"] {
        let workspace = Workspace::new();
        let project = workspace.generate_project(pm);
        workspace.lock_project(pm);
        let ready = workspace.cli(&["check", "--project", "project", "--strict", "--json"]);
        assert_success(&ready);
        let ready: serde_json::Value = serde_json::from_slice(&ready.stdout).unwrap();
        assert_eq!(ready["summary"]["warnings"], 0, "{pm}: {ready}");
        assert!(has_finding(&ready, "lock.frontend"));
        let package_path = project.join("package.json");
        let mut package: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&package_path).unwrap()).unwrap();
        package["dependencies"]["react"] = serde_json::json!("^20");
        write(
            &package_path,
            &serde_json::to_string_pretty(&package).unwrap(),
        );
        let stale = workspace.cli(&["check", "--project", "project", "--json"]);
        assert_eq!(stale.status.code(), Some(1), "{pm}");
        let stale: serde_json::Value = serde_json::from_slice(&stale.stdout).unwrap();
        assert!(has_finding(&stale, "lock.stale"), "{pm}: {stale}");
    }
}

#[test]
fn readiness_aggregates_actionable_security_and_identity_failures_without_secret_values() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    workspace.lock_project("npm");
    let config_path = project.join("src-tauri/tauri.conf.json");
    let mut config: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["identifier"] = serde_json::json!("com.wrong.product");
    config["app"]["security"]["csp"] = serde_json::Value::Null;
    write(
        &config_path,
        &serde_json::to_string_pretty(&config).unwrap(),
    );
    write(
        &project.join("src-tauri/capabilities/default.json"),
        r#"{"windows":["main"],"permissions":["*"]}"#,
    );
    write(
        &project.join(".env"),
        "VITE_CLIENT_SECRET=never-print-this-value\nINTERNAL_PASSWORD=another-private-value\n",
    );
    write(
        &project.join(".gitignore"),
        ".env\n.env.*\n!.env.example\n!.env.production\n*.key\n*.p12\n",
    );
    let before = snapshot(&project);
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    assert_eq!(result.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    for id in [
        "identity.mismatch",
        "security.csp",
        "capabilities.wildcard",
        "env.exposed-secret",
        "gitignore.env",
    ] {
        assert!(has_finding(&report, id), "{id}: {report}");
    }
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!output.contains("never-print-this-value"));
    assert!(!output.contains("another-private-value"));
    assert_eq!(snapshot(&project), before);
}

#[test]
fn readiness_handles_missing_and_corrupt_projects_as_json() {
    let workspace = Workspace::new();
    let result = workspace.cli(&["check", "--project", "missing", "--json"]);
    assert_eq!(result.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(has_finding(&report, "project.missing"));
    let project = workspace.generate_project("npm");
    write(
        &project.join("package.json"),
        "{malformed: private-error-payload",
    );
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(has_finding(&report, "manifest.invalid"));
    assert!(!String::from_utf8_lossy(&result.stdout).contains("private-error-payload"));
}

#[test]
fn readiness_detects_native_only_dependency_changes() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    workspace.lock_project("npm");
    assert_success(&workspace.cli(&["add", "window-state", "--project", "project"]));
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    assert_eq!(result.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(has_finding(&report, "lock.rust-stale"));
    assert!(has_finding(&report, "lock.frontend"));
    assert!(project.join("src-tauri/Cargo.lock").is_file());
}

#[test]
fn readiness_explains_oauth_and_updater_delivery_work() {
    let workspace = Workspace::new();
    workspace.template();
    assert_success(&workspace.cli(&[
        "new",
        "Field Notes",
        "--slug",
        "field-notes",
        "--bundle-id",
        "com.crabyard.fieldnotes",
        "--auth",
        "github",
        "--ui",
        "minimal",
        "--pm",
        "npm",
        "--extras",
        "updater",
        "--template",
        "template",
        "--output",
        "project",
        "--no-git",
        "--no-install",
    ]));
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    assert_success(&result);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(has_finding(&report, "oauth.missing"));
    assert!(has_finding(&report, "updater.incomplete"));
}

#[test]
fn context_preview_and_export_preserve_secrets_and_existing_documents() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    write(
        &project.join(".env"),
        "INTERNAL_PASSWORD=private-environment-value\nGITHUB_CLIENT_ID=public-client-id\n",
    );
    write(
        &project.join("keys/signing.key"),
        "private-signing-material",
    );
    let package_path = project.join("package.json");
    let mut package: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&package_path).unwrap()).unwrap();
    package["dependencies"]["private-sdk"] =
        serde_json::json!("https://private-user:private-password@example.com/sdk.git");
    write(
        &package_path,
        &serde_json::to_string_pretty(&package).unwrap(),
    );
    let brief = "Build a local-first research notebook with tags.\nAPI_KEY=private-brief-value\nUse sk-sensitive-test-value for development.\nSee (https://user:private-url-password@example.com/api).";
    let before = snapshot(&project);
    let result = workspace.cli(&[
        "context",
        "--project",
        "project",
        "--brief",
        brief,
        "--dry-run",
    ]);
    assert_success(&result);
    let output = String::from_utf8(result.stdout).unwrap();
    assert!(output.contains("local-first research notebook with tags"));
    assert!(output.contains("AGENTS.md"));
    assert!(output.contains("npm run check"));
    for secret in [
        "private-environment-value",
        "private-signing-material",
        "private-brief-value",
        "sk-sensitive-test-value",
        "private-password",
        "private-url-password",
    ] {
        assert!(!output.contains(secret), "Leaked {secret}");
    }
    assert_eq!(snapshot(&project), before);
    let exported = workspace.cli(&[
        "context",
        "--project",
        "project",
        "--brief",
        "Build a research notebook.",
    ]);
    assert_success(&exported);
    let path = project.join("docs/AI_CONTEXT.md");
    assert!(path.is_file());
    let original = fs::read(&path).unwrap();
    let repeated = workspace.cli(&[
        "context",
        "--project",
        "project",
        "--brief",
        "Overwrite this.",
    ]);
    assert!(!repeated.status.success());
    assert_eq!(fs::read(&path).unwrap(), original);
    let escape = workspace.cli(&[
        "context",
        "--project",
        "project",
        "--brief",
        "Build notes.",
        "--output",
        "../escaped.md",
    ]);
    assert!(!escape.status.success());
    assert!(!workspace.path("escaped.md").exists());
}

#[test]
fn generation_can_export_an_ai_brief_without_serializing_its_secrets() {
    let workspace = Workspace::new();
    workspace.template();
    let brief = "Build a research notebook.\nAPI_TOKEN=brief-secret-value";
    let dry_run = workspace.cli(&[
        "new",
        "Notes",
        "--preset",
        "minimal",
        "--brief",
        brief,
        "--dry-run",
    ]);
    assert_success(&dry_run);
    let plan: serde_json::Value = serde_json::from_slice(&dry_run.stdout).unwrap();
    assert_eq!(plan["context_output"], "docs/AI_CONTEXT.md");
    assert!(!String::from_utf8_lossy(&dry_run.stdout).contains("brief-secret-value"));
    assert_success(&workspace.cli(&[
        "new",
        "Notes",
        "--preset",
        "minimal",
        "--brief",
        brief,
        "--description",
        "Public description",
        "--template",
        "template",
        "--output",
        "project",
        "--no-git",
        "--no-install",
    ]));
    let context = fs::read_to_string(workspace.path("project/docs/AI_CONTEXT.md")).unwrap();
    assert!(context.contains("Build a research notebook."));
    assert!(!context.contains("brief-secret-value"));
    let recipe = fs::read_to_string(workspace.path("project/crabyard.recipe.toml")).unwrap();
    assert!(!recipe.contains("brief-secret-value"));
    assert!(recipe.contains("Public description"));
}

#[test]
fn removed_ui_is_rejected_before_template_access() {
    let workspace = Workspace::new();
    for args in [
        vec![
            "new",
            "App",
            "--ui",
            "legacy-ui",
            "--yes",
            "--template",
            "missing",
        ],
        vec!["preview", "--ui", "legacy-ui", "--template", "missing"],
    ] {
        let result = workspace.cli(&args);
        assert!(!result.status.success());
        let error = String::from_utf8_lossy(&result.stderr);
        assert!(error.contains("Unknown UI framework"));
        assert!(error.contains("shadcn, daisyui, minimal"));
    }
    assert_eq!(fs::read_dir(&workspace.0).unwrap().count(), 0);
}

#[test]
fn switching_ui_keeps_the_replay_recipe_and_audit_consistent() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    workspace.lock_project("npm");
    let result = Command::new(env!("CARGO_BIN_EXE_crabyard"))
        .args(["update-ui", "--switch", "shadcn", "--template"])
        .arg(workspace.path("template"))
        .current_dir(&project)
        .env("PATH", "")
        .output()
        .unwrap();
    assert_success(&result);
    let recipe: toml::Value =
        toml::from_str(&fs::read_to_string(project.join("crabyard.recipe.toml")).unwrap()).unwrap();
    assert_eq!(recipe["ui"].as_str(), Some("shadcn"));
    let result = workspace.cli(&["check", "--project", "project", "--json"]);
    assert_success(&result);
    let report: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(has_finding(&report, "identity.consistent"));
}

#[test]
fn help_does_not_echo_environment_license_keys() {
    let result = Command::new(env!("CARGO_BIN_EXE_crabyard"))
        .args(["new", "--help"])
        .env("CRABYARD_LICENSE_KEY", "private-license-help-value")
        .output()
        .unwrap();
    assert_success(&result);
    assert!(!String::from_utf8_lossy(&result.stdout).contains("private-license-help-value"));
}

#[cfg(unix)]
#[test]
fn context_rejects_output_through_an_external_symlink() {
    let workspace = Workspace::new();
    let project = workspace.generate_project("npm");
    let outside = workspace.path("outside");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, project.join("export")).unwrap();
    let result = workspace.cli(&[
        "context",
        "--project",
        "project",
        "--brief",
        "Build notes.",
        "--output",
        "export/context.md",
    ]);
    assert!(!result.status.success());
    assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
}

#[test]
fn recipe_rejects_unknown_fields_and_schema_versions() {
    let workspace = Workspace::new();
    workspace.template();
    let result = workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "app",
        "--no-git",
        "--no-install",
    ]);
    assert_success(&result);
    let recipe = fs::read_to_string(workspace.path("app/crabyard.recipe.toml")).unwrap();
    for (content, expected) in [
        (
            format!("{recipe}\nlicense_key = \"secret\"\n"),
            "unknown field",
        ),
        (
            recipe.replace("schema_version = 1", "schema_version = 2"),
            "Unsupported recipe schema_version",
        ),
    ] {
        write(&workspace.path("invalid.toml"), &content);
        let result = workspace.cli(&[
            "new",
            "--recipe",
            "invalid.toml",
            "--dry-run",
            "--output",
            "new",
        ]);
        assert!(!result.status.success());
        assert!(String::from_utf8_lossy(&result.stderr).contains(expected));
        assert!(!workspace.path("new").exists());
    }
}

#[test]
fn existing_output_is_preserved_before_any_setup() {
    let workspace = Workspace::new();
    write(&workspace.path("existing/sentinel"), "keep");
    let result = workspace.cli(&[
        "new",
        "App",
        "--yes",
        "--output",
        "existing",
        "--template",
        "missing",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already exists"));
    assert_eq!(
        fs::read_to_string(workspace.path("existing/sentinel")).unwrap(),
        "keep"
    );
}

#[test]
fn invalid_module_config_fails_without_creating_output() {
    let workspace = Workspace::new();
    let template = workspace.template();
    write(&template.join("ui/minimal/module.json"), "invalid json");
    let result = workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "output",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("Failed to parse"));
    assert!(!workspace.path("output").exists());
}

#[test]
fn failed_generation_removes_partial_output() {
    let workspace = Workspace::new();
    let template = workspace.template();
    write(&template.join("base/package.json"), "invalid json");
    let result = workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "output",
        "--no-git",
        "--no-install",
    ]);
    assert!(!result.status.success());
    assert!(!workspace.path("output").exists());
}

#[test]
fn headless_setup_failure_returns_nonzero_and_preserves_scaffold() {
    let workspace = Workspace::new();
    workspace.template();
    let result = workspace.cli(&[
        "new",
        "App",
        "--preset",
        "minimal",
        "--template",
        "template",
        "--output",
        "output",
        "--pm",
        "npm",
        "--no-git",
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("The scaffold is preserved"));
    assert!(workspace.path("output/crabyard.recipe.toml").is_file());
}
