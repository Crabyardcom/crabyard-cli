mod add;
mod config;
mod doctor;
mod eject;
mod generate;
mod hooks;
mod init;
mod license;
mod overlay;
mod plugins;
mod preview;
mod tokens;
mod tui;
mod update_ui;
mod upgrade;

use std::path::PathBuf;

use clap::{CommandFactory, Parser, Subcommand};

/// Crabyard — generate a production-ready Rust Tauri desktop app from a template.
#[derive(Parser)]
#[command(name = "crabyard", version = env!("GIT_VERSION"), about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check system prerequisites for Crabyard development
    Doctor,

    /// Create a new Crabyard project (interactive by default)
    New {
        /// App display name, e.g. "My Desktop App"
        app_name: Option<String>,

        /// App slug override (derived from app-name if omitted)
        #[arg(long, value_name = "SLUG")]
        slug: Option<String>,

        /// Bundle identifier (derived from slug if omitted)
        #[arg(long, value_name = "ID")]
        bundle_id: Option<String>,

        /// Initial version
        #[arg(long, default_value = "0.1.0")]
        app_version: Option<String>,

        /// Author name
        #[arg(long, default_value = "")]
        author: Option<String>,

        /// Short description
        #[arg(long, default_value = "")]
        description: Option<String>,

        /// Auth module: github, google, or none
        #[arg(long, value_name = "MODULE")]
        auth: Option<String>,

        /// UI framework: shadcn, daisyui, or tesign
        #[arg(long, value_name = "FRAMEWORK")]
        ui: Option<String>,

        /// Path to the template directory
        /// [env: CRABYARD_TEMPLATE]
        #[arg(long, env = "CRABYARD_TEMPLATE", value_name = "DIR")]
        template: Option<PathBuf>,

        /// Output directory (defaults to ./<app-slug>)
        #[arg(long, short, value_name = "DIR")]
        output: Option<PathBuf>,

        /// Accept all defaults without prompting
        #[arg(long, short = 'y')]
        yes: bool,

        /// Skip `git init` and initial commit
        #[arg(long)]
        no_git: bool,

        /// Skip dependency installation
        #[arg(long)]
        no_install: bool,

        /// Package manager: bun, pnpm, yarn, or npm
        #[arg(long, value_name = "PM")]
        pm: Option<String>,

        /// License key for template download
        /// [env: CRABYARD_LICENSE_KEY]
        #[arg(long, env = "CRABYARD_LICENSE_KEY", value_name = "KEY")]
        license_key: Option<String>,

        /// Extras to include (comma-separated): notifications,clipboard,sql,...
        #[arg(long, value_name = "FEATURE", value_delimiter = ',')]
        extras: Vec<String>,
    },

    /// Update or switch the UI framework in an existing project
    UpdateUi {
        /// Switch to a different UI framework: shadcn, daisyui, tesign, or minimal
        #[arg(long, value_name = "FRAMEWORK")]
        switch: Option<String>,

        /// Path to the template directory
        /// [env: CRABYARD_TEMPLATE]
        #[arg(long, env = "CRABYARD_TEMPLATE", value_name = "DIR")]
        template: Option<PathBuf>,

        /// License key for template download
        /// [env: CRABYARD_LICENSE_KEY]
        #[arg(long, env = "CRABYARD_LICENSE_KEY", value_name = "KEY")]
        license_key: Option<String>,

        /// Overwrite locally modified files without prompting
        #[arg(long)]
        force: bool,

        /// Show what would change without modifying files
        #[arg(long)]
        dry_run: bool,

        /// Rollback to the previously used UI framework
        #[arg(long)]
        rollback: bool,
    },

    /// Add a Tauri plugin or feature to an existing project
    Add {
        /// Feature to add (e.g. notifications, clipboard, sql). Use "list" to see all.
        feature: String,

        /// Path to the project directory (defaults to current directory)
        #[arg(long, short, value_name = "DIR")]
        project: Option<PathBuf>,

        /// Show what would change without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Remove Crabyard metadata, leaving a clean standalone project
    Eject {
        /// Path to the project directory (defaults to current directory)
        #[arg(long, short, value_name = "DIR")]
        project: Option<PathBuf>,

        /// Show what would change without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Preview the file tree that would be generated
    Preview {
        /// Auth module: github, google, or none
        #[arg(long, default_value = "none")]
        auth: String,

        /// UI framework: shadcn, daisyui, tesign, or minimal
        #[arg(long, default_value = "shadcn")]
        ui: String,

        /// Path to the template directory
        /// [env: CRABYARD_TEMPLATE]
        #[arg(long, env = "CRABYARD_TEMPLATE", value_name = "DIR")]
        template: Option<PathBuf>,

        /// License key for template download
        /// [env: CRABYARD_LICENSE_KEY]
        #[arg(long, env = "CRABYARD_LICENSE_KEY", value_name = "KEY")]
        license_key: Option<String>,
    },

    /// Check for and apply template updates to an existing project
    Upgrade {
        /// Path to the template directory
        /// [env: CRABYARD_TEMPLATE]
        #[arg(long, env = "CRABYARD_TEMPLATE", value_name = "DIR")]
        template: Option<PathBuf>,

        /// License key for template download
        /// [env: CRABYARD_LICENSE_KEY]
        #[arg(long, env = "CRABYARD_LICENSE_KEY", value_name = "KEY")]
        license_key: Option<String>,

        /// Overwrite locally modified files without prompting
        #[arg(long)]
        force: bool,

        /// Show what would change without modifying files
        #[arg(long)]
        dry_run: bool,
    },

    /// Initialize Crabyard in an existing Tauri project
    Init {
        /// Path to the project directory (defaults to current directory)
        #[arg(long, short, value_name = "DIR")]
        project: Option<PathBuf>,
    },

    /// Browse available Tauri plugins
    Plugins {
        /// Filter plugins by name or keyword
        filter: Option<String>,
    },

    /// Generate shell completion scripts
    Completions {
        /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Doctor => {
            doctor::run()?;
        }
        Commands::New {
            app_name,
            slug,
            bundle_id,
            app_version,
            author,
            description,
            auth,
            ui,
            template,
            output,
            yes,
            no_git,
            no_install,
            pm,
            license_key,
            extras,
        } => {
            generate::run(generate::Config {
                app_name,
                slug,
                bundle_id,
                app_version,
                author,
                description,
                auth,
                ui,
                template,
                output,
                yes,
                no_git,
                no_install,
                pm,
                license_key,
                extras,
            })?;
        }
        Commands::UpdateUi {
            switch,
            template,
            license_key,
            force,
            dry_run,
            rollback,
        } => {
            update_ui::run(update_ui::Config {
                switch,
                template,
                license_key,
                force,
                dry_run,
                rollback,
            })?;
        }
        Commands::Add {
            feature,
            project,
            dry_run,
        } => {
            add::run(add::Config {
                feature,
                project,
                dry_run,
            })?;
        }
        Commands::Eject {
            project,
            dry_run,
        } => {
            eject::run(eject::Config {
                project,
                dry_run,
            })?;
        }
        Commands::Preview {
            auth,
            ui,
            template,
            license_key,
        } => {
            preview::run(preview::Config {
                template,
                auth,
                ui,
                license_key,
            })?;
        }
        Commands::Upgrade {
            template,
            license_key,
            force,
            dry_run,
        } => {
            upgrade::run(upgrade::Config {
                template,
                license_key,
                force,
                dry_run,
            })?;
        }
        Commands::Init { project } => {
            init::run(init::Config { project })?;
        }
        Commands::Plugins { filter } => {
            plugins::run(filter.as_deref());
        }
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "crabyard", &mut std::io::stdout());
        }
    }

    Ok(())
}
