# Crabyard CLI

A Rust CLI and terminal wizard for generating Tauri v2 desktop applications with React, TypeScript, authentication, UI components, and coding assistant instructions.

## Generate an app

```sh
crabyard new
crabyard presets
crabyard new "My App" --preset desktop --yes
```

The wizard selects authentication (`none`, `github`, `google`), UI (`minimal`, `shadcn`, `daisyui`, `tesign`), package manager, app metadata, and optional Tauri plugins. Normal generation installs frontend dependencies and prepares the local toolchain.

Download the platform installer from [Crabyard](https://crabyard.dev), or build locally with `cargo install --path .`. Installers verify SHA256 sidecars published with CLI releases.

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
| `preview` | Inspect template file selections |
| `add` | Add optional Tauri plugins; supports `--dry-run` |
| `init` | Track an existing Tauri project |
| `upgrade` | Apply template updates with conflict checks |
| `update-ui` | Switch or roll back UI overlays |
| `plugins` | Browse plugin metadata |
| `eject` | Remove generator metadata |
| `completions` | Generate shell completions |

Run `crabyard COMMAND --help` for flags. License downloads use `CRABYARD_LICENSE_KEY`; local template source uses `CRABYARD_TEMPLATE` or `--template`.

## Development

```sh
cargo fmt --check
cargo test --locked
cargo clippy --all-targets --locked
cargo run -- new "Smoke App" --template ../crabyard-templates --preset minimal --yes --no-install --no-git
```

This is an independent repository. Other components live in the [Crabyardcom organization](https://github.com/Crabyardcom). See [release and deployment notes](docs/operations.md).
