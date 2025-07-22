use anyhow::{Context, Result};
use std::fs;
use std::time::SystemTime;

use crate::DisplayVec;
use crate::PirouetteDirEntry;
use crate::PirouetteRetentionTarget;
use crate::configuration::Config;
use crate::configuration::ConfigRetentionPeriod;
use crate::dry_run;

pub fn get_rotation_targets(
    config: &Config,
    all_targets: Vec<PirouetteRetentionTarget>,
) -> Result<Vec<PirouetteRetentionTarget>> {
    let mut rotation_targets = vec![];

    for retention_target in all_targets {
        log::info!("Checking existing state for {retention_target}");

        create_target_directory(config, &retention_target)?;

        match get_newest_directory_entry(&retention_target) {
            // If there's existing snapshots, check if they're old enough to need rotation
            Some(snapshot) => {
                if has_target_snapshot_aged_out(&retention_target, &snapshot) {
                    log::info!("{retention_target} requires a new snapshot");
                    rotation_targets.push(retention_target);
                } else {
                    log::info!("{retention_target} does not require a new snapshot",);
                }
            }

            // If there's no previous snapshots, we always need to rotate
            None => {
                log::info!("{retention_target} is empty and requires a new snapshot");
                rotation_targets.push(retention_target);
            }
        }
    }

    log::info!(
        "Snapshots which require rotating: {}",
        rotation_targets.display_vec()
    );
    Ok(rotation_targets)
}

fn create_target_directory(
    config: &Config,
    retention_target: &PirouetteRetentionTarget,
) -> Result<()> {
    if retention_target.path.exists() {
        return Ok(());
    }
    log::info!(
        "Retention directory {:?} does not exist, attempting to create it",
        retention_target.path
    );

    dry_run!(
        config.options.dry_run,
        format!("{:?} directory will not be created", retention_target.path),
        {
            fs::create_dir_all(&retention_target.path)
                .with_context(|| format!("failed to create directory {retention_target}"))
        }
    )
}

fn get_newest_directory_entry(
    retention_target: &PirouetteRetentionTarget,
) -> Option<PirouetteDirEntry> {
    let entries = match fs::read_dir(&retention_target.path) {
        Ok(entries) => entries,
        Err(_) => return None,
    };

    // Convert to abstracted testable type
    let typed_entries: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into())
        .collect();

    log::info!(
        "{retention_target} contains {} existing entries",
        typed_entries.len()
    );

    // Return the newest item in the directory
    typed_entries
        .into_iter()
        .max_by_key(|entry: &PirouetteDirEntry| entry.created)
}

fn has_target_snapshot_aged_out(
    retention_target: &PirouetteRetentionTarget,
    snapshot: &PirouetteDirEntry,
) -> bool {
    log::debug!("Checking age of snapshot: {snapshot:?}");

    let snapshot_age = SystemTime::now().duration_since(snapshot.created);

    let age_threshold = match retention_target.period {
        ConfigRetentionPeriod::Hours => 60 * 60,
        ConfigRetentionPeriod::Days => 24 * 60 * 60,
        ConfigRetentionPeriod::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionPeriod::Months => 30 * 24 * 60 * 60,
        ConfigRetentionPeriod::Years => 365 * 24 * 60 * 60,
    };

    match snapshot_age {
        Err(_) => {
            log::warn!("Age was in the future for {snapshot}, is the system clock correct?",);
            false
        }
        Ok(snapshot_age) => snapshot_age.as_secs() >= age_threshold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn test_has_target_snapshot_aged_out() {
        let test_params: Vec<(ConfigRetentionPeriod, u64)> = vec![
            (ConfigRetentionPeriod::Hours, 3600),
            (ConfigRetentionPeriod::Days, 86400),
            (ConfigRetentionPeriod::Weeks, 604800),
            (ConfigRetentionPeriod::Months, 2592000),
            (ConfigRetentionPeriod::Years, 31536000),
        ];

        for (retention_period, threshold_seconds) in test_params {
            let retention_target = PirouetteRetentionTarget {
                period: retention_period,
                path: PathBuf::from("/tmp"),
                max_count: 1,
            };

            let expired_snapshot = PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                created: SystemTime::now() - Duration::from_secs(threshold_seconds),
            };
            let expired_result = has_target_snapshot_aged_out(&retention_target, &expired_snapshot);
            assert!(expired_result);

            let fresh_snapshot = PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                // This assumes the function will return within 1 second
                created: SystemTime::now() - Duration::from_secs(threshold_seconds - 1),
            };
            let fresh_result = has_target_snapshot_aged_out(&retention_target, &fresh_snapshot);
            assert!(!fresh_result);
        }
    }
}
