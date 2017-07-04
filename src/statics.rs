use std::env;
use std::string::String;
use ghclient::GithubClient;
use structs::Config;

lazy_static! {
    static ref DEFAULT_CONF_PATH:String = String::from("/etc/ghteam-auth.conf");
    static ref CONF_PATH:String = env::var("GHTEAMAUTH_CONFIG").unwrap_or(DEFAULT_CONF_PATH.clone());
    pub static ref CONFIG:Config = Config::new(CONF_PATH.as_str()).unwrap();
    pub static ref CLIENT:GithubClient = GithubClient::new(&CONFIG).unwrap();
}
