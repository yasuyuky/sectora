use std::env;
use std::string::String;

lazy_static! {
    static ref DEFAULT_CONF_PATH:String = String::from("/etc/ghteam-auth.conf");
    pub static ref CONF_PATH:String = env::var("GHTEAMAUTH_CONFIG").unwrap_or(DEFAULT_CONF_PATH.clone());
}
