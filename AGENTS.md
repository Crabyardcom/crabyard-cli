# Crabyard CLI

This repository contains the Rust CLI and terminal wizard. Templates are maintained independently at https://github.com/Crabyardcom/crabyard-templates.

- Read the command handler and its existing tests before editing.
- Keep noninteractive generation deterministic. `--dry-run` must not download, install, prompt, or write files.
- Validate recipes and selections before host setup or output creation. Never serialize license keys into project files.
- Route operating-system commands through the existing hooks. Preserve generated projects when optional setup fails.
- Test invalid input, existing output protection, and actual generated files when changing generation.
- Run `cargo fmt --check`, `cargo test --locked`, and `cargo clippy --all-targets --locked`.
- Default to no code comments. Explain only nonobvious constraints in at most two lines.
