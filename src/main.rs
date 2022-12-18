mod applog;
mod connection;
mod error;
mod message;
mod statics;
mod structs;

use clap::{CommandFactory, Parser};
use clap_complete::{generate, shells};
use log::debug;
use message::*;
use std::env;
use std::io::{Error, ErrorKind};
use structs::Config;

#[derive(Debug, Parser)]
#[clap(rename_all = "kebab-case")]
enum Command {
    /// Gets user public key
    Key { user: String },
    /// Executes pam check
    Pam,
    /// Check configuration
    Check { confpath: std::path::PathBuf },
    /// Cleans caches up
    #[clap(alias = "cleanup")]
    CleanUp,
    /// Get rate limit for github api
    #[clap(alias = "ratelimit")]
    RateLimit,
    /// Displays version details
    Version,
    /// Displays completion
    Completion {
        #[clap(subcommand)]
        shell: Shell,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Parser)]
#[clap(rename_all = "kebab-case")]
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
    let command = Command::parse();
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
                Shell::Bash => shells::Shell::Bash,
                Shell::Fish => shells::Shell::Fish,
                Shell::Zsh => shells::Shell::Zsh,
                Shell::PowerShell => shells::Shell::PowerShell,
                Shell::Elvish => shells::Shell::Elvish,
            };
            let mut cmd = Command::command();
            generate(shell, &mut cmd, "wagon", &mut std::io::stdout());
        }
    };
    Ok(())
}
