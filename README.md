# Crabyard CLI

A Rust CLI and terminal wizard for generating Tauri v2 desktop applications with React, TypeScript, authentication, UI components, and coding assistant instructions.

## Generate an app

```sh
crabyard new
crabyard presets
crabyard new "My App" --preset desktop --yes
crabyard new "Fieldnotes" --preset local-first --brief "A research notebook with tags and Markdown export"
```

The wizard selects authentication (`none`, `github`, `google`), UI (`minimal`, `shadcn`, `daisyui`), package manager, app metadata, and optional Tauri plugins. Normal generation installs frontend dependencies and prepares the local toolchain.

Download the platform installer from [Crabyard](https://crabyard.com), or build locally with `cargo install --path .`. Installers verify SHA256 sidecars published with CLI releases.

## Reproducible generation

Every generated application includes `crabyard.recipe.toml`. Reuse it or check it into version control:

```sh
crabyard new --recipe ./my-app/crabyard.recipe.toml --output ./another-app --yes
crabyard new "Local Notes" --preset local-first --dry-run
crabyard presets --json
```

Explicit flags override recipe values. Recipes reject unknown fields and do not store license keys or machine paths. Dry runs produce a JSON plan without downloads, installs, or filesystem writes.

For CI or machines that only need to generate source:

```sh
git clone https://github.com/Crabyardcom/crabyard-templates ../crabyard-templates
crabyard new "Example" --template ../crabyard-templates --auth none --ui minimal --pm npm --yes --no-install --no-git
```

This headless path does not require a terminal or install desktop build tools. Building the generated desktop app still requires platform prerequisites reported by `crabyard doctor`.

## From scaffold to your first feature

Use `new --brief` to generate `docs/AI_CONTEXT.md` alongside the app, or export a focused brief from an existing project:

```sh
crabyard context --project ./fieldnotes --brief "Add notebook tagging with keyboard navigation" --dry-run
crabyard context --project ./fieldnotes --brief "Add notebook tagging with keyboard navigation"
```

The context combines the selected stack, existing file map, instructions, an implementation slice, verification commands, and current configuration findings. It references project files without copying their bodies, environment values, credential stores, signing material, or private dependency URLs. Common credential patterns in an explicit brief are redacted. The export refuses to overwrite an existing file; choose another `--output` path for a fresh brief. Dry runs print Markdown without writing files.

## Check delivery readiness

```sh
crabyard check --project ./fieldnotes
crabyard check --project ./fieldnotes --json
crabyard check --project ./fieldnotes --strict
```

The audit checks identity across manifests, recipe choices, direct dependency lock declarations, coding-assistant instructions, product briefs, OAuth public configuration, capabilities, webview policy, updater setup, workflow syntax, and secret-file ignore rules. Findings include stable IDs, severity, a project-relative path, and an action. Normal mode exits `1` for errors; `--strict` also fails on warnings. JSON remains on stdout when a readiness gate fails.

This is a read-only configuration audit. It does not install dependencies, run project scripts, execute builds or tests, scan vulnerabilities, check whether Git already tracks a secret file, or verify remote repository secrets and code-signing certificates. Run the generated project's verification commands before distribution.

## Generated foundations

- Tauri v2 backend, React 19 frontend, typed IPC, state and settings.
- GitHub device flow or Google PKCE authentication, with OS credential storage.
- Optional plugins through `crabyard add list` and `crabyard add FEATURE`.
- Agent guidance for Codex, Claude Code, GitHub Copilot, Cursor, Gemini CLI, and Windsurf.
- Frontend type checking, linting, tests, builds, and desktop CI.
- A reusable recipe and metadata for template upgrades.

## Commands

| Command | Purpose |
| --- | --- |
| `new` | Generate an app with the wizard, flags, recipe, or preset |
| `presets` | Inspect curated feature selections; supports `--json` |
| `doctor` | Check and prepare development prerequisites |
| `check` | Audit project readiness; supports `--json` and `--strict` |
| `context` | Export a curated AI implementation brief; supports `--dry-run` |
| `preview` | Inspect template file selections |
| `add` | Add optional Tauri plugins; supports `--dry-run` |
| `init` | Track an existing Tauri project |
| `upgrade` | Apply template updates with conflict checks |
| `update-ui` | Switch or roll back UI overlays |
| `plugins` | Browse plugin metadata |
| `eject` | Remove generator metadata |
| `completions` | Generate shell completions |

Run `crabyard COMMAND --help` for flags. License downloads use `CRABYARD_LICENSE_KEY`; local template source uses `CRABYARD_TEMPLATE` or `--template`.

Licensed requests default to `https://api.crabyard.com`. An explicit `CRABYARD_API_URL` can select a trusted HTTPS origin, including the hosted Railway endpoint documented in [operations](docs/operations.md). Overrides cannot contain credentials, paths, queries, or fragments. License keys stay on that configured origin; requests do not follow redirects or silently switch servers.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked
cargo run -- new "Smoke App" --template ../crabyard-templates --preset minimal --yes --no-install --no-git
CRABYARD_TEST_TEMPLATE=../crabyard-templates cargo test --locked --test template_project -- --ignored
```

This is an independent repository. Other components live in the [Crabyardcom organization](https://github.com/Crabyardcom). See [release and deployment notes](docs/operations.md).
