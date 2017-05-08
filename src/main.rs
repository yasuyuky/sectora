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

#[derive(Serialize, Deserialize, Debug)]
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

fn main() {

    let matches = App::new("ghteam-auth")
                      .version("0.1")
                      .author("Yasuyuki YAMADA <yasuyuki.ymd@gmail.com>")
                      .about("")
                      .arg(Arg::with_name("config")
                               .short("c")
                               .long("config")
                               .value_name("FILE")
                               .help("Sets a custom config file (toml)")
                               .takes_value(true))
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
                                             .about("get user public key")
                                             .arg(Arg::with_name("USER")
                                                      .required(false)
                                                      .index(1)
                                                      .help("user name")))
                      .subcommand(SubCommand::with_name("passwd")
                                             .about("get passwd"))
                      .subcommand(SubCommand::with_name("shadow")
                                             .about("get shadow"))
                      .subcommand(SubCommand::with_name("group")
                                             .about("get group"))
                      .subcommand(SubCommand::with_name("refresh")
                                             .about("refresh cache"))
                      .get_matches();

    let config = matches.value_of("config").unwrap_or("/etc/ghteam-auth.conf");
    let mut file = File::open(config).unwrap();
    let mut contents = String::new();
    file.read_to_string(&mut contents).unwrap();
    let config = toml::from_str::<Config>(contents.as_str()).unwrap();

    match std::env::var("SSL_CERT_FILE") {
        Ok(_) => (),
        Err(_) => std::env::set_var("SSL_CERT_FILE", config.cert_path.clone()),
    }

    let client = reqwest::Client::new().unwrap();
    let client = GithubClient::new(client, config);

    if let Some(matches) = matches.subcommand_matches("key") {
        client.print_user_public_key(matches.value_of("USER").unwrap()).unwrap();
    } else if let Some(_) = matches.subcommand_matches("passwd") {
        client.get_passwd().unwrap();
    } else if let Some(_) = matches.subcommand_matches("shadow") {
        client.get_shadow().unwrap();
    } else if let Some(_) = matches.subcommand_matches("group") {
        client.get_group().unwrap();
    } else if let Some(_) = matches.subcommand_matches("refresh") {
        client.clear_all_caches().unwrap();
    } else if let Some(_) = matches.subcommand_matches("pam") {
        match std::env::var("PAM_USER") {
            Ok(user) => {
                if client.check_pam(&user).unwrap() { std::process::exit(0); }
                else { std::process::exit(1) }
            }
            Err(e) => println!("couldn't interpret PAM_USER: {}", e),
        }
    }

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
        let teams:HashMap<String,Team> = self.get_teams()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            for member in self.get_members(team.id)? {
                if member.login==*user { return Ok(true) }
            }
        }
        Ok(false)
    }

    fn get_passwd(&self) -> Result<(), CliError> {
        let teams:HashMap<String,Team> = self.get_teams()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            for member in self.get_members(team.id)? {
                println!("{}", self.create_passwd_line(&member));
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    fn get_shadow(&self) -> Result<(), CliError> {
        let teams:HashMap<String,Team> = self.get_teams()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            for member in self.get_members(team.id)? {
                println!("{}", self.create_shadow_line(&member));
            }
            Ok(())
        } else {
            Ok(())
        }
    }

    fn get_group(&self) -> Result<(), CliError> {
        let teams:HashMap<String,Team> = self.get_teams()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            let members = self.get_members(team.id)?;
            println!("{}", self.create_group_line(&team.name, self.conf.gid, &members));
            Ok(())
        } else {
            Ok(())
        }
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

    fn create_group_line(&self, name:&String, id:u64, members:&Vec<Member>) -> String {
        format!("{name}:x:{id}:{members}", name=name, id=id,
                members=members.iter().map(|m|{m.login.clone()}).collect::<Vec<String>>().join(","))
    }

    fn get_teams(&self) -> Result<HashMap<String,Team>, CliError> {
        let url = format!("{}/orgs/{}/teams",self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(content.as_str())?;
        let mut team_map = HashMap::new();
        for team in teams { team_map.insert(team.name.clone(), team); }
        Ok(team_map)
    }

    fn get_members(&self, mid:u64) -> Result<Vec<Member>, CliError> {
        let url = format!("{}/teams/{}/members",self.conf.endpoint.clone(), mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(content.as_str())?;
        Ok(members)
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
