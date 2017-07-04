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
use structs::Config;
mod ghclient;
use ghclient::GithubClient;

lazy_static! {
    static ref CONFIG:Config = Config::new(
        std::env::var("GHTEAMAUTH_CONFIG")
                 .unwrap_or(String::from("/etc/ghteam-auth.conf"))
                 .as_str()
    ).unwrap();
    static ref CLIENT:GithubClient = GithubClient::new(&CONFIG).unwrap();
}

fn main() {

    let matches = App::new("ghteam-auth")
        .version("0.1")
        .author("Yasuyuki YAMADA <yasuyuki.ymd@gmail.com>")
        .about("")
        .arg(Arg::with_name("v")
                 .short("v")
                 .multiple(true)
                 .help("Sets the level of verbosity"))
        .subcommand(SubCommand::with_name("key")
                        .about("get user public key")
                        .arg(Arg::with_name("USER")
                                 .required(true)
                                 .index(1)
                                 .help("user name")))
        .subcommand(SubCommand::with_name("pam").about("execute pam check"))
        .subcommand(SubCommand::with_name("refresh").about("refresh cache"))
        .get_matches();


    if let Some(matches) = matches.subcommand_matches("key") {
        CLIENT.print_user_public_key(matches.value_of("USER").unwrap())
              .unwrap();
    } else if let Some(_) = matches.subcommand_matches("refresh") {
        CLIENT.clear_all_caches().unwrap();
    } else if let Some(_) = matches.subcommand_matches("pam") {
        match std::env::var("PAM_USER") {
            Ok(user) => {
                if CLIENT.check_pam(&user).unwrap() {
                    std::process::exit(0);
                } else {
                    std::process::exit(1)
                }
            }
            Err(e) => println!("couldn't interpret PAM_USER: {}", e),
        }
    }

}
