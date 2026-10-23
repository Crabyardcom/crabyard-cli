# Independent repositories

| Repository | Responsibility |
| --- | --- |
| [crabyard-cli](https://github.com/Crabyardcom/crabyard-cli) | Rust CLI/TUI, releases, recipes, presets |
| [crabyard-templates](https://github.com/Crabyardcom/crabyard-templates) | Base application and auth/UI overlays |
| [crabyard-api](https://github.com/Crabyardcom/crabyard-api) | License validation, template distribution, release proxy |
| [crabyard-web](https://github.com/Crabyardcom/crabyard-web) | Marketing site and installers |
| [crabyard-docs](https://github.com/Crabyardcom/crabyard-docs) | Product documentation |
| [tesign](https://github.com/Crabyardcom/tesign) | Independent component library |

Each repository has its own root, history, CI, and releases. There is no shared package workspace or parent build context.

## Railway

Project: [Crabyard](https://railway.com/project/c1c722cf-c99d-4757-88c0-d86fd94e11b1).

The `crabyard-api`, `crabyard-web`, and `crabyard-docs` services use their corresponding GitHub repository on `main`, with root directory `/`. Postgres remains the API database.

Production endpoints:

- https://crabyard-api-production.up.railway.app
- https://crabyard-web-production.up.railway.app
- https://crabyard-docs-production.up.railway.app

Deploy from a service repository with explicit project, environment, and service selectors. Check the latest deployment reaches `SUCCESS` before reporting a release deployed. Database migrations run before API startup through Railway's pre-deploy command.

## CLI releases

1. Update `Cargo.toml` and `Cargo.lock`; run the CLI checks.
2. Publish a compatible template version through `crabyard-templates`.
3. Tag the CLI commit `vX.Y.Z` and push the tag.
4. The release workflow tests and builds five targets, publishes each archive with its `.sha256` sidecar, and creates `SHA256SUMS`.
5. The API proxies public release assets from `Crabyardcom/crabyard-cli`. Installers reject checksum mismatches.

The tag must match `Cargo.toml`. CI uses a narrowly scoped `GITHUB_TOKEN` with `contents: write` for release creation, as described in [GitHub's permissions reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax).
