use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

extern crate clap;
use clap::{Arg, App, SubCommand};
extern crate toml;

extern crate reqwest;
use reqwest::header::Authorization;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

extern crate glob;
use glob::glob;

#[macro_use]
extern crate lazy_static;

use std::ffi::{CStr,CString};
extern crate libc;

#[derive(Deserialize, Debug)]
struct Config {
    token: String,
    org: String,
    team: String,
    #[serde(default="default_endpoint")]
    endpoint: String,
    #[serde(default="default_home")]
    home: String,
    #[serde(default="default_gid")]
    gid: u64,
    #[serde(default="default_sh")]
    sh: String,
    group: Option<String>,
    #[serde(default="default_cache_duration")]
    cache_duration: u64,
    #[serde(default="default_cert_path")]
    cert_path: String
}

fn default_endpoint() -> String { String::from("https://api.github.com") }
fn default_home() -> String { String::from("/home/{}") }
fn default_gid() -> u64 { 2000 }
fn default_sh() -> String { String::from("/bin/bash") }
fn default_cache_duration() -> u64 { 3600 }
fn default_cert_path() -> String { String::from("/etc/ssl/certs/ca-certificates.crt") }

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Team {
    id: u64,
    name: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct Member {
    id: u64,
    login: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct PublicKey {
    id: u64,
    key: String,
}

#[derive(Debug)]
enum CliError {
    Serde(serde_json::Error),
    Reqwest(reqwest::Error),
    Io(std::io::Error),
}

impl From<serde_json::Error> for CliError {fn from(err: serde_json::Error) -> CliError { CliError::Serde(err) }}
impl From<reqwest::Error> for CliError {fn from(err: reqwest::Error) -> CliError { CliError::Reqwest(err) }}
impl From<std::io::Error> for CliError {fn from(err: std::io::Error) -> CliError { CliError::Io(err) }}

lazy_static! {
    static ref CLIENT:GithubClient = create_github_client(
        std::env::var("GHTEAMAUTH_CONFIG")
                 .unwrap_or(String::from("/etc/ghteam-auth.conf"))
                 .as_str()
    ).unwrap();
}

#[allow(dead_code)]
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
                      .subcommand(SubCommand::with_name("pam")
                                             .about("execute pam check"))
                      .subcommand(SubCommand::with_name("passwd")
                                             .about("get passwd"))
                      .subcommand(SubCommand::with_name("shadow")
                                             .about("get shadow"))
                      .subcommand(SubCommand::with_name("group")
                                             .about("get group"))
                      .subcommand(SubCommand::with_name("refresh")
                                             .about("refresh cache"))
                      .get_matches();


    if let Some(matches) = matches.subcommand_matches("key") {
        CLIENT.print_user_public_key(matches.value_of("USER").unwrap()).unwrap();
    } else if let Some(_) = matches.subcommand_matches("passwd") {
        CLIENT.print_passwd().unwrap();
    } else if let Some(_) = matches.subcommand_matches("shadow") {
        CLIENT.print_shadow().unwrap();
    } else if let Some(_) = matches.subcommand_matches("group") {
        CLIENT.print_group().unwrap();
    } else if let Some(_) = matches.subcommand_matches("refresh") {
        CLIENT.clear_all_caches().unwrap();
    } else if let Some(_) = matches.subcommand_matches("pam") {
        match std::env::var("PAM_USER") {
            Ok(user) => {
                if CLIENT.check_pam(&user).unwrap() { std::process::exit(0); }
                else { std::process::exit(1) }
            }
            Err(e) => println!("couldn't interpret PAM_USER: {}", e),
        }
    }

}

fn create_github_client( configpath: &str ) -> Result<GithubClient, CliError> {
    let mut file = File::open(configpath)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let config = toml::from_str::<Config>(contents.as_str()).unwrap();
    match std::env::var("SSL_CERT_FILE") {
        Ok(_) => (),
        Err(_) => std::env::set_var("SSL_CERT_FILE", config.cert_path.clone()),
    }
    let client = reqwest::Client::new()?;
    Ok(GithubClient::new(client, config))
}

struct GithubClient {
    client: reqwest::Client,
    conf: Config
}

impl GithubClient {
    fn new(client:reqwest::Client, conf:Config) -> GithubClient {
        GithubClient {client:client, conf:conf}
    }

    fn load_content_from_cache(&self, url:&String) -> Result<(std::fs::Metadata,String),CliError> {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push(url.as_str());
        let metadata = std::fs::metadata(path.to_str().unwrap())?;
        let mut f = File::open(path.to_str().unwrap())?;
        let mut content = String::new();
        f.read_to_string(&mut content)?;
        Ok((metadata,content))
    }

    fn store_content_to_cache(&self, url:&String, content:&String) -> Result<(),CliError> {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push(url.as_str());
        let mut dirpath = path.clone(); dirpath.pop();
        std::fs::create_dir_all(dirpath)?;
        let mut f = File::create(path.to_str().unwrap())?;
        f.write(content.as_bytes())?;
        Ok(())
    }

    fn get_content_from_url(&self, url:&String) -> Result<String,CliError> {
        let token = String::from("token ") + self.conf.token.clone().as_str();
        let res = self.client.get(url.as_str()).header(Authorization(token)).send();
        let mut content = String::new();
        res?.read_to_string(&mut content)?;
        self.store_content_to_cache(url,&content)?;
        Ok(content)
    }

    fn get_content(&self, url:&String) -> Result<String,CliError> {
        match self.load_content_from_cache(url) {
            Ok((metadata,cache_content)) => {
                match std::time::SystemTime::now().duration_since(metadata.modified()?) {
                    Ok(caching_duration) => {
                        if caching_duration.as_secs() > self.conf.cache_duration {
                            match self.get_content_from_url(url) {
                                Ok(content_from_url) => Ok(content_from_url),
                                Err(_) => Ok(cache_content)
                            }
                        } else {
                            Ok(cache_content)
                        }
                    },
                    Err(_) => {
                        Ok(cache_content)
                    }
                }
            },
            Err(_) => self.get_content_from_url(url)
        }
    }

    fn print_user_public_key(&self, user:&str) -> Result<(), CliError> {
        let keys = self.get_user_public_key(user)?;
        println!("{}", keys);
        Ok(())
    }

    fn get_user_public_key(&self, user:&str) -> Result<String, CliError> {
        let url = format!("{}/users/{}/keys", self.conf.endpoint, user);
        let content = self.get_content(&url);
        let keys = serde_json::from_str::<Vec<PublicKey>>(content?.as_str())?;
        Ok(keys.iter().map(|k|{k.key.clone()}).collect::<Vec<String>>().join("\n"))
    }

    fn check_pam(&self, user:&String) -> Result<bool, CliError> {
        let (_,members) = self.get_team_members()?;
        Ok(members.contains_key(user))
    }

    fn print_passwd(&self) -> Result<(), CliError> {
        let (_,members) = self.get_team_members()?;
        for member in members.values() {
            println!("{}", self.create_passwd_line(&member));
        }
        Ok(())
    }

    fn print_shadow(&self) -> Result<(), CliError> {
        let (_,members) = self.get_team_members()?;
        for member in members.values() {
            println!("{}", self.create_shadow_line(&member));
        }
        Ok(())
    }

    fn print_group(&self) -> Result<(), CliError> {
        let (team,members) = self.get_team_members()?;
        println!("{}", self.create_group_line(&team.name, self.conf.gid, &members));
        Ok(())
    }

    fn create_passwd_line(&self, member:&Member) -> String {
        format!("{login}:x:{uid}:{gid}:user@{org}:{home}:{sh}",
                login=member.login,
                uid=member.id,
                gid=self.conf.gid,
                org=self.conf.org,
                home=self.conf.home.replace("{}",member.login.as_str()),
                sh=self.conf.sh,
                )
    }

    fn create_shadow_line(&self, member:&Member) -> String {
        format!("{login}:!!:::::::", login=member.login )
    }

    fn create_group_line(&self, name:&String, id:u64, members:&HashMap<String,Member>) -> String {
        format!("{name}:x:{id}:{members}", name=name, id=id,
                members=members.values().map(|m|{m.login.clone()}).collect::<Vec<String>>().join(","))
    }

    fn get_team_members(&self) -> Result<(Team,HashMap<String,Member>),CliError> {
        let teams:HashMap<String,Team> = self.get_teams()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            Ok((team.clone(),self.get_members(team.id)?))
        } else {
            Err(CliError::from(std::io::Error::new(std::io::ErrorKind::NotFound, "Team not found")))
        }
    }

    fn get_teams(&self) -> Result<HashMap<String,Team>, CliError> {
        let url = format!("{}/orgs/{}/teams",self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(content.as_str())?;
        let mut team_map = HashMap::new();
        for team in teams { team_map.insert(team.name.clone(), team); }
        Ok(team_map)
    }

    fn get_members(&self, mid:u64) -> Result<HashMap<String,Member>, CliError> {
        let url = format!("{}/teams/{}/members",self.conf.endpoint.clone(), mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(content.as_str())?;
        let mut member_map = HashMap::new();
        for member in members { member_map.insert(member.login.clone(), member); }
        Ok(member_map)
    }

    fn clear_all_caches(&self) -> Result<(), CliError> {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push("**");
        path.push("*");
        for entry in glob(&path.to_str().unwrap()).unwrap() {
            match entry {
                Ok(path) => {if path.is_file() {std::fs::remove_file(path)?}},
                Err(e) => println!("{:?}", e),
            }
        }
        Ok(())
    }

}

#[allow(dead_code)]
enum NssStatus {
    TryAgain,
    Unavail,
    NotFound,
    Success,
}

impl From<NssStatus> for libc::c_int {
    fn from(status:NssStatus) -> libc::c_int {
        match status {
            NssStatus::TryAgain => -2,
            NssStatus::Unavail  => -1,
            NssStatus::NotFound => 0,
            NssStatus::Success  => 1,
        }
    }
}

#[derive(Debug)]
struct Buffer {
    buf: *mut libc::c_char,
    offset: isize,
    buflen: libc::size_t,
}

impl Buffer {
    fn new(buf: *mut libc::c_char,buflen: libc::size_t) -> Self { Self{buf,offset:0,buflen} }

    fn write(&mut self, data: *const libc::c_char, len: usize) -> *mut libc::c_char {
        unsafe {
            let pos = self.buf.offset(self.offset);
            std::ptr::copy(data as *mut i8, pos, len);
            self.offset += len as isize;
            self.buflen -= len as libc::size_t; //TODO: range checking
            pos
        }
    }

    fn write_string(&mut self, s: &String) -> *mut libc::c_char {
        let cs = CString::new(s.clone()).unwrap();
        self.write(cs.as_ptr(), s.len() + 1)
    }
}

#[repr(C)]
pub struct Passwd {
    name:   *mut libc::c_char,
    passwd: *mut libc::c_char,
    uid:    libc::uid_t,
    gid:    libc::gid_t,
    gecos:  *mut libc::c_char,
    dir:    *mut libc::c_char,
    shell:  *mut libc::c_char,
}

impl Passwd {
    fn pack(&mut self,
            buf:&mut Buffer,
            name:&String,
            passwd:&String,
            uid:libc::uid_t,
            gid:libc::gid_t,
            gecos:&String,
            dir:&String,
            shell:&String) {
        self.name = buf.write_string(name);
        self.passwd = buf.write_string(passwd);
        self.dir = buf.write_string(dir);
        self.shell = buf.write_string(shell);
        self.gecos = buf.write_string(gecos);
        self.uid = uid;
        self.gid = gid;
    }

}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwnam_r(cnameptr: *const libc::c_char,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (_,members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            unsafe {
                (*pw).pack(
                    &mut buffer,
                    &member.login,
                    &String::from("x"),
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    &String::new(),
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
                );
            }
            libc::c_int::from(NssStatus::Success)
        },
        None => libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwuid_r(uid: libc::uid_t,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let (_,members) = CLIENT.get_team_members().unwrap();
    for member in members.values() {
        if uid == member.id as libc::uid_t {
            unsafe {
                (*pw).pack(
                    &mut buffer,
                    &member.login,
                    &String::from("x"),
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    &String::new(),
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
                );
            }
            return libc::c_int::from(NssStatus::Success);
        }
    }
    libc::c_int::from(NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setpwent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endpwent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }


#[repr(C)]
pub struct Spwd {
    namp:   *mut libc::c_char,
    pwdp:   *mut libc::c_char,
    lstchg:      libc::c_long,
    min:         libc::c_long,
    max:         libc::c_long,
    warn:        libc::c_long,
    inact:       libc::c_long,
    expire:      libc::c_long,
    flag:        libc::c_ulong,
}

impl Spwd {
    fn pack(&mut self,
            buf:&mut Buffer,
            namp:&String,
            pwdp:&String,
            lstchg:libc::c_long,
            min:libc::c_long,
            max:libc::c_long,
            warn:libc::c_long,
            inact:libc::c_long,
            expire:libc::c_long,
            flag:libc::c_ulong,
        ) {
        self.namp = buf.write_string(namp);
        self.pwdp = buf.write_string(pwdp);
        self.lstchg = lstchg;
        self.min = min;
        self.max = max;
        self.warn = warn;
        self.inact = inact;
        self.expire = expire;
        self.flag = flag;
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspnam_r(cnameptr: *const libc::c_char,
                                         spwd: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (_,members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            unsafe {
                (*spwd).pack(
                    &mut buffer,
                    &member.login,
                    &String::from("!!"),
                    -1,-1,-1,-1,-1,-1,0
                );
            }
            libc::c_int::from(NssStatus::Success)
        },
        None => libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setspent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endspent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }


#[repr(C)]
pub struct Group {
    name:   *mut libc::c_char,
    passwd: *mut libc::c_char,
    gid:         libc::gid_t,
    mem:    *mut *mut libc::c_char,
}

impl Group {
    fn pack(&mut self,
            buf:&mut Buffer,
            name:&String,
            passwd:&String,
            gid:libc::gid_t,
            mem:&Vec<String>){
        self.name = buf.write_string(name);
        self.passwd = buf.write_string(passwd);
        self.gid = gid;
        let mut ptrs = Vec::<*mut libc::c_char>::new();
        for m in mem {
            ptrs.push(buf.write_string(&m));
        }
        unsafe {
            self.mem = buf.buf.offset(buf.offset) as *mut *mut libc::c_char;
        }
        for (i,p) in ptrs.iter().enumerate() {
            unsafe {
                *(self.mem.offset(i as isize)) = *p;
            }
        }
        unsafe {
            *(self.mem.offset(ptrs.len() as isize)) = 0x0 as *mut libc::c_char;
        }
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrgid_r(gid: libc::gid_t,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let (team,members) = CLIENT.get_team_members().unwrap();
    let members:Vec<String> = members.values().map(|m| m.login.clone()).collect::<Vec<String>>();
    if gid == CLIENT.conf.gid as libc::gid_t {
        unsafe {
            (*group).pack(
                &mut buffer,
                &team.name,
                &String::from("x"),
                gid,
                &members
            )
        }
        libc::c_int::from(NssStatus::Success)
    } else {
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrnam_r(cnameptr: *const libc::c_char,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (team,members) = CLIENT.get_team_members().unwrap();
    let members:Vec<String> = members.values().map(|m| m.login.clone()).collect::<Vec<String>>();
    if name == CLIENT.conf.team {
        unsafe {
            (*group).pack(
                &mut buffer,
                &team.name,
                &String::from("x"),
                team.id as libc::gid_t,
                &members
            )
        }
        libc::c_int::from(NssStatus::Success)
    } else {
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setgrent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endgrent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }
