use anyhow::{Context, Result};
use std::fs;
use std::time;
use std::path::PathBuf;
use chrono;
use flate2;
use tar;

use configuration::ConfigRetentionKind;
use configuration::ConfigOptsOutputFormat;
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
            Some(snapshot) => match has_target_snapshot_aged_out(&retention_period, &snapshot)? {
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
        Ok(entries) => entries.flatten(),
        Err(_) => return None, 
    };

    let newest_child = dir_entries.max_by_key(|item|
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

fn has_target_snapshot_aged_out(retention_kind: &ConfigRetentionKind, snapshot: &fs::DirEntry) -> Result<bool> {
    let snapshot_metadata = match snapshot.metadata() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for snapshot {:?}: {}", snapshot, e)),
        Ok(snapshot_metadata) => snapshot_metadata,
    };
    let snapshot_time = match snapshot_metadata.created() {
        Err(e) => anyhow::bail!(format!("Failed to read metadata for snapshot {:?}: {}", snapshot, e)),
        Ok(snapshot_time) => snapshot_time,
    };

    let snapshot_age = time::SystemTime::now().duration_since(snapshot_time)
        .context("Failed to calculate snapshot age")?;

    let age_threshold = match retention_kind {
        ConfigRetentionKind::Hours => 60 * 60,
        ConfigRetentionKind::Days => 24 * 60 * 60,
        ConfigRetentionKind::Weeks => 7 * 24 * 60 * 60,
        ConfigRetentionKind::Months => 30 * 24 * 60 * 60,
        ConfigRetentionKind::Years => 365 * 24 * 60 * 60,
    };

    if snapshot_age.as_secs() >= age_threshold {
        return Ok(true);
    } else {
        return Ok(false);
    }
}

/*
    Create source snapshot
*/

fn copy_snapshot(config: &Config, retention_kind: &ConfigRetentionKind) -> Result<()> {
    // Default behaviour if unspecified is Directory
    let snapshot_output_format = match &config.options {
        Some(config_opts) => match &config_opts.output_format {
            Some(output_format) => output_format.clone(),
            None => ConfigOptsOutputFormat::Directory,
        },
        None => ConfigOptsOutputFormat::Directory,
    };

    let base_dir: PathBuf = [
        config.target.path.display().to_string(),
        retention_kind.to_string()
        ].iter().collect();

    fs::create_dir_all(&base_dir)
        .with_context(|| format!("failed to create directory {}", base_dir.display()))?;

    let snapshot_path: PathBuf = match snapshot_output_format {
        ConfigOptsOutputFormat::Directory => [
            base_dir.to_owned(),
            format_snapshot_name_time().into()
        ].iter().collect(),

        ConfigOptsOutputFormat::Tarball => [
            base_dir.to_owned(),
            format!("{}.tgz", format_snapshot_name_time()).into()
        ].iter().collect(),
    };

    match snapshot_output_format {
        ConfigOptsOutputFormat::Directory => copy_snapshot_to_dir(config, &snapshot_path)?,
        ConfigOptsOutputFormat::Tarball => copy_snapshot_to_tarball(config, &snapshot_path)?,
    }

    Ok(())
}

fn format_snapshot_name_time() -> String {
    chrono::Local::now()
        .format("%Y-%m-%dT%H:%M")
        .to_string()
}

fn copy_snapshot_to_dir(config: &Config, snapshot_path: &PathBuf) -> Result<()> {
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

    fs::create_dir_all(&snapshot_path)
        .with_context(|| format!("failed to create directory {}", snapshot_path.display()))?;

    uu_cp::copy(&[config.source.path.clone()],  &snapshot_path, &options)
        .with_context(|| format!("failed to copy directory {}", config.source.path.display()))?;

    Ok(())
}

fn copy_snapshot_to_tarball(config: &Config, snapshot_path: &PathBuf) -> Result<()> { 
    let snapshot_file = fs::File::create(snapshot_path)
        .with_context(|| format!("failed to create tarball {}", snapshot_path.display()))?;

    let snapshot_writer = flate2::write::GzEncoder::new(&snapshot_file, flate2::Compression::best());
    let mut snapshot_archive = tar::Builder::new(snapshot_writer);

    match &config.source.path.is_dir() {
        // Recursive copy directory contents to root of tar file
        true => snapshot_archive.append_dir_all(".", &config.source.path)
            .with_context(|| format!("Failed to write tarball {}", snapshot_path.display()))?,
        
        // Write file contents into archive
        false => {
            let mut f = fs::File::open(&config.source.path)
                .with_context(|| format!("Failed to read file {}", &config.source.path.display()))?;
            
            snapshot_archive.append_file(&config.source.path.file_name().unwrap(), &mut f)
                .with_context(|| format!("Failed to write tarball {}", snapshot_path.display()))?;
        },
    }

    snapshot_archive.into_inner()
        .with_context(|| format!("failed to close tarball {}", snapshot_path.display()))?;

    Ok(())
}
