use anyhow::{Context, Result};
use std::fs;
use std::time;
use std::path::PathBuf;
use chrono;

use configuration::ConfigRetentionKind;
use configuration::Config;
mod configuration;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;

    let rotation_targets = get_rotation_targets(&config)?;

    if !rotation_targets.is_empty() {
        for retention_kind in rotation_targets {
            copy_snapshot(&config, retention_kind)
                .with_context(|| format!("failed to create snapshot for {}", retention_kind))?;
        }
    }

    // todo: clean up old snaps
    Ok(())
}

/*
    Check current target state
*/

fn get_rotation_targets(config: &Config) -> Result<Vec<&ConfigRetentionKind>> {
    let mut rotation_targets = vec![];

    for (retention_period, _retention_value) in &config.retention {
        
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        // Path doesn't already exist, but we can try to create it ourselves
        if !retention_path.exists() {
            fs::create_dir_all(&retention_path)
                .with_context(|| format!("failed to create directory {}", retention_path.display()))?;
        }

        let newest_child = get_newest_directory_child(&retention_path);
        match newest_child {
            // If there's existing snapshots, check if they're old enough to need rotation
            Some(file) => match has_target_snapshot_aged_out(&retention_period, &file)? {
                true => rotation_targets.push(retention_period),
                false => (),
            },

            // If there's no previous snapshots, we always need to rotate
            None => rotation_targets.push(retention_period),
        }
    }

    Ok(rotation_targets)
}

fn get_newest_directory_child(directory: &PathBuf) -> Option<fs::DirEntry> {
    let dir_entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(_) => return None, 
    };

    let dir_items = dir_entries.flatten().filter(|item| 
        match item.metadata() {
            Ok(item_metadata) => item_metadata.is_dir(),
            Err(_) => false,
        }
    );

    let newest_child = dir_items.max_by_key(|item|
        match item.metadata() {
            Ok(item_metadata) => match item_metadata.created() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    );

    newest_child
}

fn has_target_snapshot_aged_out(retention_kind: &ConfigRetentionKind, file: &fs::DirEntry) -> Result<bool> {
    let file_metadata = match file.metadata() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for file {:?}: {}", file, e)),
        Ok(file_metadata) => file_metadata,
    };
    let file_time = match file_metadata.created() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for file {:?}: {}", file, e)),
        Ok(file_time) => file_time,
    };

    let file_age = time::SystemTime::now().duration_since(file_time)
        .context("Failed to calculate file age")?;

    let age_threshold = match retention_kind {
        ConfigRetentionKind::Hours => 60 * 60,
        ConfigRetentionKind::Days => 24 * 60 * 60,
        ConfigRetentionKind::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionKind::Months => 30 * 24 * 60 * 60,
        ConfigRetentionKind::Years => 365 * 24 * 60 * 60,
    };

    if file_age.as_secs() >= age_threshold {
        return Ok(true);
    } else {
        return Ok(false);
    }
}

/*
    Create source snapshot
*/

fn copy_snapshot(config: &Config, retention_kind: &ConfigRetentionKind) -> Result<()> {
    let retention_dir_path: PathBuf = [
        config.target.path.display().to_string(),
        retention_kind.to_string(),
        format_snapshot_dir_name()
        ].iter().collect();

    fs::create_dir_all(&retention_dir_path)
        .with_context(|| format!("failed to create directory {}", retention_dir_path.display()))?;

    let retention_file_path: PathBuf = [
        retention_dir_path,
        config.source.path.file_name().unwrap().into(),
    ].iter().collect();

    match config.source.path.is_file() {
        true => copy_snapshot_file(config, &retention_file_path)?,
        false => copy_snapshot_dir(config, &retention_file_path)?,
    }

    Ok(())
}

fn copy_snapshot_dir(config: &Config, retention_file_path: &PathBuf) -> Result<()> {
    let options = uu_cp::Options {
        attributes: uu_cp::Attributes::NONE, 
        attributes_only: false,
        copy_contents: false,
        cli_dereference: false,
        copy_mode: uu_cp::CopyMode::Copy,
        dereference: true,
        one_file_system: false,
        parents: false,
        update: uu_cp::UpdateMode::ReplaceAll,
        debug: false,
        verbose: false,
        strip_trailing_slashes: false,
        reflink_mode: uu_cp::ReflinkMode::Auto,
        sparse_mode: uu_cp::SparseMode::Auto,
        backup: uu_cp::BackupMode::NoBackup,
        backup_suffix: "~".to_owned(),
        no_target_dir: false,
        overwrite: uu_cp::OverwriteMode::Clobber(uu_cp::ClobberMode::Standard),
        recursive: true,
        target_dir: None,
        progress_bar: false
    };

    uu_cp::copy(&[config.source.path.clone()],  &retention_file_path, &options)
        .with_context(|| format!("failed to copy directory {}", config.source.path.display()))?;

    Ok(())
}

fn copy_snapshot_file(config: &Config, retention_file_path: &PathBuf) -> Result<()> {
    fs::copy(&config.source.path, retention_file_path)
        .with_context(|| format!("failed to copy file {}", config.source.path.display()))?;

    Ok(())
}

fn format_snapshot_dir_name() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M")
        .to_string()
}
