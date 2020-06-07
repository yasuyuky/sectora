extern crate futures;
extern crate glob;
extern crate hyper;
extern crate hyper_tls;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate log;
#[macro_use]
extern crate serde;
extern crate sd_notify;
extern crate serde_json;
extern crate structopt;
extern crate syslog;
extern crate tokio;
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
use std::path::Path;
use structs::{Config, SocketConfig, UserConfig};

#[tokio::main]
async fn main() {
    applog::init(Some("sectorad"));
    let mut d = Daemon::new();
    d.run().await.expect("run");
    log::debug!("Run stopped");
}

struct Daemon {
    client: GithubClient,
    socket_conf: SocketConfig,
    msg_cache: HashMap<u32, VecDeque<DaemonMessage>>,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        log::debug!("Drop daemon");
        fs::remove_file(&self.socket_conf.socket_path).expect("remove socket");
    }
}

impl Daemon {
    fn new() -> Self {
        let config = Config::from_path(&(*CONF_PATH)).expect("valid config");
        let socket_conf = SocketConfig::new();
        fs::create_dir_all(&socket_conf.socket_dir).expect("create socket dir");
        fs::set_permissions(&socket_conf.socket_dir, unix::fs::PermissionsExt::from_mode(0o777)).unwrap_or_default();
        let client = GithubClient::new(&config);
        log::debug!("Initialised");
        Daemon { client,
                 socket_conf,
                 msg_cache: HashMap::new() }
    }

    async fn run(&mut self) -> Result<(), Error> {
        let rl = self.client.get_rate_limit().await.expect("get rate limit");
        log::info!("Rate Limit: {:?}", rl);
        let sectors = self.client.get_sectors().await.expect("get sectors");
        log::info!("{} sector[s] loaded", sectors.len());
        let socket = unix::net::UnixDatagram::bind(&self.socket_conf.socket_path)?;
        fs::set_permissions(&self.socket_conf.socket_path,
                            unix::fs::PermissionsExt::from_mode(0o666)).unwrap_or_default();
        let _ = sd_notify::notify(true, &[sd_notify::NotifyState::Ready]);
        log::info!("Start running @ {}", &self.socket_conf.socket_path);
        loop {
            let mut buf = [0u8; 4096];
            let (recv_cnt, src) = socket.recv_from(&mut buf)?;
            let msgstr = String::from_utf8(buf[..recv_cnt].to_vec()).expect("decode msg str");
            log::debug!("recv: {}, src:{:?}", msgstr, src);
            let response = self.handle(&msgstr.parse::<ClientMessage>().expect("parse ClientMessage"))
                               .await;
            log::debug!("-> response: {}", response);
            match socket.send_to(&response.to_string().as_bytes(), src.as_pathname().expect("src")) {
                Ok(sendsize) => log::debug!("send: {}", sendsize),
                Err(err) => log::warn!("failed to send back to the client {:?}:{}", src, err),
            }
        }
    }

    async fn handle(&mut self, msg: &ClientMessage) -> DaemonMessage {
        match msg {
            ClientMessage::Key { user } => match self.client.get_user_public_key(&user).await {
                Ok(keys) => DaemonMessage::Key { keys },
                Err(_) => DaemonMessage::Error { message: String::from("get key failed") },
            },
            ClientMessage::Pam { user } => match self.client.check_pam(&user).await {
                Ok(result) => DaemonMessage::Pam { result },
                Err(_) => DaemonMessage::Error { message: String::from("check pam failed") },
            },
            ClientMessage::CleanUp => match self.client.clear_all_caches().await {
                Ok(_) => DaemonMessage::Success,
                Err(_) => DaemonMessage::Error { message: String::from("clean up failed") },
            },
            ClientMessage::RateLimit => match self.client.get_rate_limit().await {
                Ok(rl) => DaemonMessage::RateLimit { limit: rl.rate.limit,
                                                     remaining: rl.rate.remaining,
                                                     reset: rl.rate.reset },
                Err(_) => DaemonMessage::Error { message: String::from("clean up failed") },
            },
            ClientMessage::SectorGroups => match self.client.get_sectors().await {
                Ok(sectors) => DaemonMessage::SectorGroups { sectors },
                Err(_) => DaemonMessage::Error { message: String::from("get sectors failed") },
            },
            ClientMessage::Pw(pw) => self.handle_pw(pw).await,
            ClientMessage::Sp(sp) => self.handle_sp(sp).await,
            ClientMessage::Gr(gr) => self.handle_gr(gr).await,
        }
    }

