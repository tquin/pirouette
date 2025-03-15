use anyhow::{Context, Result};
use std::fs;
use std::time;
use std::path::PathBuf;

use configuration::ConfigRetentionKind;
use configuration::Config;
mod configuration;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;
    println!("config {:#?}", config);

    let rotation_targets = get_rotation_targets(&config)?;
    println!("rotation_targets {:#?}", rotation_targets);

    Ok(())
}

fn get_rotation_targets(config: &Config) -> Result<Vec<&ConfigRetentionKind>> {
    let mut rotation_targets = vec![];

    for (retention_period, _retention_value) in &config.retention {
        
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        // Path doesn't already exist, but we can try to create it ourselves
        if !retention_path.exists() {
            fs::create_dir_all(&retention_path)
                .with_context(|| format!("failed to create directory {}", retention_path.display()))?;
        }

        let newest_file = get_newest_file(&retention_path);
        match newest_file {
            // If there's existing files, check if they're old enough to need rotation
            Some(file) => match has_target_file_aged_out(&retention_period, &file)? {
                true => rotation_targets.push(retention_period),
                false => (),
            },

            // If the directory contains no files, we always need to rotate
            None => rotation_targets.push(retention_period),
        }
    }

    Ok(rotation_targets)
}

fn get_newest_file(directory: &PathBuf) -> Option<fs::DirEntry> {
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

    let newest_file = dir_files.max_by_key(|item|
        match item.metadata() {
            Ok(item_metadata) => match item_metadata.created() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    );

    newest_file
}

fn has_target_file_aged_out(retention_kind: &ConfigRetentionKind, file: &fs::DirEntry) -> Result<bool> {
    let file_metadata = match file.metadata() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for file {:?}: {}", file, e)),
        Ok(file_metadata) => file_metadata,
    };
    let file_time = match file_metadata.created() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for file {:?}: {}", file, e)),
        Ok(file_time) => file_time,
    };

    let file_age = time::SystemTime::now().duration_since(file_time)
        .context("Failed to calculate file age")?;

    let age_threshold = match retention_kind {
        ConfigRetentionKind::Hours => 60 * 60,
        ConfigRetentionKind::Days => 24 * 60 * 60,
        ConfigRetentionKind::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionKind::Months => 30 * 24 * 60 * 60,
        ConfigRetentionKind::Years => 365 * 24 * 60 * 60,
    };

    if file_age.as_secs() >= age_threshold {
        return Ok(true);
    } else {
        return Ok(false);
    }
}
