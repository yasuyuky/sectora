use std::env;
use std::path::Path;
use std::string::String;

lazy_static! {
    static ref DEFAULT_CONF_PATH_STR: String = String::from("/etc/sectora.conf");
    static ref CONF_PATH_STR: String = env::var("SECTORA_CONFIG").unwrap_or(DEFAULT_CONF_PATH_STR.clone());
    pub static ref CONF_PATH: &'static Path = Path::new(&(*CONF_PATH_STR));
}

pub const TEMP_DIRNAME: &'static str = "sectora-cache";
