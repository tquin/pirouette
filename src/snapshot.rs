use anyhow::{Context, Result};
use glob::Pattern;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;

use crate::PirouetteDirEntry;
use crate::PirouetteRetentionTarget;
use crate::configuration::Config;
use crate::configuration::ConfigOptsOutputFormat;
use crate::dry_run;

pub fn copy_snapshot(config: &Config, retention_target: &PirouetteRetentionTarget) -> Result<()> {
    let snapshot_output_format = &config.options.output_format;

    let snapshot_path = format_snapshot_path(retention_target, snapshot_output_format);
    log::info!(
        "Creating a {snapshot_output_format:?} {:?} snapshot at {snapshot_path:?}",
        retention_target.period
    );

    dry_run!(
        config.options.dry_run,
        format!("snapshot will not be created"),
        {
            match snapshot_output_format {
                ConfigOptsOutputFormat::Directory => copy_snapshot_to_dir(config, &snapshot_path),
                ConfigOptsOutputFormat::Tarball => copy_snapshot_to_tarball(config, &snapshot_path),
            }
        }
    )
}

fn format_snapshot_path(
    retention_target: &PirouetteRetentionTarget,
    snapshot_output_format: &ConfigOptsOutputFormat,
) -> PathBuf {
    let snapshot_timestamp = chrono::Local::now()
        .format("%Y-%m-%dT%H:%M")
        .to_string();

    match snapshot_output_format {
        ConfigOptsOutputFormat::Directory => {
            [retention_target.path.clone(), snapshot_timestamp.into()]
                .iter()
                .collect()
        }

        ConfigOptsOutputFormat::Tarball => [
            retention_target.path.clone(),
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

#[allow(dead_code)]
fn get_source_contents_iter(source_path: &PathBuf) -> impl Iterator<Item = PirouetteDirEntry> {
    WalkDir::new(source_path)
        .into_iter()
        .filter_map(|result| match result {
            Ok(entry) => Some(entry),
            Err(e) => {
                log::warn!("Error reading some source contents: {e}");
                None
            }
        })
        .filter(|entry| {
            let ft = entry.file_type();
            ft.is_file() || ft.is_symlink()
        })
        .map(|x| x.into())
}

impl PirouetteDirEntry {
    #[allow(dead_code)]
    fn glob_includes(&self, patterns: &[Pattern]) -> bool {
        patterns
            .iter()
            .any(|pat| pat.matches_path(&self.path))
    }

    #[allow(dead_code)]
    fn glob_excludes(&self, patterns: &[Pattern]) -> bool {
        // NOT .any() == none
        !patterns
            .iter()
            .any(|pat| pat.matches_path(&self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PirouetteDirEntry;
    use std::time::SystemTime;

    fn create_test_entries(paths: Vec<&str>) -> Vec<PirouetteDirEntry> {
        let mut entries = vec![];
        for path in paths {
            entries.push(PirouetteDirEntry {
                path: PathBuf::from(path),
                timestamp: SystemTime::UNIX_EPOCH,
            });
        }
        entries
    }

    #[test]
    fn test_glob_filters() {
        let test_data = create_test_entries(vec!["a/foo", "b/bar", "c", "d/baz"]).into_iter();

        let include_patterns = vec![
            glob::Pattern::new("a/*").unwrap(),
            glob::Pattern::new("b/*").unwrap(),
            glob::Pattern::new("c").unwrap(),
        ];

        let exclude_patterns: Vec<Pattern> = vec![
            glob::Pattern::new("b/*").unwrap(),
            glob::Pattern::new("d/*").unwrap(),
        ];

        let expected_data: Vec<PirouetteDirEntry> = create_test_entries(vec!["a/foo", "c"])
            .into_iter()
            .collect();

        let result_data: Vec<PirouetteDirEntry> = test_data
            .filter(|entry| entry.glob_includes(&include_patterns))
            .filter(|entry| entry.glob_excludes(&exclude_patterns))
            .collect();

        assert_eq!(result_data, expected_data);
    }
}
