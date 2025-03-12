use anyhow::{Result};

mod configuration;

fn main() -> Result<()> {
    let config = configuration::parse_config();
    println!("config {:#?}", config);

    Ok(())
}
