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
        .filter(|entry| {
            glob_includes(
                &format_inner_entry_path(config, entry),
                &config.options.include,
            )
        })
        .filter(|entry| {
            glob_excludes(
                &format_inner_entry_path(config, entry),
                &config.options.exclude,
            )
        });

    dry_run!(
        config.options.dry_run,
        format!("snapshot will not be created"),
        {
            match snapshot_output_format {
                ConfigOptsOutputFormat::Directory => {
                    copy_snapshot_to_dir(config, source_contents, &snapshot_path)
                }
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

fn copy_snapshot_to_dir<I>(
    config: &Config,
    source_contents: I,
    snapshot_path: &PathBuf,
) -> Result<()>
where
    I: Iterator<Item = PirouetteDirEntry>,
{
    fs::create_dir_all(snapshot_path)
        .with_context(|| format!("failed to create directory {snapshot_path:?}"))?;

    for entry in source_contents {
        let inner_entry_path = format_inner_entry_path(config, &entry);
        let target_entry_path: PathBuf = [snapshot_path, &inner_entry_path]
            .iter()
            .collect();
        log::debug!("Copying {:?} to {target_entry_path:?}", entry.path);

        if let Some(parent) = target_entry_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create directory {parent:?}"))?;
        }

        fs::copy(&entry.path, &target_entry_path)
            .with_context(|| format!("failed to copy file {:?}", &entry.path))?;
    }

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
        let inner_entry_path = format_inner_entry_path(config, &entry);
        log::debug!("Copying {:?} to {inner_entry_path:?}", entry.path);

        let mut f = fs::File::open(&entry.path)
            .with_context(|| format!("Failed to read file {:?}", &entry.path))?;

        snapshot_archive
            .append_file(inner_entry_path, &mut f)
            .with_context(|| format!("Failed to write tarball {snapshot_path:?}"))?;
    }

    snapshot_archive
        .into_inner()
        .with_context(|| format!("failed to close tarball {snapshot_path:?}"))?;

    Ok(())
}

fn format_inner_entry_path(config: &Config, entry: &PirouetteDirEntry) -> PathBuf {
    // For some entry "/path/to/source/foo/bar.txt", return the inner path "foo/bar.txt"
    entry
        .path
        .strip_prefix(&config.source.path)
        .unwrap()
        .into()
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

fn glob_includes(path: &PathBuf, patterns: &[Pattern]) -> bool {
    let result = match patterns.is_empty() {
        true => true,
        false => patterns.iter().any(|pat| pat.matches_path(path)),
    };

    log::debug!("Testing if {path:?} include-matches {patterns:?}: result={result}");

    result
}

fn glob_excludes(path: &PathBuf, patterns: &[Pattern]) -> bool {
    let result = match patterns.is_empty() {
        true => true,
        // NOT .any() == none
        false => !patterns.iter().any(|pat| pat.matches_path(path)),
    };

    log::debug!("Testing if {path:?} exclude-matches {patterns:?}: result={result}");

    result
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
            .filter(|entry| glob_includes(&entry.path, &include_patterns))
            .filter(|entry| glob_excludes(&entry.path, &exclude_patterns))
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
            .filter(|entry| glob_includes(&entry.path, &include_patterns))
            .filter(|entry| glob_excludes(&entry.path, &exclude_patterns))
            .collect();

        assert_eq!(result_data, expected_data);
    }
}
