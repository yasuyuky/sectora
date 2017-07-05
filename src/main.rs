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
use statics::CLIENT;

fn main() {

    let matches = App::new(crate_name!())
        .version(crate_version!())
        .author(crate_authors!())
        .about(crate_description!())
        .arg(Arg::with_name("v")
                 .short("v")
                 .multiple(true)
                 .help("Sets the level of verbosity"))
        .subcommand(SubCommand::with_name("key")
                        .about("Gets user public key")
                        .arg(Arg::with_name("USER")
                                 .required(true)
                                 .index(1)
                                 .help("user name")))
        .subcommand(SubCommand::with_name("pam").about("Executes pam check"))
        .subcommand(SubCommand::with_name("cleanup").about("Cleans caches up"))
        .get_matches();

    match matches.subcommand() {
        ("key", Some(sub)) => {
            CLIENT.print_user_public_key(sub.value_of("USER").unwrap())
                  .unwrap()
        }
        ("cleanup", Some(_)) => CLIENT.clear_all_caches().unwrap(),
        ("pam", Some(_)) => {
            match std::env::var("PAM_USER") {
                Ok(user) => {
                    if CLIENT.check_pam(&user).unwrap() {
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
