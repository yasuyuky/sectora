use glob::glob;
use reqwest;
use reqwest::header::Authorization;
use serde_json;
use std;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use structs::{CliError, Config, Member, PublicKey, Sector, SectorGroup, Team};

pub struct GithubClient {
    client: reqwest::Client,
    conf: Config,
}

impl GithubClient {
    pub fn new(config: &Config) -> GithubClient {
        if std::env::var("SSL_CERT_FILE").is_err() {
            std::env::set_var("SSL_CERT_FILE", &config.cert_path);
        }
        GithubClient { client: match config.proxy_url {
                           Some(ref proxy_url) => {
                               let p = reqwest::Proxy::all(proxy_url).unwrap();
                               reqwest::Client::builder().proxy(p).build().unwrap()
                           }
                           None => reqwest::Client::new(),
                       },
                       conf: config.clone(), }
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
            Ok((metadata, cache_content)) => match std::time::SystemTime::now().duration_since(metadata.modified()?) {
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
            },
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
        Ok(keys.iter().map(|k| k.key.clone())
               .collect::<Vec<String>>()
               .join("\n"))
    }

    #[allow(dead_code)]
    pub fn check_pam(&self, user: &str) -> Result<bool, CliError> {
        let teams = self.get_teams();
        Ok(teams.iter().any(|team| team.members.contains_key(user)))
    }

    pub fn get_teams(&self) -> Vec<SectorGroup> { self.get_teams_result().unwrap_or(Vec::new()) }

    fn get_teams_result(&self) -> Result<Vec<SectorGroup>, CliError> {
        let ghteams = self.get_ghteam_map()?;
        let mut teams = Vec::new();
        for team_conf in &self.conf.team {
            if let &Some(ghteam) = &ghteams.get(&team_conf.name) {
                teams.push(SectorGroup { sector: Sector::from(ghteam.clone()),
                                         gid: team_conf.gid.clone(),
                                         group: team_conf.group.clone(),
                                         members: self.get_members(ghteam.id)?, });
            }
        }
        Ok(teams)
    }

    fn get_ghteam_map(&self) -> Result<HashMap<String, Team>, CliError> {
        let url = format!("{}/orgs/{}/teams", self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(&content)?;
        Ok(teams.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    fn get_members(&self, mid: u64) -> Result<HashMap<String, Member>, CliError> {
        let url = format!("{}/teams/{}/members", self.conf.endpoint, mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&content)?;
        Ok(members.iter()
                  .map(|m| (m.login.clone(), m.clone()))
                  .collect())
    }

    #[allow(dead_code)]
    pub fn clear_all_caches(&self) -> Result<(), CliError> {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache/**/*");
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
