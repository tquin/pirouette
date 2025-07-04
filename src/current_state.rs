use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::PirouetteDirEntry;
use crate::configuration::Config;
use crate::configuration::ConfigRetentionKind;

pub fn get_rotation_targets(config: &Config) -> Result<Vec<&ConfigRetentionKind>> {
    let mut rotation_targets = vec![];
    log::info!("Retention periods: {:?}", config.retention.keys());

    for retention_period in config.retention.keys() {
        log::info!("Checking existing state for {retention_period}");
        let retention_path: PathBuf = [
            config.target.path.display().to_string(),
            retention_period.to_string(),
        ]
        .iter()
        .collect();

        // Path doesn't already exist, but we can try to create it ourselves
        if !retention_path.exists() {
            log::info!(
                "Retention directory {retention_path:?} does not exist, attempting to create it"
            );
            fs::create_dir_all(&retention_path).with_context(|| {
                format!("failed to create directory {}", retention_path.display())
            })?;
        }

        let newest_entry = get_newest_directory_entry(&retention_path);
        match newest_entry {
            // If there's existing snapshots, check if they're old enough to need rotation
            Some(snapshot) => {
                if has_target_snapshot_aged_out(retention_period, &snapshot) {
                    log::info!("{retention_path:?} requires a new snapshot");
                    rotation_targets.push(retention_period);
                } else {
                    log::info!("{retention_path:?} does not require a new snapshot");
                }
            }

            // If there's no previous snapshots, we always need to rotate
            None => {
                log::info!("{retention_path:?} requires a new snapshot");
                rotation_targets.push(retention_period);
            }
        }
    }

    log::info!("Snapshots which require rotating: {rotation_targets:?}");
    Ok(rotation_targets)
}

fn get_newest_directory_entry(directory: &PathBuf) -> Option<PirouetteDirEntry> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    // Convert to abstracted testable type
    let typed_entries: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into())
        .collect();

    log::info!(
        "{directory:?} contains {} existing entries",
        typed_entries.len()
    );

    // Return the newest item in the directory
    typed_entries
        .into_iter()
        .max_by_key(|entry: &PirouetteDirEntry| entry.created)
}

fn has_target_snapshot_aged_out(
    retention_period: &ConfigRetentionKind,
    snapshot: &PirouetteDirEntry,
) -> bool {
    let snapshot_age = SystemTime::now().duration_since(snapshot.created);

    let age_threshold = match retention_period {
        ConfigRetentionKind::Hours => 60 * 60,
        ConfigRetentionKind::Days => 24 * 60 * 60,
        ConfigRetentionKind::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionKind::Months => 30 * 24 * 60 * 60,
        ConfigRetentionKind::Years => 365 * 24 * 60 * 60,
    };

    match snapshot_age {
        Err(_) => {
            log::warn!(
                "Age was in the future for {:?}, is the system clock correct?",
                snapshot.path
            );
            false
        }
        Ok(snapshot_age) => snapshot_age.as_secs() >= age_threshold,
    }
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
            let expired_result = has_target_snapshot_aged_out(&retention_period, &expired_snapshot);
            assert!(expired_result);

            let fresh_snapshot = PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                // This assumes the function will return within 1 second
                created: SystemTime::now() - Duration::from_secs(threshold_seconds - 1),
            };
            let fresh_result = has_target_snapshot_aged_out(&retention_period, &fresh_snapshot);
            assert!(!fresh_result);
        }
    }
}
