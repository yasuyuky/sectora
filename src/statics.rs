use std::env;
use std::path::Path;
use std::string::String;

const DEFAULT_CONF_PATH_STR: &str = "/etc/sectora.conf";

lazy_static! {
    static ref CONF_PATH_STR: String = env::var("SECTORA_CONFIG").unwrap_or(String::from(DEFAULT_CONF_PATH_STR));
    pub static ref CONF_PATH: &'static Path = Path::new(&(*CONF_PATH_STR));
}
