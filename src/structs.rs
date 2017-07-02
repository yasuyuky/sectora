use serde_json;
use reqwest;
use std;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub token: String,
    pub org: String,
    pub team: String,
    #[serde(default = "default_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_home")]
    pub home: String,
    #[serde(default = "default_gid")]
    pub gid: u64,
    #[serde(default = "default_sh")]
    pub sh: String,
    pub group: Option<String>,
    #[serde(default = "default_cache_duration")]
    pub cache_duration: u64,
    #[serde(default = "default_cert_path")]
    pub cert_path: String,
}

fn default_endpoint() -> String { String::from("https://api.github.com") }
fn default_home() -> String { String::from("/home/{}") }
fn default_gid() -> u64 { 2000 }
fn default_sh() -> String { String::from("/bin/bash") }
fn default_cache_duration() -> u64 { 3600 }
fn default_cert_path() -> String { String::from("/etc/ssl/certs/ca-certificates.crt") }

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Team {
    pub id: u64,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Member {
    pub id: u64,
    pub login: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PublicKey {
    pub id: u64,
    pub key: String,
}

#[derive(Debug)]
pub enum CliError {
    Serde(serde_json::Error),
    Reqwest(reqwest::Error),
    Io(std::io::Error),
}

impl From<serde_json::Error> for CliError {
    fn from(err: serde_json::Error) -> CliError { CliError::Serde(err) }
}
impl From<reqwest::Error> for CliError {
    fn from(err: reqwest::Error) -> CliError { CliError::Reqwest(err) }
}
impl From<std::io::Error> for CliError {
    fn from(err: std::io::Error) -> CliError { CliError::Io(err) }
}
