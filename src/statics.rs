use once_cell::sync::Lazy;
use std::env;
use std::path::PathBuf;
use std::string::String;

const DEFAULT_CONF_PATH_STR: &str = "/etc/sectora.conf";

static CONF_PATH_STR: Lazy<String> =
    Lazy::new(|| env::var("SECTORA_CONFIG").unwrap_or(String::from(DEFAULT_CONF_PATH_STR)));
pub static CONF_PATH: Lazy<PathBuf> = Lazy::new(|| PathBuf::from((*CONF_PATH_STR).clone()));
