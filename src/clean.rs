use anyhow::{Context, Result};
use std::fs;

use crate::PirouetteDirEntry;
use crate::PirouetteRetentionTarget;

pub fn clean_snapshots(retention_target: &PirouetteRetentionTarget) -> Result<()> {
    log::info!(
        "Checking {:?} for expired snapshots",
        retention_target.period
    );
    let entries = fs::read_dir(&retention_target.path)
        .context("Failed to read snapshot directory contents")?;

    // Convert to abstracted testable type
    let typed_entries: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.into())
        .collect();

    let current_snapshot_count = typed_entries.len();
    log::info!(
        "Currently {current_snapshot_count} snapshots, want to keep {}",
        retention_target.max_count
    );

    // Are there more snapshots than the user wants?
    if current_snapshot_count < retention_target.max_count {
        return Ok(());
    }

    // If so, we need to delete the excess
    let expired_snapshot_count = current_snapshot_count - retention_target.max_count;
    log::info!("Deleting {expired_snapshot_count} expired snapshots");

    if let Ok(expired_snapshots) = get_expired_snapshots(typed_entries, expired_snapshot_count) {
        delete_snapshots(expired_snapshots);
    }

    Ok(())
}

fn get_expired_snapshots(
    entries: Vec<PirouetteDirEntry>,
    count: usize,
) -> Result<Vec<PirouetteDirEntry>> {
    // Sort the snapshots from oldest -> newest
    let mut sorted_entries = entries;
    sorted_entries.sort_by_key(|entry| entry.created);

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
                println!("{err}");
            }
        }
        if snapshot.path.is_file() {
            if let Err(err) = fs::remove_file(&snapshot.path) {
                println!("{err}");
            }
        }
    }
}
