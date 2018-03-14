#[macro_use]
extern crate clap;
use clap::{App, Arg, SubCommand};
extern crate glob;
extern crate reqwest;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;

#[macro_use]
extern crate lazy_static;

mod structs;
mod ghclient;
mod statics;
use statics::CLIENT;

fn main() {
    let user_arg = Arg::with_name("USER").required(true)
                                         .index(1)
                                         .help("user name");
    let conf_arg = Arg::with_name("CONF").required(true)
                                         .index(1)
                                         .help("conf path");
    let app = App::new(crate_name!()).version(crate_version!())
                                     .author(crate_authors!())
                                     .about(crate_description!())
                                     .subcommand(SubCommand::with_name("key").about("Gets user public key")
                                                                             .arg(user_arg))
                                     .subcommand(SubCommand::with_name("pam").about("Executes pam check"))
                                     .subcommand(SubCommand::with_name("cleanup").about("Cleans caches up"))
                                     .subcommand(SubCommand::with_name("check").about("Check configuration")
                                                                               .arg(conf_arg))
                                     .get_matches();

    match app.subcommand() {
        ("key", Some(sub)) => {
            CLIENT.print_user_public_key(sub.value_of("USER").unwrap())
                  .unwrap()
        }
        ("check", Some(sub)) => {
            match structs::Config::new(std::path::Path::new(sub.value_of("CONF").unwrap())) {
                Ok(_) => std::process::exit(0),
                Err(_) => std::process::exit(11),
            }
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
        (&_, _) => println!("{}", app.usage()),
    }
}
