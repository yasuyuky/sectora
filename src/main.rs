mod applog;
mod connection;
mod error;
mod message;
mod statics;
mod structs;

use log::debug;
use message::*;
use std::env;
use std::io::{Error, ErrorKind};
use structopt::StructOpt;
use structs::Config;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Command {
    /// Gets user public key
    Key {
        #[structopt(parse(from_str))]
        user: String,
    },
    /// Executes pam check
    Pam,
    /// Check configuration
    Check {
        #[structopt(parse(from_os_str))]
        confpath: std::path::PathBuf,
    },
    /// Cleans caches up
    #[structopt(alias = "cleanup")]
    CleanUp,
    /// Get rate limit for github api
    #[structopt(alias = "ratelimit")]
    RateLimit,
    /// Displays version details
    Version,
    /// Displays completion
    Completion {
        #[structopt(subcommand)]
        shell: Shell,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
enum Shell {
    Bash,
    Fish,
    Zsh,
    PowerShell,
    Elvish,
}

fn show_keys(conn: &connection::Connection, user: &str) -> Result<(), Error> {
    match conn.communicate(ClientMessage::Key { user: user.to_owned() }) {
        Ok(DaemonMessage::Key { keys }) => {
            println!("{}", keys);
            Ok(())
        }
        _ => Err(Error::new(ErrorKind::PermissionDenied, "key check failed")),
    }
}

fn main() -> Result<(), Error> {
    let command = Command::from_args();
    let conn = match connection::Connection::new(&format!("{:?}", command)) {
        Ok(conn) => conn,
        Err(err) => return Err(Error::new(ErrorKind::Other, format!("{:?}", err))),
    };
    debug!("connected to socket: {:?}", conn);

    match command {
        Command::Check { confpath } => match Config::from_path(&confpath) {
            Ok(_) => return Ok(()),
            Err(_) => return Err(Error::new(ErrorKind::Other, "check failed")),
        },
        Command::Key { user } => show_keys(&conn, &user)?,
        Command::Pam => match env::var("PAM_USER") {
            Ok(user) => match conn.communicate(ClientMessage::Pam { user }) {
                Ok(DaemonMessage::Pam { result }) => {
                    if result {
                        return Ok(());
                    } else {
                        return Err(Error::new(ErrorKind::NotFound, "user not found"));
                    }
                }
                _ => return Err(Error::new(ErrorKind::Other, "faild")),
            },
            Err(_) => return Err(Error::new(ErrorKind::ConnectionRefused, "failed")),
        },
        Command::CleanUp => match conn.communicate(ClientMessage::CleanUp) {
            Ok(_) => return Ok(()),
            Err(_) => return Err(Error::new(ErrorKind::Other, "failed")),
        },
        Command::RateLimit => match conn.communicate(ClientMessage::RateLimit) {
            Ok(DaemonMessage::RateLimit { limit,
                                          remaining,
                                          reset, }) => {
                println!("remaining: {}/{}, reset:{}", remaining, limit, reset);
            }
            _ => return Err(Error::new(ErrorKind::Other, "failed")),
        },
        Command::Version => {
            println!("{}",
                     concat!(env!("CARGO_PKG_VERSION"),
                             include_str!(concat!(env!("OUT_DIR"), "/commit-info.txt"))));
        }
        Command::Completion { shell } => {
            let shell = match shell {
                Shell::Bash => structopt::clap::Shell::Bash,
                Shell::Fish => structopt::clap::Shell::Fish,
                Shell::Zsh => structopt::clap::Shell::Zsh,
                Shell::PowerShell => structopt::clap::Shell::PowerShell,
                Shell::Elvish => structopt::clap::Shell::Elvish,
            };
            Command::clap().gen_completions_to("sectora", shell, &mut std::io::stdout());
        }
    };
    Ok(())
}
