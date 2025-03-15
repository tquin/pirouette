use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::configuration::Config;
mod configuration;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;
    println!("config {:#?}", config);

    check_target_state(&config)?;

    Ok(())
}

fn check_target_state(config: &Config) -> Result<()> {
    for (retention_period, _retention_value) in &config.retention {
        
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        // Path doesn't already exist, but we can try to create it ourselves
        if !retention_path.exists() {
            fs::create_dir_all(&retention_path)
                .with_context(|| format!("failed to create directory {}", retention_path.display()))?;
        }

        let latest_file = get_latest_created_file(&retention_path);
        match latest_file {
            // todo: something in dir, check if it's old enough to rotate
            Some(file) => println!("{} contains a file {:#?}", retention_path.display(), file),
            // todo: nothing in dir, always needs to rotate
            None => println!("{} has no files", retention_path.display()),
        }
    }

    Ok(())
}

fn get_latest_created_file(directory: &PathBuf) -> Option<fs::DirEntry> {
    let dir_entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return None, 
    };

    let dir_files = dir_entries.flatten().filter(|item| 
        match item.metadata() {
            Ok(item_metadata) => item_metadata.is_file(),
            Err(_) => false,
        }
    );

    let latest_file = dir_files.max_by_key(|item|
        match item.metadata() {
            Ok(item_metadata) => match item_metadata.created() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    );

    latest_file
}