    fn get_msg(&mut self, pid: u32) -> DaemonMessage {
        match self.msg_cache.entry(pid) {
            Entry::Occupied(mut o) => match o.get_mut().pop_front() {
                Some(msg) => msg,
                None => DaemonMessage::Error { message: String::from("not found") },
            },
            Entry::Vacant(_) => DaemonMessage::Error { message: String::from("not found") },
        }
    }

    fn clear_cache(&mut self, pid: u32) -> DaemonMessage {
        self.msg_cache.remove(&pid).unwrap_or_default();
        DaemonMessage::Success
    }

    fn get_home_sh(&self, login: &str) -> (String, String) {
        let conf = &self.client.conf;
        let home = conf.home.replace("{}", login);
        let sh: String = match UserConfig::from_path(&Path::new(&home).join(&conf.user_conf_path)) {
            Ok(personal) => match personal.sh {
                Some(sh) => {
                    if Path::new(&sh).exists() {
                        sh
                    } else {
                        conf.sh.clone()
                    }
                }
                None => conf.sh.clone(),
            },
            Err(_) => conf.sh.clone(),
        };
        (home, sh)
    }

    fn get_pass(&self, login: &str) -> String {
        let home = self.client.conf.home.replace("{}", login);
        let pass: String = match UserConfig::from_path(&Path::new(&home).join(&self.client.conf.user_conf_path)) {
            Ok(personal) => match personal.pass {
                Some(pass) => pass,
                None => String::from("*"),
            },
            Err(_) => String::from("*"),
        };
        pass
    }

    async fn handle_pw(&mut self, pw: &Pw) -> DaemonMessage {
        match pw {
            Pw::Uid(uid) => {
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    for member in sector.members.values() {
                        if uid == &member.id {
                            let (home, sh) = self.get_home_sh(&member.login);
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: *uid,
                                                       gid: sector.get_gid(),
                                                       home,
                                                       sh };
                        }
                    }
                }
            }
            Pw::Nam(name) => {
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    for member in sector.members.values() {
                        if name == &member.login {
                            let (home, sh) = self.get_home_sh(&member.login);
                            return DaemonMessage::Pw { login: member.login.clone(),
                                                       uid: member.id,
                                                       gid: sector.get_gid(),
                                                       home,
                                                       sh };
                        }
                    }
                }
            }
            Pw::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    for member in sector.members.values() {
                        let (home, sh) = self.get_home_sh(&member.login);
                        let pw = DaemonMessage::Pw { login: member.login.clone(),
                                                     uid: member.id,
                                                     gid: sector.get_gid(),
                                                     home,
                                                     sh };
                        ents.push_back(pw);
                    }
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Pw::Ent(Ent::Get(pid)) => return self.get_msg(*pid),
            Pw::Ent(Ent::End(pid)) => return self.clear_cache(*pid),
        }
        DaemonMessage::Error { message: String::from("not found") }
    }

    async fn handle_sp(&mut self, sp: &Sp) -> DaemonMessage {
        match sp {
            Sp::Nam(name) => {
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    if let Some(member) = sector.members.get(name) {
                        let pass = self.get_pass(name);
                        return DaemonMessage::Sp { login: member.login.clone(),
                                                   pass };
                    }
                }
            }
            Sp::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    for member in sector.members.values() {
                        let pass = self.get_pass(&member.login);
                        let sp = DaemonMessage::Sp { login: member.login.clone(),
                                                     pass };
                        ents.push_back(sp);
                    }
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Sp::Ent(Ent::Get(pid)) => return self.get_msg(*pid),
            Sp::Ent(Ent::End(pid)) => return self.clear_cache(*pid),
        }
        DaemonMessage::Error { message: String::from("not found") }
    }

    async fn handle_gr(&mut self, gr: &Gr) -> DaemonMessage {
        match gr {
            Gr::Gid(gid) => {
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    if gid == &sector.get_gid() {
                        return DaemonMessage::Gr { sector };
                    }
                }
            }
            Gr::Nam(name) => {
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    if name == &sector.get_group() {
                        return DaemonMessage::Gr { sector };
                    }
                }
            }
            Gr::Ent(Ent::Set(pid)) => {
                let mut ents = VecDeque::new();
                for sector in self.client.get_sectors().await.unwrap_or_default() {
                    ents.push_back(DaemonMessage::Gr { sector });
                }
                self.msg_cache.insert(*pid, ents).unwrap_or_default();
                return DaemonMessage::Success;
            }
            Gr::Ent(Ent::Get(pid)) => return self.get_msg(*pid),
            Gr::Ent(Ent::End(pid)) => return self.clear_cache(*pid),
        }
        DaemonMessage::Error { message: String::from("not found") }
    }
}
