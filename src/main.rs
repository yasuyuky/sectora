extern crate futures;
extern crate glob;
extern crate hyper;
extern crate hyper_rustls;
#[macro_use]
extern crate lazy_static;
extern crate libc;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
#[macro_use]
extern crate structopt;
extern crate tokio_core;
extern crate toml;

mod ghclient;
mod statics;
mod structs;
#[macro_use]
mod syslog;

use statics::CONF_PATH;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
enum Command {
    /// Gets user public key
    #[structopt(name = "key")]
    Key {
        #[structopt(parse(from_str))]
        user: String,
    },
    /// Executes pam check
    #[structopt(name = "pam")]
    Pam,
    /// Check configuration
    #[structopt(name = "check")]
    Check {
        #[structopt(parse(from_os_str))]
        confpath: std::path::PathBuf,
    },
    /// Cleans caches up
    #[structopt(name = "cleanup")]
    CleanUp,
}

fn main() {
    let command = Command::from_args();

    use std::env;
    use std::process;

    let config = match structs::Config::new(&CONF_PATH) {
        Ok(c) => c,
        Err(_) => {
            syslog!(libc::LOG_WARNING, "sectora fail to open config.");
            process::exit(2);
        }
    };
    let client = ghclient::GithubClient::new(&config);

    match command {
        Command::Check { confpath } => match structs::Config::new(&confpath) {
            Ok(_) => process::exit(0),
            Err(_) => process::exit(11),
        },
        Command::Key { user } => match client.print_user_public_key(&user) {
            Ok(_) => process::exit(0),
            Err(_) => process::exit(21),
        },
        Command::Pam => match env::var("PAM_USER") {
            Ok(user) => match client.check_pam(&user) {
                Ok(true) => process::exit(0),
                Ok(false) => process::exit(1),
                Err(_) => process::exit(31),
            },
            Err(_) => process::exit(41),
        },
        Command::CleanUp => match client.clear_all_caches() {
            Ok(_) => process::exit(0),
            Err(_) => process::exit(51),
        },
    };
}
