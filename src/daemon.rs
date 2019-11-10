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
use std::collections::hash_map::Entry;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::os::unix;
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
    let mut d = Daemon::new(opt.socket_path);
    d.run().unwrap();
    log::debug!("Run stopped");
}

struct Daemon {
    client: GithubClient,
    socket_path: String,
    msg_cache: HashMap<u32, VecDeque<DaemonMessage>>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        log::debug!("Drop daemon");
        fs::remove_file(&self.socket_path).expect("remove socket");
    }
}

impl Daemon {
    fn new(socket_path: Option<String>) -> Self {
        let config = Config::from_path(&(*CONF_PATH)).unwrap();
        fs::create_dir_all(&config.socket_dir).expect("create socket dir");
        fs::set_permissions(&config.socket_dir, unix::fs::PermissionsExt::from_mode(0o777)).unwrap_or_default();
        let client = GithubClient::new(&config);
        log::debug!("Initialised");
        Daemon { client,
                 socket_path: socket_path.unwrap_or(config.socket_path),
                 msg_cache: HashMap::new() }
    }

    fn run(&mut self) -> Result<(), Error> {
        let socket = unix::net::UnixDatagram::bind(&self.socket_path)?;
        fs::set_permissions(&self.socket_path, unix::fs::PermissionsExt::from_mode(0o666)).unwrap_or_default();
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

    fn handle(&mut self, msg: &ClientMessage) -> DaemonMessage {
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
                Ok(_) => DaemonMessage::Success,
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
            ClientMessage::Pw(pw) => self.handle_pw(pw),
            ClientMessage::Sp(sp) => self.handle_sp(sp),
            ClientMessage::Gr(gr) => self.handle_gr(gr),
        }
    }

    fn handle_pw(&mut self, pw: &Pw) -> DaemonMessage {
        match pw {
            Pw::Uid(uid) => {
                for sector in self.client.get_sectors().unwrap_or_default() {
                    for member in sector.members.values() {
                        if uid == &member.id {
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: *uid,
                                                       gid: sector.get_gid() };
                        }
                    }
                }
            }
            Pw::Nam(name) => {
                for sector in self.client.get_sectors().unwrap_or_default() {
                    for member in sector.members.values() {
                        if name == &member.login {
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: member.id,
                                                       gid: sector.get_gid() };
                        }
                    }
                }
            }
            Pw::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().unwrap_or_default() {
                    for member in sector.members.values() {
                        let pw = DaemonMessage::Pw { login: member.login.clone(),
                                                     uid: member.id,
                                                     gid: sector.get_gid() };
                        ents.push_back(pw);
                    }
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Pw::Ent(Ent::Get(pid)) => match self.msg_cache.entry(*pid) {
                Entry::Occupied(mut o) => match o.get_mut().pop_front() {
                    Some(msg) => return msg,
                    None => return DaemonMessage::Error { message: String::from("not found") },
                },
                Entry::Vacant(_) => {}
            },
            Pw::Ent(Ent::End(pid)) => {
                self.msg_cache.remove(pid).unwrap_or_default();
                return DaemonMessage::Success;
            }
        }
        DaemonMessage::Error { message: String::from("not found") }
    }

    fn handle_sp(&mut self, sp: &Sp) -> DaemonMessage {
        match sp {
            Sp::Nam(name) => {
                for sector in self.client.get_sectors().unwrap_or_default() {
                    if let Some(member) = sector.members.get(name) {
                        return DaemonMessage::Sp { login: member.login.clone() };
                    }
                }
            }
            Sp::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().unwrap_or_default() {
                    for member in sector.members.values() {
                        let sp = DaemonMessage::Sp { login: member.login.clone() };
                        ents.push_back(sp);
                    }
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Sp::Ent(Ent::Get(pid)) => match self.msg_cache.entry(*pid) {
                Entry::Occupied(mut o) => match o.get_mut().pop_front() {
                    Some(msg) => return msg,
                    None => return DaemonMessage::Error { message: String::from("not found") },
                },
                Entry::Vacant(_) => {}
            },
            Sp::Ent(Ent::End(pid)) => {
                self.msg_cache.remove(pid).unwrap_or_default();
                return DaemonMessage::Success;
            }
        }
        DaemonMessage::Error { message: String::from("not found") }
    }

    fn handle_gr(&mut self, gr: &Gr) -> DaemonMessage {
        match gr {
            Gr::Gid(gid) => {
                for sector in self.client.get_sectors().unwrap_or_default() {
                    if gid == &sector.get_gid() {
                        return DaemonMessage::Gr { sector: sector.clone() };
                    }
                }
            }
            Gr::Nam(name) => {
                for sector in self.client.get_sectors().unwrap_or_default() {
                    if name == &sector.get_group() {
                        return DaemonMessage::Gr { sector: sector.clone() };
                    }
                }
            }
            Gr::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().unwrap_or_default() {
                    ents.push_back(DaemonMessage::Gr { sector: sector.clone() });
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Gr::Ent(Ent::Get(pid)) => match self.msg_cache.entry(*pid) {
                Entry::Occupied(mut o) => match o.get_mut().pop_front() {
                    Some(msg) => return msg,
                    None => return DaemonMessage::Error { message: String::from("not found") },
                },
                Entry::Vacant(_) => {}
            },
            Gr::Ent(Ent::End(pid)) => {
                self.msg_cache.remove(pid).unwrap_or_default();
                return DaemonMessage::Success;
            }
        }
        DaemonMessage::Error { message: String::from("not found") }
    }
}
