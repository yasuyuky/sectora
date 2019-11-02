extern crate glob;
extern crate http;
extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate log;
#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate structopt;
extern crate syslog;
extern crate toml;

mod applog;
mod error;
mod message;
mod statics;
mod structs;

use log::debug;
use message::*;
use statics::CONF_PATH;
use std::env;
use std::os::unix::net::UnixDatagram;
use std::process;
use std::time::Duration;
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

fn send_recv(socket: &UnixDatagram, msg: ClientMessage) -> Result<DaemonMessage, error::Error> {
    socket.send(msg.to_string().as_bytes())?;
    let mut buf = [0u8; 4096];
    let recv_cnt = socket.recv(&mut buf)?;
    let s = String::from_utf8(buf[..recv_cnt].to_vec()).unwrap();
    debug!("!!recv: {}", s);
    Ok(s.parse::<DaemonMessage>()?)
}

fn main() {
    let command = Command::from_args();

    applog::init(Some("sectora"));
    debug!("{:?}", command);
    let conf = Config::from_path(&CONF_PATH).unwrap_or_else(|_| process::exit(11));
    let client_socket_path = format!("{}/{}", &conf.socket_dir, std::process::id());
    let socket = UnixDatagram::bind(client_socket_path).unwrap_or_else(|_| process::exit(101));
    socket.set_read_timeout(Some(Duration::from_secs(5)))
          .unwrap_or_else(|_| process::exit(111));
    socket.connect(conf.socket_path).unwrap_or_else(|_| process::exit(121));
    debug!("connected to socket: {:?}", socket);

    match command {
        Command::Check { confpath } => match Config::from_path(&confpath) {
            Ok(_) => process::exit(0),
            Err(_) => process::exit(11),
        },
        Command::Key { user } => match send_recv(&socket, ClientMessage::Key { user }) {
            Ok(DaemonMessage::Key { keys }) => {
                println!("{}", keys);
                process::exit(0)
            }
            _ => process::exit(21),
        },
        Command::Pam => match env::var("PAM_USER") {
            Ok(user) => match send_recv(&socket, ClientMessage::Pam { user }) {
                Ok(DaemonMessage::Pam { result }) => process::exit(if result { 0 } else { 1 }),
                _ => process::exit(31),
            },
            Err(_) => process::exit(41),
        },
        Command::CleanUp => match send_recv(&socket, ClientMessage::CleanUp) {
            Ok(_) => process::exit(0),
            Err(_) => process::exit(51),
        },
        Command::RateLimit => match send_recv(&socket, ClientMessage::RateLimit) {
            Ok(DaemonMessage::RateLimit { limit }) => {
                println!("{:?}", limit);
                process::exit(0)
            }
            _ => process::exit(61),
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
}
