use std::env;
use std::path;
use std::fs;
use serde::Deserialize;
use toml;

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct PirouetteConfig {
    source: ConfigSource,
    target: ConfigTarget,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct ConfigSource {
    path: String,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct ConfigTarget {
    path: String,
}

// TODO: allow an environment variable to overwrite this default location
fn get_config_file_path() -> path::PathBuf {
    let current_directory = env::current_dir()
        .expect("failed to read current directory");

    let mut config_file_path = current_directory;
    config_file_path.push("pirouette.toml");

    return config_file_path;
}

// TODO: validate the file paths we get exist on the system (or create them, for target)
pub fn parse_config() -> PirouetteConfig {
    let config_file_str = fs::read_to_string(get_config_file_path())
        .expect("failed to read configuration file contents");
    // println!("{}", config_file_str);

    let config_file_toml: PirouetteConfig = toml::from_str(&config_file_str)
        .expect("Failed to deserialize config file");

    return config_file_toml;
}