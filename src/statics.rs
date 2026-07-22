use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;

const DEFAULT_CONF_PATH_STR: &str = "/etc/sectora.conf";

static CONF_PATH_STR: LazyLock<String> =
    LazyLock::new(|| env::var("SECTORA_CONFIG").unwrap_or(String::from(DEFAULT_CONF_PATH_STR)));
pub static CONF_PATH: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from((*CONF_PATH_STR).clone()));
