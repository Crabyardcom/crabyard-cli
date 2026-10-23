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
            ("base/package.json", r#"{"name":"{{APP_SLUG}}","version":"{{APP_VERSION}}","description":"{{APP_DESCRIPTION}}","dependencies":{}}"#),
            ("base/manifest.toml", "[project]\napp_name = \"{{APP_NAME}}\"\nauthor = \"{{APP_AUTHOR}}\"\n"),
            ("base/src-tauri/Cargo.toml", "[package]\nname = \"{{APP_SLUG}}\"\nversion = \"{{APP_VERSION}}\"\n[dependencies]\ntauri = { version = \"2\", features = [\"custom-protocol\"] }\n"),
            ("base/src-tauri/tauri.conf.json", r#"{"productName":"{{APP_NAME}}","identifier":"{{APP_BUNDLE_ID}}"}"#),
            ("base/src-tauri/src/lib.rs", "fn main() { tauri::Builder::default().setup(|app| { Ok(()) }); }\n"),
            ("base/src-tauri/capabilities/default.json", r#"{"permissions":["core:default"]}"#),
            ("base/index.html", "<title>{{APP_NAME}}</title>\n"),
            ("base/src/title.ts", "const title = \"{{APP_NAME}}\";\n"),
            ("auth/none/.env.example", "VITE_APP_NAME={{APP_NAME}}\n"),
            ("ui/minimal/module.json", "{}"),
            ("ui/shadcn/module.json", "{}"),
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
