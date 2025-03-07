
mod configuration;

fn main() {
    let config_file_toml = configuration::parse_config();
    println!("{:#?}", config_file_toml);
}
