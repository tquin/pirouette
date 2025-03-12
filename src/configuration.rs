use std::env;
use std::path;
use std::fs;
use serde::Deserialize;
use toml;
use anyhow::{Context, Result};
use anyhow;

#[derive(Debug, Deserialize)]
pub struct PirouetteConfig {
    source: ConfigPath,
    target: ConfigPath,
    retention: ConfigRetention,
}

#[derive(Debug, Deserialize)]
pub struct ConfigPath {
    path: path::PathBuf,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct ConfigRetention {
    hours: Option<u32>,
    days: Option<u32>,
    weeks: Option<u32>,
    months: Option<u32>,
    years: Option<u32>,
}

fn get_config_file_path() -> path::PathBuf {
    let config_file_path = match env::var("PIROUETTE_CONFIG_FILE") {
        // Read from envvar path if provided
        Ok(config_file_path) => path::PathBuf::from(config_file_path),
        // Read from default path if envvar is unset
        Err(_) => get_config_file_path_default(),
    };

    return config_file_path;
}

fn get_config_file_path_default() -> path::PathBuf {
    let current_directory = env::current_dir()
        .expect("Failed to read current directory");

    let mut config_file_path = current_directory;
    config_file_path.push("pirouette.toml");
    
    return config_file_path;
}

fn validate_config_source(source: &ConfigPath) -> Result<()> {
    // A valid `source` can be any file or directory

    // Bad input: source doesn't exist
    if !source.path.exists() {
        anyhow::bail!("source path does not exist");
    }

    Ok(())
}

fn validate_config_target(target: &ConfigPath) -> Result<()> {
    // A valid `target` is only a directory

    // Path doesn't already exist, but we can create it ourselves
    if !target.path.exists() {
        fs::create_dir_all(&target.path)
            .context("failed to create target directory")?;
    }

    // Bad input: path exists, but it's a file, not a dir
    if target.path.exists() && !target.path.is_dir() {
        anyhow::bail!("target path is a file, not a directory");
    }

    Ok(())
}

impl ConfigRetention {
    fn at_least_one_populated_field(&self) -> bool {
        let fields = [&self.hours, &self.days, &self.weeks, &self.months, &self.years];
        fields.iter().any(|&field| field.is_some())
    }
}

#[allow(unused)]
fn validate_config_retention(retention: &ConfigRetention) -> Result<()> {
    // A valid `retention` has at least one non-None field
    match &retention.at_least_one_populated_field() {
        true => (),
        false => anyhow::bail!("no retention period was specified"),
    }

    Ok(())
}

fn validate_config(config_file_toml: &PirouetteConfig) -> Result<()> {
    // These may panic on bad user input
    validate_config_source(&config_file_toml.source)
        .context("failed to validate source")?;
    validate_config_target(&config_file_toml.target)
        .context("failed to validate target")?;
    validate_config_retention(&config_file_toml.retention)
        .context("failed to validate retention")?;

    Ok(())
}

pub fn parse_config() -> Result<PirouetteConfig> {
    // Read configuration file as string
    let config_file_path = get_config_file_path();
    let config_file_str = fs::read_to_string(&config_file_path)
        .with_context(|| format!("failed to read config file: {}", config_file_path.display()))?;

    // Parse the toml into a struct
    let config_file_toml: PirouetteConfig = toml::from_str(&config_file_str)
        .with_context(|| format!("failed to parse config file: {}", config_file_path.display()))?;

    // Validate the format, create paths if required, etc.
    validate_config(&config_file_toml)?;
    Ok(config_file_toml)
}
