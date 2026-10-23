# Independent repositories

| Repository | Responsibility |
| --- | --- |
| [crabyard-cli](https://github.com/Crabyardcom/crabyard-cli) | Rust CLI/TUI, releases, recipes, presets |
| [crabyard-templates](https://github.com/Crabyardcom/crabyard-templates) | Base application and auth/UI overlays |
| [crabyard-api](https://github.com/Crabyardcom/crabyard-api) | License validation, template distribution, release proxy |
| [crabyard-web](https://github.com/Crabyardcom/crabyard-web) | Marketing site and installers |
| [crabyard-docs](https://github.com/Crabyardcom/crabyard-docs) | Product documentation |

Each repository has its own root, history, CI, and releases. There is no shared package workspace or parent build context.

## Railway

Project: [Crabyard](https://railway.com/project/c1c722cf-c99d-4757-88c0-d86fd94e11b1).

The `crabyard-api`, `crabyard-web`, and `crabyard-docs` services use their corresponding GitHub repository on `main`, with root directory `/`. Postgres remains the API database.

Production endpoints:

- https://api.crabyard.com
- https://crabyard.com
- https://docs.crabyard.com

The apex and `www.crabyard.com` route to the web service; `api.crabyard.com` and `docs.crabyard.com` route to their matching services. Railway-managed registration and DNS records are verified. HTTPS certificate issuance is currently blocked by Let's Encrypt `RATE_LIMIT` with `retryable: false`; there is no confirmed reset time. Keep these hosted endpoints available until the custom domains serve healthy HTTPS:

- Web: https://crabyard-web-production.up.railway.app
- API: https://crabyard-api-production.up.railway.app
- Docs: https://crabyard-docs-production.up.railway.app

During this TLS block, use an explicit trusted endpoint for licensed CLI requests, such as `CRABYARD_API_URL=https://crabyard-api-production.up.railway.app`. The value must be an HTTPS origin without credentials, a path, a query, or a fragment. Installers provide their selected API endpoint during the run. Keep the publishing workflow's `CRABYARD_API_URL` variable on the healthy hosted API until the canonical API domain is ready. Never repeatedly detach or recreate domains to bypass certificate limits.

Deploy from a service repository with explicit project, environment, and service selectors. Check the latest deployment reaches `SUCCESS` before reporting a release deployed. Database migrations run before API startup through Railway's pre-deploy command.

## CLI releases

1. Update `Cargo.toml` and `Cargo.lock`; run the CLI checks.
2. Confirm the supported `TEMPLATE_VERSION` in `src/license.rs` is published through `crabyard-templates`.
3. Tag the CLI commit `vX.Y.Z` and push the tag.
4. The release workflow tests and builds five targets, publishes each archive with its `.sha256` sidecar, and creates `SHA256SUMS`.
5. The API proxies public release assets from `Crabyardcom/crabyard-cli`. Installers reject checksum mismatches.

The tag must match `Cargo.toml`. CI uses a narrowly scoped `GITHUB_TOKEN` with `contents: write` for release creation, as described in [GitHub's permissions reference](https://docs.github.com/en/actions/reference/workflows-and-actions/workflow-syntax).

CLI and template versions are independent. CLI 1.8.0 downloads template 1.8.0. CLI patches keep the tested template version; they do not require a duplicate template release. Change `TEMPLATE_VERSION` only when adopting a compatible, published template. Cache directories and licensed download requests use that supported template version.

To retry packaging an existing tag without moving it, dispatch the release workflow from `main` with its `tag` input. The workflow checks out that immutable tag and validates its Cargo version before publishing.

## Readiness and AI context

`crabyard check --project DIR --json` returns schema version 1, a `passed` flag, counts, and findings with stable `id`, `severity`, `path`, `message`, and `action` fields. Exit code 0 passes; exit code 1 means configuration errors, or warnings when `--strict` is enabled. It performs no installs, network requests, project-script execution, or writes. Lock checks compare direct declarations and project identity; frozen installs and actual tests remain CI responsibilities.

`crabyard context --brief TEXT` writes `docs/AI_CONTEXT.md` using exclusive creation. Existing files are never replaced. `--dry-run` prints the context only, and `--output` selects a new Markdown path inside the project. `new --brief TEXT` uses the same exporter before the initial Git commit. Product briefs are not serialized into recipes or dry-run JSON plans.
