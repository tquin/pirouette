use anyhow::{Context, Result};
use std::fs;
use std::fs::DirEntry;
use std::time;
use std::path::PathBuf;

use crate::configuration::ConfigRetentionKind;
use crate::configuration::Config;

pub fn get_rotation_targets(config: &Config) -> Result<Vec<&ConfigRetentionKind>> {
    let mut rotation_targets = vec![];

    for retention_period in config.retention.keys() {
        
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        // Path doesn't already exist, but we can try to create it ourselves
        if !retention_path.exists() {
            fs::create_dir_all(&retention_path)
                .with_context(|| format!("failed to create directory {}", retention_path.display()))?;
        }

        let newest_entry = get_newest_directory_entry(&retention_path);
        match newest_entry {
            // If there's existing snapshots, check if they're old enough to need rotation
            Some(snapshot) => if has_target_snapshot_aged_out(retention_period, &snapshot)? {
                rotation_targets.push(retention_period);
            },

            // If there's no previous snapshots, we always need to rotate
            None => rotation_targets.push(retention_period),
        }
    }

    Ok(rotation_targets)
}

fn get_newest_directory_entry(directory: &PathBuf) -> Option<DirEntry> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    let readable_entries = entries.filter_map(|entry|
        match entry {
            Ok(entry) => Some(entry),
            Err(_) => None,
        }
    );

    // Return the newest item in the directory
    readable_entries.max_by_key(|entry|
        match entry.metadata() {
            Ok(entry_metadata) => match entry_metadata.created() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    )
}

fn has_target_snapshot_aged_out(retention_kind: &ConfigRetentionKind, snapshot: &DirEntry) -> Result<bool> {
    let snapshot_metadata = match snapshot.metadata() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for snapshot {:?}: {}", snapshot, e)),
        Ok(snapshot_metadata) => snapshot_metadata,
    };
    let snapshot_time = match snapshot_metadata.created() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for snapshot {:?}: {}", snapshot, e)),
        Ok(snapshot_time) => snapshot_time,
    };

    let snapshot_age = time::SystemTime::now().duration_since(snapshot_time)
        .context("Failed to calculate snapshot age")?;

    let age_threshold = match retention_kind {
        ConfigRetentionKind::Hours => 60 * 60,
        ConfigRetentionKind::Days => 24 * 60 * 60,
        ConfigRetentionKind::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionKind::Months => 30 * 24 * 60 * 60,
        ConfigRetentionKind::Years => 365 * 24 * 60 * 60,
    };

    let result: bool = snapshot_age.as_secs() >= age_threshold;
    Ok(result)
}
