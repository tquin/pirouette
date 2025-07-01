use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::PirouetteDirEntry;
use crate::configuration::Config;

pub fn clean_snapshots(config: &Config) -> Result<()> {
    for (retention_period, retention_value) in &config.retention {
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        let entries =
            fs::read_dir(&retention_path).context("Failed to read snapshot directory contents")?;

        let readable_entries: Vec<PirouetteDirEntry> = entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.into()),
                Err(_) => None,
            })
            .collect();
        let current_snapshot_count = readable_entries.len();

        // Are there more snapshots than the user wants?
        if current_snapshot_count < *retention_value {
            return Ok(());
        }

        // If so, we need to delete the excess
        let expired_snapshot_count = current_snapshot_count - *retention_value;

        if let Ok(expired_snapshots) =
            get_expired_snapshots(readable_entries, expired_snapshot_count)
        {
            delete_snapshots(&expired_snapshots);
        }
    }

    Ok(())
}

fn get_expired_snapshots(entries: Vec<PirouetteDirEntry>, count: usize) -> Result<Vec<PathBuf>> {
    // Sort the snapshots from oldest -> newest
    let mut sorted_entries = entries;
    sorted_entries.sort_by_key(|entry| entry.created);

    let (expired_snapshots, _) = sorted_entries
        .split_at_checked(count)
        .context("Failed to calculate expired snapshots")?;

    let mut result = vec![];
    for entry in expired_snapshots {
        result.push(entry.path.clone());
    }

    Ok(result)
}

fn delete_snapshots(expired_snapshots: &[PathBuf]) {
    for snapshot in expired_snapshots {
        if snapshot.is_dir() {
            if let Err(err) = fs::remove_dir_all(snapshot) {
                println!("{err}");
            }
        }
        if snapshot.is_file() {
            if let Err(err) = fs::remove_file(snapshot) {
                println!("{err}");
            }
        }
    }
}
