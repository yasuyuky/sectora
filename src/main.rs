#[macro_use]
extern crate clap;
use clap::{Arg, App, SubCommand};
extern crate toml;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate glob;

#[macro_use]
extern crate lazy_static;

extern crate libc;

mod structs;
mod ghclient;
mod statics;
use statics::CONF_PATH;

fn main() {

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .subcommand(SubCommand::with_name("key")
                        .about("Gets user public key")
                        .arg(Arg::with_name("USER").required(true).index(1).help("user name")))
        .subcommand(SubCommand::with_name("pam").about("Executes pam check"))
        .subcommand(SubCommand::with_name("cleanup").about("Cleans caches up"))
        .get_matches();

    let config = match structs::Config::new(&CONF_PATH) {
        Ok(config) => config,
        Err(err) => {
            println!("Failed to open configuration file: {:?}", *CONF_PATH);
            println!("[{:?}]", err);
            std::process::exit(11);
        }
    };
    let client = match ghclient::GithubClient::new(&config) {
        Ok(client) => client,
        Err(err) => {
            println!("Failed to open github client [{:?}]", err);
            std::process::exit(21);
        }
    };

    match matches.subcommand() {
        ("key", Some(sub)) => client.print_user_public_key(sub.value_of("USER").unwrap()).unwrap(),
        ("cleanup", Some(_)) => client.clear_all_caches().unwrap(),
        ("pam", Some(_)) => {
            match std::env::var("PAM_USER") {
                Ok(user) => {
                    if client.check_pam(&user).unwrap() {
                        std::process::exit(0);
                    } else {
                        std::process::exit(1)
                    }
                }
                Err(e) => println!("PAM_USER: {}", e),
            }
        }
        (&_, _) => println!("{}", matches.usage()),
    }
}
