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
mod ghclient;
mod message;
mod statics;
mod structs;

use error::Error;
use ghclient::GithubClient;
use message::*;
use statics::CONF_PATH;
use std::os::unix::net::UnixDatagram;
use structopt::StructOpt;
use structs::Config;

#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct Opt {
    #[structopt(short = "s", long = "socket")]
    socket_path: Option<String>,
}

fn main() {
    let opt = Opt::from_args();
    applog::init(Some("sectorad"));
    let d = Daemon::new(opt.socket_path);
    d.run().unwrap();
    log::debug!("Run stopped");
}

struct Daemon {
    client: GithubClient,
    socket_path: String,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        log::debug!("Drop daemon");
        std::fs::remove_file(&self.socket_path).expect("remove socket");
    }
}

impl Daemon {
    fn new(socket_path: Option<String>) -> Self {
        let config = Config::from_path(&(*CONF_PATH)).unwrap();
        std::fs::create_dir_all(&config.socket_dir).expect("create socket dir");
        std::fs::set_permissions(&config.socket_dir, std::os::unix::fs::PermissionsExt::from_mode(0o777)).unwrap_or_default();
        let client = GithubClient::new(&config);
        log::debug!("Initialised");
        Daemon { client,
                 socket_path: socket_path.unwrap_or(config.socket_path) }
    }

    fn run(&self) -> Result<(), Error> {
        let socket = UnixDatagram::bind(&self.socket_path)?;
        std::fs::set_permissions(&self.socket_path, std::os::unix::fs::PermissionsExt::from_mode(0o666)).unwrap_or_default();
        log::debug!("Start running @ {}", &self.socket_path);
        loop {
            let mut buf = [0u8; 4096];
            let (recv_cnt, src) = socket.recv_from(&mut buf)?;
            let msgstr = String::from_utf8(buf[..recv_cnt].to_vec()).expect("decode msg str");
            log::debug!("recv: {}, src:{:?}", msgstr, src);
            let response = self.handle(&msgstr.parse::<ClientMessage>().unwrap());
            log::debug!("-> response: {}", response);
            socket.send_to(&response.to_string().as_bytes(), src.as_pathname().expect("src"))?;
        }
    }

    fn handle(&self, msg: &ClientMessage) -> DaemonMessage {
        match msg {
            ClientMessage::Key { user } => match self.client.get_user_public_key(&user) {
                Ok(keys) => DaemonMessage::Key { keys },
                Err(_) => DaemonMessage::Error { message: String::from("get key failed") },
            },
            ClientMessage::Pam { user } => match self.client.check_pam(&user) {
                Ok(result) => DaemonMessage::Pam { result },
                Err(_) => DaemonMessage::Error { message: String::from("check pam failed") },
            },
            ClientMessage::CleanUp => match self.client.clear_all_caches() {
                Ok(_) => DaemonMessage::CleanUp,
                Err(_) => DaemonMessage::Error { message: String::from("clean up failed") },
            },
            ClientMessage::RateLimit => match self.client.get_rate_limit() {
                Ok(ratelimit) => DaemonMessage::RateLimit { limit: ratelimit.rate.limit },
                Err(_) => DaemonMessage::Error { message: String::from("clean up failed") },
            },
            ClientMessage::SectorGroups => match self.client.get_sectors() {
                Ok(sectors) => DaemonMessage::SectorGroups { sectors },
                Err(_) => DaemonMessage::Error { message: String::from("get sectors failed") },
            },
            ClientMessage::Pw(pw) => match self.client.get_sectors() {
                Ok(sectors) => self.handle_pw(pw, &sectors),
                Err(_) => DaemonMessage::Error { message: String::from("get sectors failed") },
            },
            ClientMessage::Gr(gr) => match self.client.get_sectors() {
                Ok(sectors) => self.handle_gr(gr, &sectors),
                Err(_) => DaemonMessage::Error { message: String::from("get sectors failed") },
            },
        }
    }

    fn handle_pw(&self, pw: &Pw, sectors: &Vec<structs::SectorGroup>) -> DaemonMessage {
        for sector in sectors {
            for member in sector.members.values() {
                match pw {
                    Pw::Uid(uid) => {
                        if uid == &member.id {
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: *uid,
                                                       gid: sector.get_gid() };
                        }
                    }
                    Pw::Nam(name) => {
                        if name == &member.login {
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: member.id,
                                                       gid: sector.get_gid() };
                        }
                    }
                }
            }
        }
        DaemonMessage::Error { message: String::from("not found") }
    }

    fn handle_gr(&self, gr: &Gr, sectors: &Vec<structs::SectorGroup>) -> DaemonMessage {
        for sector in sectors {
            match gr {
                Gr::Gid(gid) => {
                    if gid == &sector.get_gid() {
                        return DaemonMessage::Gr { sector: sector.clone() };
                    }
                }
                Gr::Nam(name) => {
                    if name == &sector.get_group() {
                        return DaemonMessage::Gr { sector: sector.clone() };
                    }
                }
            }
        }
        DaemonMessage::Error { message: String::from("not found") }
    }
}
