use hyper;
use serde_json;
use std;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::str::FromStr;
use toml;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub token: String,
    pub org: String,
    #[serde(default = "default_team")]
    pub team: Vec<TeamConfig>,
    #[serde(default = "default_repo")]
    pub repo: Vec<RepoConfig>,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_home")]
    pub home: String,
    #[serde(default = "default_sh")]
    pub sh: String,
    #[serde(default = "default_cache_duration")]
    pub cache_duration: u64,
    #[serde(default = "default_cert_path")]
    pub cert_path: String,
    #[serde(default = "default_user_conf_path")]
    pub user_conf_path: String,
    pub proxy_url: Option<String>,
}

fn default_team() -> Vec<TeamConfig> { Vec::new() }
fn default_repo() -> Vec<RepoConfig> { Vec::new() }
fn default_endpoint() -> String { String::from("https://api.github.com") }
fn default_home() -> String { String::from("/home/{}") }
fn default_sh() -> String { String::from("/bin/bash") }
fn default_cache_duration() -> u64 { 3600 }
fn default_cert_path() -> String { String::from("/etc/ssl/certs/ca-certificates.crt") }
fn default_user_conf_path() -> String { String::from(".config/sectora.toml") }

impl Config {
    pub fn new(configpath: &std::path::Path) -> Result<Self, CliError> {
        let mut file = File::open(configpath)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(toml::from_str::<Config>(&contents)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UserConfig {
    pub sh: Option<String>,
    pub pass: Option<String>,
}

impl UserConfig {
    #[allow(dead_code)]
    pub fn new(configpath: &std::path::Path) -> Result<Self, CliError> {
        let mut file = File::open(configpath)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(toml::from_str::<UserConfig>(&contents)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Team {
    pub id: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TeamConfig {
    pub name: String,
    pub gid: Option<u64>,
    pub group: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Repo {
    pub id: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RepoConfig {
    pub name: String,
    pub gid: Option<u64>,
    pub group: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SectorType {
    Team,
    Repo,
}

impl fmt::Display for SectorType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &SectorType::Team => write!(f, "T"),
            &SectorType::Repo => write!(f, "R"),
        }
    }
}

impl FromStr for SectorType {
    type Err = std::io::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "T" => Ok(SectorType::Team),
            "R" => Ok(SectorType::Repo),
            _ => Err(std::io::Error::new(std::io::ErrorKind::Other, "unknown sector type")),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Sector {
    pub id: u64,
    pub name: String,
    pub sector_type: SectorType,
}

impl From<Team> for Sector {
    fn from(team: Team) -> Self {
        Self { id: team.id,
               name: team.name,
               sector_type: SectorType::Team, }
    }
}

impl From<Repo> for Sector {
    fn from(repo: Repo) -> Self {
        Self { id: repo.id,
               name: repo.name,
               sector_type: SectorType::Repo, }
    }
}

impl fmt::Display for Sector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}:{}:{}", self.id, self.name, self.sector_type) }
}

pub enum ParseSectorError {
    Id(std::num::ParseIntError),
    Type(std::io::Error),
}

impl FromStr for Sector {
    type Err = ParseSectorError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(":").collect::<Vec<&str>>();
        Ok(Self { id: parts[0].parse().map_err(|e| ParseSectorError::Id(e))?,
                  name: String::from(parts[1]),
                  sector_type: parts[2].parse().map_err(|e| ParseSectorError::Type(e))?, })
    }
}

#[derive(Debug, Clone)]
pub struct SectorGroup {
    pub sector: Sector,
    pub gid: Option<u64>,
    pub group: Option<String>,
    pub members: HashMap<String, Member>,
}

impl SectorGroup {
    #[allow(dead_code)]
    pub fn get_gid(&self) -> u64 { self.gid.unwrap_or(self.sector.id) }
    #[allow(dead_code)]
    pub fn get_group(&self) -> String { self.group.clone().unwrap_or(self.sector.name.clone()) }
}

impl fmt::Display for SectorGroup {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let members_str = self.members.values()
                              .map(|v| v.to_string())
                              .collect::<Vec<_>>()
                              .join(" ");
        write!(f,
               "{}\t{}\t{}\t{}",
               self.sector,
               self.gid.and_then(|i| Some(i.to_string())).unwrap_or(String::new()),
               self.group.clone().unwrap_or(String::new()),
               members_str)
    }
}

pub enum ParseSectorGroupError {
    Sector(ParseSectorError),
    Gid(std::num::ParseIntError),
    Member(std::num::ParseIntError),
}

impl FromStr for SectorGroup {
    type Err = ParseSectorGroupError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split("\t").collect::<Vec<&str>>();
        let sector = parts[0].parse().map_err(|e| ParseSectorGroupError::Sector(e))?;
        let gid: Option<u64> = match parts[1] {
            "" => None,
            s => Some(s.parse::<u64>().map_err(|e| ParseSectorGroupError::Gid(e))?),
        };
        let group: Option<String> = match parts[2] {
            "" => None,
            s => Some(String::from(s)),
        };
        let members = parts[3].split(" ")
                              .map(|s| s.parse::<Member>().map_err(|e| ParseSectorGroupError::Member(e)))
                              .collect::<Result<Vec<Member>, _>>()?
                              .into_iter()
                              .map(|m| (m.login.clone(), m))
                              .collect::<HashMap<_, _>>();
        Ok(Self { sector,
                  gid,
                  group,
                  members, })
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Member {
    pub id: u64,
    pub login: String,
}

impl fmt::Display for Member {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}:{}", self.id, self.login) }
}

impl FromStr for Member {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split(":").collect::<Vec<&str>>();
        Ok(Self { id: parts[0].parse()?,
                  login: String::from(parts[1]), })
    }
}

#[allow(dead_code)]
pub struct MemberGid {
    pub member: Member,
    pub gid: u64,
}

impl fmt::Display for MemberGid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { write!(f, "{}|{}", self.member, self.gid) }
}

impl FromStr for MemberGid {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts = s.split("|").collect::<Vec<&str>>();
        Ok(Self { member: parts[0].parse()?,
                  gid: parts[1].parse()?, })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicKey {
    pub id: u64,
    pub key: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Rate {
    pub limit: usize,
    pub remaining: usize,
    pub reset: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RateLimit {
    pub rate: Rate,
}

#[derive(Debug)]
pub enum CliError {
    Serde(serde_json::Error),
    Io(std::io::Error),
    Toml(toml::de::Error),
    Hyper(hyper::Error),
}

impl From<serde_json::Error> for CliError {
    fn from(err: serde_json::Error) -> CliError { CliError::Serde(err) }
}
impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> CliError { CliError::Io(err) }
}
impl From<toml::de::Error> for CliError {
    fn from(err: toml::de::Error) -> CliError { CliError::Toml(err) }
}
impl From<hyper::Error> for CliError {
    fn from(err: hyper::Error) -> CliError { CliError::Hyper(err) }
}
