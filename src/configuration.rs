use std::env;
use std::path;
use std::fs;
use serde::Deserialize;
use toml;
use anyhow::{Context, Result};
use anyhow;

#[derive(Debug, Deserialize)]
pub struct PirouetteConfigRaw {
    pub source: ConfigPath,
    pub target: ConfigPath,
    pub retention: ConfigRetentionRaw,
}

#[derive(Debug, Deserialize)]
pub struct PirouetteConfig {
    pub source: ConfigPath,
    pub target: ConfigPath,
    pub retention: Vec<ConfigRetention>,
}

#[derive(Debug, Deserialize)]
pub struct ConfigPath {
    pub path: path::PathBuf,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct ConfigRetentionRaw {
    hours: Option<u32>,
    days: Option<u32>,
    weeks: Option<u32>,
    months: Option<u32>,
    years: Option<u32>,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct ConfigRetention {
    kind: ConfigRetentionKind,
    value: u32,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub enum ConfigRetentionKind {
    Hours,
    Days,
    Weeks,
    Months,
    Years,
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
    // Bad input: source doesn't exist
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

    // Bad input: path exists, but it's a file, not a dir
    if target.path.exists() && !target.path.is_dir() {
        anyhow::bail!("target path is a file, not a directory");
    }

    Ok(())
}

impl ConfigRetentionRaw {
    fn retention_field_list(&self) -> Vec<Option<u32>> {
        return vec![self.hours, self.days, self.weeks, self.months, self.years];
    }

    fn at_least_one_populated_field(&self) -> bool {
        self.retention_field_list().into_iter()
            .any(|field| field.is_some())
    }
}

// A valid `retention` has at least one non-None field
fn validate_config_retention(retention: &ConfigRetentionRaw) -> Result<()> {
    match &retention.at_least_one_populated_field() {
        true => (),
        // Bad input: no retention values provided
        false => anyhow::bail!("no retention period was specified"),
    }

    Ok(())
}

fn check_user_input(raw_config: &PirouetteConfigRaw) -> Result<()> {
    validate_config_source(&raw_config.source)
        .context("failed to validate source")?;
    validate_config_target(&raw_config.target)
        .context("failed to validate target")?;
    validate_config_retention(&raw_config.retention)
        .context("failed to validate retention")?;

    Ok(())
}

/*
    Data conversion
*/

fn convert_retention_type(raw_retention: &ConfigRetentionRaw) -> Result<Vec<ConfigRetention>> {
    let mut converted_retention = vec![];

    if raw_retention.hours.is_some() {
        converted_retention.push(
            ConfigRetention {
                kind: ConfigRetentionKind::Hours,
                value: raw_retention.hours.unwrap(),
            }
        )
    }

    if raw_retention.days.is_some() {
        converted_retention.push(
            ConfigRetention {
                kind: ConfigRetentionKind::Days,
                value: raw_retention.days.unwrap(),
            }
        )
    }

    if raw_retention.weeks.is_some() {
        converted_retention.push(
            ConfigRetention {
                kind: ConfigRetentionKind::Weeks,
                value: raw_retention.weeks.unwrap(),
            }
        )
    }

    if raw_retention.months.is_some() {
        converted_retention.push(
            ConfigRetention {
                kind: ConfigRetentionKind::Months,
                value: raw_retention.months.unwrap(),
            }
        )
    }

    if raw_retention.years.is_some() {
        converted_retention.push(
            ConfigRetention {
                kind: ConfigRetentionKind::Years,
                value: raw_retention.years.unwrap(),
            }
        )
    }

    Ok(converted_retention)
}

pub fn parse_config() -> Result<PirouetteConfig> {
    // Read configuration file as string
    let config_file_path = get_config_file_path();
    let config_file_str = fs::read_to_string(&config_file_path)
        .with_context(|| format!("failed to read config file: {}", config_file_path.display()))?;

    // Parse the toml into a struct
    let raw_config: PirouetteConfigRaw = toml::from_str(&config_file_str)
        .with_context(|| format!("failed to parse config file: {}", config_file_path.display()))?;

    // Panic if we have any invalid input
    check_user_input(&raw_config)?;

    // Convert raw format into type-compliant data
    Ok(PirouetteConfig {
        source: raw_config.source,
        target: raw_config.target,
        retention: convert_retention_type(&raw_config.retention)?,
    })
}
