use std::env;
use std::hash::Hash;
use std::path;
use std::fs;
use std::fmt;
use std::collections::HashMap;
use serde::Deserialize;
use toml;
use anyhow::{Context, Result};
use anyhow;

#[derive(Debug, Deserialize)]
struct ConfigRaw {
    source: ConfigPath,
    target: ConfigPath,
    retention: HashMap<String, u32>,
}

#[derive(Debug, Deserialize)]
pub struct Config {
    pub source: ConfigPath,
    pub target: ConfigPath,
    pub retention: HashMap<ConfigRetentionKind, u32>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigPath {
    pub path: path::PathBuf,
}

#[derive(PartialEq, Eq, Hash, Debug, Deserialize)]
pub enum ConfigRetentionKind {
    Hours,
    Days,
    Weeks,
    Months,
    Years,
}


impl fmt::Display for ConfigRetentionKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigRetentionKind::Hours => write!(f, "hours"),
            ConfigRetentionKind::Days => write!(f, "days"),
            ConfigRetentionKind::Weeks => write!(f, "weeks"),
            ConfigRetentionKind::Months => write!(f, "months"),
            ConfigRetentionKind::Years => write!(f, "years"),
        }
    }
}

/*
    Read config from disk
*/

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

/*
    User input validation
*/

// A valid `source` can be any file or directory
fn validate_config_source(source: &ConfigPath) -> Result<()> {
    if !source.path.exists() {
        anyhow::bail!("source path does not exist");
    }

    Ok(())
}

// A valid `target` is only a directory
fn validate_config_target(target: &ConfigPath) -> Result<()> {
    // Path doesn't already exist, but we can try to create it ourselves
    if !target.path.exists() {
        fs::create_dir_all(&target.path)
            .context("failed to create target directory")?;
    }

    if target.path.exists() && !target.path.is_dir() {
        anyhow::bail!("target path is a file, not a directory");
    }

    Ok(())
}

// A valid `retention` has at least one non-None field
fn validate_config_retention(retention: &HashMap<String, u32>) -> Result<()> {
    if retention.is_empty() {
        anyhow::bail!("no retention period was specified");
    }

    Ok(())
}

/*
    Data type conversion
*/

fn convert_config_retention(retention: &HashMap<String, u32>) -> HashMap<ConfigRetentionKind, u32> {
    let mut validated_retention = HashMap::new();

    for (period, value) in retention {
        match period.as_ref() {
            "hours" => {validated_retention.insert(ConfigRetentionKind::Hours, *value);},
            "days" => {validated_retention.insert(ConfigRetentionKind::Days, *value);},
            "weeks" => {validated_retention.insert(ConfigRetentionKind::Weeks, *value);},
            "months" => {validated_retention.insert(ConfigRetentionKind::Months, *value);},
            "years" => {validated_retention.insert(ConfigRetentionKind::Years, *value);},
            &_ => (),
        }
    }

    validated_retention
}

pub fn parse_config() -> Result<Config> {
    // Read configuration file as string
    let config_file_path = get_config_file_path();
    let config_file_str = fs::read_to_string(&config_file_path)
        .with_context(|| format!("failed to read config file: {}", config_file_path.display()))?;

    // Parse the toml into a struct
    let raw_config: ConfigRaw = toml::from_str(&config_file_str)
        .with_context(|| format!("failed to parse config file: {}", config_file_path.display()))?;

    // Panic if we have any invalid input
    validate_config_source(&raw_config.source)
        .context("failed to validate source")?;
    validate_config_target(&raw_config.target)
        .context("failed to validate target")?;
    validate_config_retention(&raw_config.retention)
        .context("failed to validate retention")?;

    Ok(Config {
        source: raw_config.source,
        target: raw_config.target,
        retention: convert_config_retention(&raw_config.retention),
    })
}
