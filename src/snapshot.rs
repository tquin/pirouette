use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

use crate::configuration::Config;
use crate::configuration::ConfigOptsOutputFormat;
use crate::configuration::ConfigRetentionKind;

pub fn copy_snapshot(config: &Config, retention_period: &ConfigRetentionKind) -> Result<()> {
    let snapshot_output_format = &config.options.output_format;

    let retention_path: PathBuf = [
        config.target.path.display().to_string(),
        retention_period.to_string(),
    ]
    .iter()
    .collect();

    fs::create_dir_all(&retention_path)
        .with_context(|| format!("failed to create directory {retention_path:?}"))?;

    let snapshot_path = format_snapshot_path(retention_path, snapshot_output_format);

    match snapshot_output_format {
        ConfigOptsOutputFormat::Directory => copy_snapshot_to_dir(config, &snapshot_path)?,
        ConfigOptsOutputFormat::Tarball => copy_snapshot_to_tarball(config, &snapshot_path)?,
    }

    Ok(())
}

fn format_snapshot_path(
    retention_path: PathBuf,
    snapshot_output_format: &ConfigOptsOutputFormat,
) -> PathBuf {
    let snapshot_timestamp = chrono::Local::now()
        .format("%Y-%m-%dT%H:%M")
        .to_string();

    match snapshot_output_format {
        ConfigOptsOutputFormat::Directory => [retention_path.clone(), snapshot_timestamp.into()]
            .iter()
            .collect(),

        ConfigOptsOutputFormat::Tarball => [
            retention_path.clone(),
            format!("{snapshot_timestamp}.tgz").into(),
        ]
        .iter()
        .collect(),
    }
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
        progress_bar: false,
    };

    fs::create_dir_all(snapshot_path)
        .with_context(|| format!("failed to create directory {snapshot_path:?}"))?;

    uu_cp::copy(&[config.source.path.clone()], snapshot_path, &options)
        .with_context(|| format!("failed to copy directory {:?}", config.source.path))?;

    Ok(())
}

fn copy_snapshot_to_tarball(config: &Config, snapshot_path: &PathBuf) -> Result<()> {
    let snapshot_file = fs::File::create(snapshot_path)
        .with_context(|| format!("failed to create tarball {snapshot_path:?}"))?;

    let snapshot_writer =
        flate2::write::GzEncoder::new(&snapshot_file, flate2::Compression::best());
    let mut snapshot_archive = tar::Builder::new(snapshot_writer);

    match &config.source.path.is_dir() {
        // Recursive copy directory contents to root of tar file
        true => snapshot_archive
            .append_dir_all(".", &config.source.path)
            .with_context(|| format!("Failed to write tarball {snapshot_path:?}"))?,

        // Write file contents into archive
        false => {
            let mut f = fs::File::open(&config.source.path)
                .with_context(|| format!("Failed to read file {:?}", &config.source.path))?;

            snapshot_archive
                .append_file(config.source.path.file_name().unwrap(), &mut f)
                .with_context(|| format!("Failed to write tarball {snapshot_path:?}"))?;
        }
    }

    snapshot_archive
        .into_inner()
        .with_context(|| format!("failed to close tarball {snapshot_path:?}"))?;

    Ok(())
}
