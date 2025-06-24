use anyhow::{Context, Result};
use std::fs::DirEntry;
use std::time::SystemTime;
use std::path::PathBuf;

mod configuration;
mod snapshot;
mod clean;
mod check_targets;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;

    let rotation_targets = check_targets::get_rotation_targets(&config)?;

    if !rotation_targets.is_empty() {
        for retention_kind in rotation_targets {
            snapshot::copy_snapshot(&config, retention_kind)
                .with_context(|| format!("failed to create snapshot for {retention_kind}"))?;
        }
    }

    clean::clean_snapshots(&config)?;
    Ok(())
}

/*
    Shared Structs
*/

#[derive(Clone)]
pub struct PirouetteDirEntry {
    pub path: PathBuf,
    pub created: SystemTime,
}

impl From<DirEntry> for PirouetteDirEntry {
    fn from(entry: DirEntry) -> Self {
        PirouetteDirEntry {
            path: entry.path(),
            created: match entry.metadata() {
                Ok(entry_metadata) => match entry_metadata.created() {
                    Ok(time) => time,
                    Err(_) => SystemTime::UNIX_EPOCH,
                },
                Err(_) => SystemTime::UNIX_EPOCH,
            }
        }
    }
}
