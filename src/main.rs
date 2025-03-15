use anyhow::{Result};

use crate::configuration::Config;
mod configuration;

fn main() -> Result<()> {
    let config = configuration::parse_config()?;
    println!("config {:#?}", config);

    check_target_state(&config);

    Ok(())
}

fn check_target_state(_config: &Config) {
    // let x = config.retention.all_retention_fields().into_iter();
    // println!("{:#?}", x);
}
