use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::generate::Config;
use crate::tokens;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Preset {
    Minimal,
    Desktop,
    LocalFirst,
}

impl Preset {
    pub fn defaults(self) -> (&'static str, &'static str, &'static [&'static str]) {
        match self {
            Self::Minimal => ("none", "minimal", &[]),
            Self::Desktop => ("none", "shadcn", &["log", "window-state", "notifications"]),
            Self::LocalFirst => ("none", "shadcn", &["sql", "log", "window-state", "cmdk"]),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub schema_version: u32,
    pub app_name: String,
    pub slug: String,
    pub bundle_id: String,
    pub app_version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    pub auth: String,
    pub ui: String,
    pub pm: String,
    #[serde(default)]
    pub extras: Vec<String>,
}

impl Recipe {
    pub fn read(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Cannot read recipe {}", path.display()))?;
        let recipe: Self = toml::from_str(&content)
            .with_context(|| format!("Invalid recipe {}", path.display()))?;
        recipe.validate()?;
        Ok(recipe)
    }

    pub fn apply(self, config: &mut Config) {
        config.app_name = config.app_name.take().or(Some(self.app_name));
        config.slug = config.slug.take().or(Some(self.slug));
        config.bundle_id = config.bundle_id.take().or(Some(self.bundle_id));
        config.app_version = config.app_version.take().or(Some(self.app_version));
        config.author = config.author.take().or(Some(self.author));
        config.description = config.description.take().or(Some(self.description));
        config.auth = config.auth.take().or(Some(self.auth));
        config.ui = config.ui.take().or(Some(self.ui));
        config.pm = config.pm.take().or(Some(self.pm));
        if config.extras.is_empty() {
            config.extras = self.extras;
        }
    }

    pub fn resolve(config: &Config) -> Self {
        let app_name = config.app_name.clone().unwrap_or_else(|| "My App".into());
        let slug = config
            .slug
            .clone()
            .unwrap_or_else(|| tokens::to_slug(&app_name));
        Self {
            schema_version: 1,
            app_name,
            bundle_id: config
                .bundle_id
                .clone()
                .unwrap_or_else(|| tokens::to_bundle_id(&slug)),
            slug,
            app_version: config.app_version.clone().unwrap_or_else(|| "0.1.0".into()),
            author: config.author.clone().unwrap_or_default(),
            description: config.description.clone().unwrap_or_default(),
            auth: config.auth.clone().unwrap_or_else(|| "none".into()),
            ui: config.ui.clone().unwrap_or_else(|| "shadcn".into()),
            pm: config.pm.clone().unwrap_or_else(|| "bun".into()),
            extras: config.extras.clone(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            bail!(
                "Unsupported recipe schema_version {}; expected 1",
                self.schema_version
            );
        }
        if self.app_name.trim().is_empty() {
            bail!("App name must not be empty");
        }
        for (name, value) in [
            ("App name", &self.app_name),
            ("Author", &self.author),
            ("Description", &self.description),
        ] {
            if value.chars().any(char::is_control) || value.contains("{{") || value.contains("}}") {
                bail!("{name} must not contain control characters or template tokens");
            }
        }
        if self.slug.len() > 64
            || !self
                .slug
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_lowercase)
            || self.slug.ends_with('-')
            || self.slug.contains("--")
            || !self
                .slug
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
        {
            bail!("Slug must start with a lowercase ASCII letter and contain up to 64 lowercase letters, digits, or single hyphens");
        }
        let bundle_parts: Vec<_> = self.bundle_id.split('.').collect();
        if bundle_parts.len() < 2
            || bundle_parts.iter().any(|part| {
                part.is_empty()
                    || !part.as_bytes()[0].is_ascii_alphabetic()
                    || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            })
        {
            bail!("Bundle ID must use reverse DNS notation, for example com.example.myapp");
        }
        semver::Version::parse(&self.app_version)
            .context("App version must be valid semantic versioning, for example 0.1.0")?;
        validate_choice("Auth module", &self.auth, &["github", "google", "none"])?;
        validate_choice(
            "UI framework",
            &self.ui,
            &["shadcn", "daisyui", "tesign", "minimal"],
        )?;
        validate_choice("Package manager", &self.pm, &["bun", "pnpm", "yarn", "npm"])?;
        for extra in &self.extras {
            crate::add::validate_feature(extra)?;
        }
        Ok(())
    }

    pub fn write(&self, output: &Path) -> Result<()> {
        fs::write(
            output.join("crabyard.recipe.toml"),
            toml::to_string_pretty(self)?,
        )
        .context("Failed to write crabyard.recipe.toml")
    }
}

fn validate_choice(label: &str, value: &str, choices: &[&str]) -> Result<()> {
    if !choices.contains(&value) {
        bail!(
            "Unknown {label} '{value}'. Choose one of: {}",
            choices.join(", ")
        );
    }
    Ok(())
}

pub fn list(json: bool) -> Result<()> {
    let presets = [
        ("minimal", "Lean desktop app with Tailwind", Preset::Minimal),
        (
            "desktop",
            "Desktop utilities with logging, window persistence, and notifications",
            Preset::Desktop,
        ),
        (
            "local-first",
            "SQLite workspace with logging, window persistence, and a command palette",
            Preset::LocalFirst,
        ),
    ];
    if json {
        let values: Vec<_> = presets.iter().map(|(name, description, preset)| {
            let (auth, ui, extras) = preset.defaults();
            serde_json::json!({"name": name, "description": description, "auth": auth, "ui": ui, "extras": extras})
        }).collect();
        println!("{}", serde_json::to_string_pretty(&values)?);
    } else {
        for (name, description, _) in presets {
            println!("{name:12} {description}");
        }
        println!("\nUsage: crabyard new \"My App\" --preset local-first --yes");
    }
    Ok(())
}
