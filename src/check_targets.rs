use anyhow::{Context, Result};
use std::fs;
use std::time::SystemTime;
use std::path::PathBuf;

use crate::configuration::ConfigRetentionKind;
use crate::configuration::Config;
use crate::PirouetteDirEntry;

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

fn get_newest_directory_entry(directory: &PathBuf) -> Option<PirouetteDirEntry> {
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

    // Convert to abstracted testable type
    let typed_entries = readable_entries.map(|entry| entry.into());

    // Return the newest item in the directory
    typed_entries.max_by_key(|entry: &PirouetteDirEntry|
        entry.created
    )
}

fn has_target_snapshot_aged_out(retention_kind: &ConfigRetentionKind, snapshot: &PirouetteDirEntry) -> Result<bool> {
    let snapshot_age = SystemTime::now().duration_since(snapshot.created)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_has_target_snapshot_aged_out() {
        let test_params: Vec<(ConfigRetentionKind, u64)> = vec![
            (ConfigRetentionKind::Hours, 3600),
            (ConfigRetentionKind::Days, 86400),
            (ConfigRetentionKind::Weeks, 604800),
            (ConfigRetentionKind::Months, 2592000),
            (ConfigRetentionKind::Years, 31536000),
        ];

        for (retention_period, threshold_seconds) in test_params {

            let expired_snapshot = PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                created: SystemTime::now() - Duration::from_secs(threshold_seconds),
            };
            let expired_result = has_target_snapshot_aged_out(&retention_period, &expired_snapshot).unwrap();
            assert!(expired_result);

            let fresh_snapshot = PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                // This assumes the function will return within 1 second
                created: SystemTime::now() - Duration::from_secs(threshold_seconds - 1),
            };
            let fresh_result = has_target_snapshot_aged_out(&retention_period, &fresh_snapshot).unwrap();
            assert!(!fresh_result);
        }
    }

}
