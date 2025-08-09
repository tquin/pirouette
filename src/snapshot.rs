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

    let source_contents = get_source_contents_iter(&config.source.path)
        .filter(|entry| entry.glob_includes(&config.options.include))
        .filter(|entry| entry.glob_excludes(&config.options.exclude));

    dry_run!(
        config.options.dry_run,
        format!("snapshot will not be created"),
        {
            match snapshot_output_format {
                ConfigOptsOutputFormat::Directory => copy_snapshot_to_dir(config, &snapshot_path),
                ConfigOptsOutputFormat::Tarball => {
                    copy_snapshot_to_tarball(config, source_contents, &snapshot_path)
                }
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

fn copy_snapshot_to_tarball<I>(
    config: &Config,
    source_contents: I,
    snapshot_path: &PathBuf,
) -> Result<()>
where
    I: Iterator<Item = PirouetteDirEntry>,
{
    let snapshot_file = fs::File::create(snapshot_path)
        .with_context(|| format!("failed to create tarball {snapshot_path:?}"))?;

    let snapshot_writer =
        flate2::write::GzEncoder::new(&snapshot_file, flate2::Compression::best());
    let mut snapshot_archive = tar::Builder::new(snapshot_writer);

    for entry in source_contents {
        log::debug!("Copying {:?} to tarball", &entry.path);

        let mut f = fs::File::open(&entry.path)
            .with_context(|| format!("Failed to read file {:?}", &entry.path))?;

        let tarball_entry_path = format_tarball_entry_path(config, &entry);

        snapshot_archive
            .append_file(tarball_entry_path, &mut f)
            .with_context(|| format!("Failed to write tarball {snapshot_path:?}"))?;
    }

    snapshot_archive
        .into_inner()
        .with_context(|| format!("failed to close tarball {snapshot_path:?}"))?;

    Ok(())
}

// For some entry "/path/to/source/foo/bar.txt", return the inner path "source/foo/bar.txt"
fn format_tarball_entry_path(config: &Config, entry: &PirouetteDirEntry) -> PathBuf {
    let prefix: PathBuf = match config.source.path.parent() {
        Some(prefix) => prefix.into(),
        None => config.source.path.clone(),
    };
    entry.path.strip_prefix(prefix).unwrap().into()
}

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
    fn glob_includes(&self, patterns: &[Pattern]) -> bool {
        if patterns.is_empty() {
            return true;
        }

        patterns
            .iter()
            .any(|pat| pat.matches_path(&self.path))
    }

    fn glob_excludes(&self, patterns: &[Pattern]) -> bool {
        if patterns.is_empty() {
            return true;
        }

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
    fn test_glob_with_filters() {
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

    #[test]
    fn test_glob_empty_filters() {
        let test_data = create_test_entries(vec!["a/foo", "b/bar", "c", "d/baz"]).into_iter();

        let include_patterns = vec![];

        let exclude_patterns: Vec<Pattern> = vec![];

        let expected_data: Vec<PirouetteDirEntry> =
            create_test_entries(vec!["a/foo", "b/bar", "c", "d/baz"])
                .into_iter()
                .collect();

        let result_data: Vec<PirouetteDirEntry> = test_data
            .filter(|entry| entry.glob_includes(&include_patterns))
            .filter(|entry| entry.glob_excludes(&exclude_patterns))
            .collect();

        assert_eq!(result_data, expected_data);
    }
}
