use anyhow::{Context, Result};
use std::fs::DirEntry;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

mod clean;
mod configuration;
use crate::configuration::Config;
mod current_state;
mod snapshot;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;

    initialise_logger(&config);
    log::info!("Logger initialised");

    let rotation_targets = current_state::get_rotation_targets(&config)?;

    for retention_period in rotation_targets {
        snapshot::copy_snapshot(&config, retention_period)
            .with_context(|| format!("failed to create snapshot for {retention_period}"))?;
    }

    clean::clean_snapshots(&config)?;
    Ok(())
}

fn initialise_logger(config: &Config) {
    env_logger::Builder::from_default_env()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {}] {}",
                chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%Z"),
                record.level(),
                record.args()
            )
        })
        .filter_level(config.options.log_level)
        .init();
}

/*
    Shared Structs
*/

#[derive(Clone, Debug)]
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
            },
        }
    }
}
