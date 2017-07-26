use std;
use std::env;
use std::string::String;
use std::path::Path;
use structs::Config;
use ghclient::GithubClient;

lazy_static! {
    static ref DEFAULT_CONF_PATH_STR:String = String::from("/etc/ghteam-auth.conf");
    static ref CONF_PATH_STR:String = env::var("GHTEAMAUTH_CONFIG").unwrap_or(DEFAULT_CONF_PATH_STR.clone());
    static ref CONF_PATH:&'static Path = Path::new(&(*CONF_PATH_STR));
    pub static ref CONFIG:Config = match Config::new(&CONF_PATH) {
        Ok(config) => config,
        Err(err) => {
            println!("Failed to open configuration file: {:?}", *CONF_PATH);
            println!("[{:?}]", err);
            std::process::exit(11);
        }
    };
    pub static ref CLIENT:GithubClient = match GithubClient::new(&CONFIG) {
        Ok(client) => client,
        Err(err) => {
            println!("Failed to open github client [{:?}]", err);
            std::process::exit(21);
        }
    };
}
