use std::env;
use std::string::String;
use std::path::Path;

lazy_static! {
    static ref DEFAULT_CONF_PATH_STR:String = String::from("/etc/ghteam-auth.conf");
    static ref CONF_PATH_STR:String = env::var("GHTEAMAUTH_CONFIG").unwrap_or(DEFAULT_CONF_PATH_STR.clone());
    pub static ref CONF_PATH:&'static Path = Path::new(&(*CONF_PATH_STR));
}
