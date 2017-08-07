use structs::{CliError, Config, UserConfig, Member, Team, TeamGroup, PublicKey};
use std;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;
use std::collections::HashMap;

use glob::glob;
use reqwest;
use reqwest::header::Authorization;
use serde_json;

pub struct GithubClient {
    client: reqwest::Client,
    conf: Config,
}

impl GithubClient {
    pub fn new(config: &Config) -> Result<GithubClient, CliError> {
        if std::env::var("SSL_CERT_FILE").is_err() {
            std::env::set_var("SSL_CERT_FILE", &config.cert_path);
        }
        let client = reqwest::Client::new()?;
        Ok(GithubClient { client: client, conf: config.clone() })
    }

    fn get_cache_path(url: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push(url);
        path
    }

    fn load_content_from_cache(&self, url: &str) -> Result<(std::fs::Metadata, String), CliError> {
        let path = Self::get_cache_path(url);
        let metadata = std::fs::metadata(path.to_str().unwrap())?;
        let mut f = File::open(path.to_str().unwrap())?;
        let mut content = String::new();
        f.read_to_string(&mut content)?;
        Ok((metadata, content))
    }

    fn store_content_to_cache(&self, url: &str, content: &str) -> Result<(), CliError> {
        let path = Self::get_cache_path(url);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut f = File::create(path.to_str().unwrap())?;
        f.write(content.as_bytes())?;
        Ok(())
    }

    fn get_content_from_url(&self, url: &str) -> Result<String, CliError> {
        let token = String::from("token ") + &self.conf.token;
        let res = self.client.get(url).header(Authorization(token)).send();
        let mut content = String::new();
        res?.read_to_string(&mut content)?;
        self.store_content_to_cache(url, &content)?;
        Ok(content)
    }

    fn get_content(&self, url: &str) -> Result<String, CliError> {
        match self.load_content_from_cache(url) {
            Ok((metadata, cache_content)) => {
                match std::time::SystemTime::now().duration_since(metadata.modified()?) {
                    Ok(caching_duration) => {
                        if caching_duration.as_secs() > self.conf.cache_duration {
                            match self.get_content_from_url(url) {
                                Ok(content_from_url) => Ok(content_from_url),
                                Err(_) => Ok(cache_content),
                            }
                        } else {
                            Ok(cache_content)
                        }
                    }
                    Err(_) => Ok(cache_content),
                }
            }
            Err(_) => self.get_content_from_url(url),
        }
    }

    #[allow(dead_code)]
    pub fn print_user_public_key(&self, user: &str) -> Result<(), CliError> {
        let keys = self.get_user_public_key(user)?;
        println!("{}", keys);
        Ok(())
    }

    fn get_user_public_key(&self, user: &str) -> Result<String, CliError> {
        let url = format!("{}/users/{}/keys", self.conf.endpoint, user);
        let content = self.get_content(&url)?;
        let keys = serde_json::from_str::<Vec<PublicKey>>(&content)?;
        Ok(keys.iter().map(|k| k.key.clone()).collect::<Vec<String>>().join("\n"))
    }

    #[allow(dead_code)]
    pub fn check_pam(&self, user: &str) -> Result<bool, CliError> {
        let team = self.get_team()?;
        Ok(team.members.contains_key(user))
    }

    pub fn get_team(&self) -> Result<TeamGroup, CliError> {
        let teams: HashMap<String, TeamGroup> = self.get_team_map()?;
        if let Some(team) = teams.get(&self.conf.team.clone()) {
            Ok(team.clone())
        } else {
            Err(CliError::from(std::io::Error::new(std::io::ErrorKind::NotFound, "Team not found")))
        }
    }

    fn get_team_map(&self) -> Result<HashMap<String, TeamGroup>, CliError> {
        let url = format!("{}/orgs/{}/teams", self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(&content)?;
        let mut team_map = HashMap::new();
        for team in teams {
            team_map.insert(team.name.clone(),
                            TeamGroup { team: team.clone(),
                                        gid: self.conf.gid.clone(),
                                        group: self.conf.group.clone(),
                                        members: self.get_members(team.id)?, });
        }
        Ok(team_map)
    }

    fn get_members(&self, mid: u64) -> Result<HashMap<String, Member>, CliError> {
        let url = format!("{}/teams/{}/members", self.conf.endpoint, mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&content)?;
        let mut member_map = HashMap::new();
        for member in members {
            member_map.insert(member.login.clone(), member);
        }
        Ok(member_map)
    }

    fn get_user_conf(&self, user: &str) -> Result<UserConfig, CliError> {
        let home = self.conf.home.replace("{}", user);
        UserConfig::new(&Path::new(&home).join(&self.conf.user_conf_path))
    }

    #[allow(dead_code)]
    pub fn clear_all_caches(&self) -> Result<(), CliError> {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push("**");
        path.push("*");
        for entry in glob(&path.to_str().unwrap()).unwrap() {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        std::fs::remove_file(path)?
                    }
                }
                Err(e) => println!("{:?}", e),
            }
        }
        Ok(())
    }
}
