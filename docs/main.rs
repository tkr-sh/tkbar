//! Generate the default configuration
use tkbar::Config;

fn main() -> Result<(), std::io::Error> {
    let default_config = Config::hardcoded();
    let file_content = toml::to_string(&default_config).expect("Should be a correct string");

    std::fs::write("./docs/config.toml", file_content.as_bytes())
}
