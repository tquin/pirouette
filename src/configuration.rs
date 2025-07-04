use anyhow::{Context, Result};
use log::LevelFilter;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::fs;
use std::hash::Hash;
use std::path;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub source: ConfigPath,
    pub target: ConfigPath,
    pub retention: HashMap<ConfigRetentionKind, usize>,
    #[serde(default = "default_opts")]
    pub options: ConfigOpts,
}

#[derive(Debug, Deserialize)]
pub struct ConfigPath {
    pub path: path::PathBuf,
}

#[derive(Debug, Deserialize)]
pub struct ConfigOpts {
    #[serde(default = "default_opts_output_format")]
    pub output_format: ConfigOptsOutputFormat,
    #[serde(
        default = "default_opts_log_level",
        deserialize_with = "deserialize_opts_log_level"
    )]
    pub log_level: LevelFilter,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ConfigOptsOutputFormat {
    Directory,
    Tarball,
}

#[derive(PartialEq, Eq, Hash, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
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

fn default_opts() -> ConfigOpts {
    ConfigOpts {
        output_format: default_opts_output_format(),
        log_level: default_opts_log_level(),
    }
}

fn default_opts_output_format() -> ConfigOptsOutputFormat {
    ConfigOptsOutputFormat::Directory
}

fn default_opts_log_level() -> LevelFilter {
    LevelFilter::Warn
}

fn deserialize_opts_log_level<'a, D>(deserializer: D) -> Result<LevelFilter, D::Error>
where
    D: Deserializer<'a>,
{
    let s = String::deserialize(deserializer)?;
    let level = match s.to_lowercase().as_str() {
        "off" => LevelFilter::Off,
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => default_opts_log_level(),
    };
    Ok(level)
}

/*
    Read config from disk
*/

fn get_config_file_path() -> path::PathBuf {
    match env::var("PIROUETTE_CONFIG_FILE") {
        Ok(env_var) => match env_var.as_str() {
            // Read from default path if envvar is set, but empty
            "" => get_config_file_path_default(),
            // Read from envvar path if provided
            _ => path::PathBuf::from(env_var),
        },
        // Read from default path if envvar is unset
        Err(_) => get_config_file_path_default(),
    }
}

fn get_config_file_path_default() -> path::PathBuf {
    let default_directory = match in_container::in_container() {
        true => path::PathBuf::from("/config"),
        false => env::current_dir().expect("Failed to read current directory"),
    };

    let mut config_file_path = default_directory;
    config_file_path.push("pirouette.toml");

    config_file_path
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
        fs::create_dir_all(&target.path).context("failed to create target directory")?;
    }

    if target.path.exists() && !target.path.is_dir() {
        anyhow::bail!("target path is a file, not a directory");
    }

    Ok(())
}

// A valid `retention` has at least one non-None field
fn validate_config_retention(retention: &HashMap<ConfigRetentionKind, usize>) -> Result<()> {
    if retention.is_empty() {
        anyhow::bail!("no retention period was specified");
    }

    Ok(())
}

pub fn parse_config() -> Result<Config> {
    // Read configuration file as string
    let config_file_path = get_config_file_path();
    let config_file_str = fs::read_to_string(&config_file_path)
        .with_context(|| format!("failed to read config file: {config_file_path:?}"))?;

    // Parse the toml into a struct
    let config: Config = toml::from_str(&config_file_str)
        .with_context(|| format!("failed to parse config file: {config_file_path:?}"))?;

    // Panic if we have any invalid input
    validate_config_source(&config.source).context("failed to validate source")?;
    validate_config_target(&config.target).context("failed to validate target")?;
    validate_config_retention(&config.retention).context("failed to validate retention")?;

    Ok(config)
}

/*
    Unit tests
*/

#[cfg(test)]
mod tests {
    use super::*;
    use rand::Rng;

    #[test]
    fn get_config_file_from_envvar() {
        // Temporarily sets the var, reset to original state at test end
        temp_env::with_vars([("PIROUETTE_CONFIG_FILE", Some("/test/path.toml"))], || {
            let expected_path = path::PathBuf::from("/test/path.toml");
            let actual_path = get_config_file_path();
            assert_eq!(actual_path, expected_path);
        })
    }

    #[test]
    fn get_config_file_with_unset_envvar() {
        temp_env::with_vars([("PIROUETTE_CONFIG_FILE", None::<&str>)], || {
            let expected_path = get_config_file_path_default();
            let actual_path = get_config_file_path();
            assert_eq!(actual_path, expected_path);
        })
    }

    #[test]
    fn get_config_file_with_empty_envvar() {
        temp_env::with_vars([("PIROUETTE_CONFIG_FILE", Some(""))], || {
            let expected_path = get_config_file_path_default();
            let actual_path = get_config_file_path();
            assert_eq!(actual_path, expected_path);
        })
    }

    #[test]
    fn validate_source_fails_on_nonexistent_file() {
        let test_data = ConfigPath {
            path: path::PathBuf::from(""), // No such "" file
        };
        let actual_result = validate_config_source(&test_data);
        assert!(actual_result.is_err());
    }

    fn get_random_string(length: u8) -> String {
        let mut rng = rand::rng();
        let s: String = (&mut rng)
            .sample_iter(rand::distr::Alphanumeric)
            .take(length.into())
            .map(char::from)
            .collect();
        s
    }

    #[test]
    fn validate_source_succeeds_on_existing_file() -> Result<()> {
        // Create some real test file
        let mut temp_file = env::temp_dir();
        temp_file.push(format!("pirouette_{}", get_random_string(10)));
        std::fs::write(&temp_file, "foo")?;

        let test_data = ConfigPath {
            path: temp_file.clone(),
        };
        let actual_result = validate_config_source(&test_data);

        // Clean up test file afterwards
        std::fs::remove_file(temp_file)?;

        assert!(actual_result.is_ok());
        Ok(())
    }
}
