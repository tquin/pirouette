use anyhow::{Context, Result};

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
