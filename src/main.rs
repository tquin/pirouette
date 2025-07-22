use anyhow::{Context, Result};
use std::fmt;
use std::fs::DirEntry;
use std::io::Write;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::configuration::Config;
use crate::configuration::ConfigRetentionPeriod;

mod clean;
mod configuration;
mod current_state;
mod snapshot;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;

    initialise_logger(&config);
    log::info!("Logger initialised");
    log::debug!("Parsed config file:\n{config:#?}");

    let all_targets: Vec<PirouetteRetentionTarget> = get_all_retention_targets(&config);
    let rotation_targets = current_state::get_rotation_targets(&config, all_targets)?;

    for retention_target in rotation_targets {
        snapshot::copy_snapshot(&config, &retention_target)
            .with_context(|| format!("failed to create snapshot for {retention_target}"))?;

        clean::clean_snapshots(&config, &retention_target)?;
    }

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

fn get_all_retention_targets(config: &Config) -> Vec<PirouetteRetentionTarget> {
    let mut all_targets: Vec<PirouetteRetentionTarget> = vec![];

    for (retention_period, retention_value) in config.retention.iter() {
        all_targets.push(PirouetteRetentionTarget {
            period: retention_period.clone(),
            path: [
                config.target.path.display().to_string(),
                retention_period.to_string(),
            ]
            .iter()
            .collect(),
            max_count: *retention_value,
        });
    }

    all_targets
}

#[macro_export]
macro_rules! dry_run {
    ($dry_run:expr, $message:expr, $action:block) => {
        if $dry_run {
            log::debug!("[DRY RUN] {}", $message);
            Ok(())
        } else {
            $action
        }
    };
}

/*
    Shared Structs
*/

#[derive(Clone, Debug, PartialEq)]
pub struct PirouetteDirEntry {
    pub path: PathBuf,
    pub timestamp: SystemTime,
}

impl From<DirEntry> for PirouetteDirEntry {
    fn from(entry: DirEntry) -> Self {
        PirouetteDirEntry {
            path: entry.path(),
            timestamp: match entry.metadata() {
                Ok(entry_metadata) => match entry_metadata.modified() {
                    Ok(time) => time,
                    Err(_) => SystemTime::UNIX_EPOCH,
                },
                Err(_) => SystemTime::UNIX_EPOCH,
            },
        }
    }
}

impl fmt::Display for PirouetteDirEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.path)
    }
}

#[derive(Clone, Debug)]
pub struct PirouetteRetentionTarget {
    pub period: ConfigRetentionPeriod,
    pub path: PathBuf,
    pub max_count: usize,
}

impl fmt::Display for PirouetteRetentionTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.period)
    }
}

// This is just to pretty-print Vec<PirouetteRetentionTarget>
pub trait DisplayVec {
    fn display_vec(&self) -> String;
}

impl<T: std::fmt::Display> DisplayVec for Vec<T> {
    fn display_vec(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(|item| format!("{item}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}
