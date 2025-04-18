use anyhow::{Context, Result};
use std::fs;
use std::fs::DirEntry;
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

    clean_snapshots(&config)?;
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

        let newest_entry = get_newest_directory_entry(&retention_path);
        match newest_entry {
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

fn get_newest_directory_entry(directory: &PathBuf) -> Option<DirEntry> {
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

    let newest_entry = readable_entries.max_by_key(|entry|
        match entry.metadata() {
            Ok(entry_metadata) => match entry_metadata.created() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    );

    newest_entry
}

fn has_target_snapshot_aged_out(retention_kind: &ConfigRetentionKind, snapshot: &DirEntry) -> Result<bool> {
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

/*
    Snapshot cleanup
*/

fn clean_snapshots(config: &Config) -> Result<()> {
    for (retention_period, retention_value) in &config.retention {
        let mut retention_path = config.target.path.clone();
        retention_path.push(retention_period.to_string());

        let entries = fs::read_dir(&retention_path)
            .context("Failed to read snapshot directory contents")?;
    
        let readable_entries: Vec<DirEntry> = entries.filter_map(|entry|
            match entry {
                Ok(entry) => Some(entry),
                Err(_) => None,
            }
        ).collect();
        let current_snapshot_count = readable_entries.len();

        // Do we have more snapshots than the user wants to keep?
        if current_snapshot_count > *retention_value {
            let expired_snapshot_count = current_snapshot_count - *retention_value;

            match get_expired_snapshots(readable_entries, expired_snapshot_count) {
                Ok(expired_snapshots) => delete_snapshots(&expired_snapshots),
                Err(_) => (),
            }
        }
    }

    Ok(())
}

fn get_expired_snapshots(entries: Vec<DirEntry>, count: usize) -> Result<Vec<PathBuf>> {
    // Sort the snapshots from oldest -> newest
    let mut sorted_entries = entries;
    sorted_entries.sort_by_key(|entry|
        match entry.metadata() {
            Ok(entry_metadata) => match entry_metadata.modified() {
                Ok(time) => time,
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            },
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        }
    );
    
    let (expired_snapshots, _) = sorted_entries.split_at_checked(count)
        .context("Failed to calculate expired snapshots")?;

    let mut result = vec!();
    for entry in expired_snapshots {
        result.push(entry.path());
    }

    Ok(result)
}

fn delete_snapshots(expired_snapshots: &[PathBuf]) {
    for snapshot in expired_snapshots {
        match snapshot.is_dir() {
            true => {
                println!("deleting directory {:?}", snapshot);
                if let Err(err) = fs::remove_dir_all(snapshot) {
                    println!("{}", err);
                }
            },
            false => {
                println!("deleting file {:?}", snapshot);
                if let Err(err) = fs::remove_file(snapshot) {
                    println!("{}", err);
                }
            },
        }
    }
}
