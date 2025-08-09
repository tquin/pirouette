use anyhow::{Context, Result};
use std::fs;

use crate::PirouetteDirEntry;
use crate::PirouetteRetentionTarget;
use crate::configuration::Config;
use crate::dry_run;

pub fn clean_snapshots(config: &Config, retention_target: &PirouetteRetentionTarget) -> Result<()> {
    log::info!(
        "Checking {:?} for expired snapshots",
        retention_target.period
    );
    let entries = get_directory_entries(retention_target);

    let current_snapshot_count = entries.len();
    log::info!(
        "Currently {current_snapshot_count} snapshots, want to keep {}",
        retention_target.max_count
    );

    // Are we under the configured retention threshold?
    if current_snapshot_count <= retention_target.max_count {
        return Ok(());
    }

    // If so, we need to delete the excess
    let expired_snapshot_count = current_snapshot_count - retention_target.max_count;
    log::info!("Deleting {expired_snapshot_count} expired snapshots");

    if let Ok(expired_snapshots) = get_expired_snapshots(entries, expired_snapshot_count) {
        dry_run!(
            config.options.dry_run,
            format!("snapshots will not be deleted"),
            {
                delete_snapshots(expired_snapshots);
                // This function doesn't fail, but dry_run!() expects a Result<>
                Ok::<(), anyhow::Error>(())
            }
        )
    } else {
        log::warn!("Failed to calculate expired snapshots");
        Ok(())
    }
}

fn get_directory_entries(target: &PirouetteRetentionTarget) -> Vec<PirouetteDirEntry> {
    let entries = match fs::read_dir(&target.path) {
        Ok(entries) => entries,
        Err(_) => {
            log::warn!("failed to read {:?} contents", &target.path);
            return vec![];
        }
    };

    // Convert to abstracted testable type
    entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into())
        .collect()
}

fn get_expired_snapshots(
    entries: Vec<PirouetteDirEntry>,
    count: usize,
) -> Result<Vec<PirouetteDirEntry>> {
    // Sort the snapshots from oldest -> newest
    let mut sorted_entries = entries;
    sorted_entries.sort_by_key(|entry| entry.timestamp);

    // In theory, this fails if count > len, but we already early return
    // in the parent function for that case, so this should always be Ok()
    let (expired_snapshots, _) = sorted_entries
        .split_at_checked(count)
        .context("Failed to calculate expired snapshots")?;

    let mut result = vec![];
    for entry in expired_snapshots {
        result.push(entry.clone());
    }

    Ok(result)
}

fn delete_snapshots(expired_snapshots: Vec<PirouetteDirEntry>) {
    for snapshot in expired_snapshots {
        log::info!("Deleting {snapshot}");

        if snapshot.path.is_dir() {
            if let Err(err) = fs::remove_dir_all(&snapshot.path) {
                log::error!("{err}");
            }
        } else if snapshot.path.is_file()
            && let Err(err) = fs::remove_file(&snapshot.path)
        {
            log::error!("{err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn test_expired_snapshot_count() {
        let mut test_data = vec![];
        for i in 0..10 {
            test_data.push(PirouetteDirEntry {
                path: PathBuf::from("/tmp/fake"),
                timestamp: UNIX_EPOCH + Duration::from_secs(i),
            })
        }

        // Should return the number of entries we asked for
        for i in 0..10 {
            assert_eq!(
                get_expired_snapshots(test_data.clone(), i)
                    .unwrap()
                    .len(),
                i
            );
        }
    }

    #[test]
    fn test_expired_snapshot_order() {
        let earlier_entry = PirouetteDirEntry {
            path: PathBuf::from("/tmp/fake"),
            timestamp: UNIX_EPOCH + Duration::from_secs(1),
        };
        let later_entry = PirouetteDirEntry {
            path: PathBuf::from("/tmp/fake"),
            timestamp: UNIX_EPOCH + Duration::from_secs(2),
        };

        let test_data = vec![earlier_entry.clone(), later_entry.clone()];
        let result = get_expired_snapshots(test_data, 1).unwrap();

        assert!(result.contains(&earlier_entry));
        assert!(!result.contains(&later_entry));
    }
}
