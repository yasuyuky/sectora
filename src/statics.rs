use ghclient::GithubClient;
use std;
use std::env;
use std::path::Path;
use std::string::String;
use structs::Config;

lazy_static! {
    static ref DEFAULT_CONF_PATH_STR:String = String::from("/etc/sectora.conf");
    static ref CONF_PATH_STR:String = env::var("SECTORA_CONFIG").unwrap_or(DEFAULT_CONF_PATH_STR.clone());
    static ref CONF_PATH:&'static Path = Path::new(&(*CONF_PATH_STR));
    pub static ref CONFIG:Config = match Config::new(&CONF_PATH) {
        Ok(config) => config,
        Err(err) => {
            println!("Failed to open configuration file: {:?}", *CONF_PATH);
            println!("[{:?}]", err);
            std::process::exit(-2);
        }
    };
    pub static ref CLIENT:GithubClient = GithubClient::new(&CONFIG);
}
