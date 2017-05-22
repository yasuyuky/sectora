use structs::{CliError, Config, Member, Team, PublicKey};
use std;
use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;

use toml;
use glob::glob;
use reqwest;
use reqwest::header::Authorization;
use serde_json;

pub struct GithubClient {
    client: reqwest::Client,
    pub conf: Config
}

impl GithubClient {
    pub fn new(configpath:&str) -> Result<GithubClient, CliError> {
        let mut file = File::open(configpath)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();
        let config = toml::from_str::<Config>(contents.as_str()).unwrap();
        if std::env::var("SSL_CERT_FILE").is_err() {
            std::env::set_var("SSL_CERT_FILE", config.cert_path.as_str());
        }
        let client = reqwest::Client::new()?;
        Ok(GithubClient {client:client, conf:config})
    }

    fn get_cache_path(url:&String) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push("ghteam-auth-cache");
        path.push(url.as_str());
        path
    }

    fn load_content_from_cache(&self, url:&String) -> Result<(std::fs::Metadata,String),CliError> {
        let path = Self::get_cache_path(url);
        let metadata = std::fs::metadata(path.to_str().unwrap())?;
        let mut f = File::open(path.to_str().unwrap())?;
        let mut content = String::new();
        f.read_to_string(&mut content)?;
        Ok((metadata,content))
    }

    fn store_content_to_cache(&self, url:&String, content:&String) -> Result<(),CliError> {
        let path = Self::get_cache_path(url);
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut f = File::create(path.to_str().unwrap())?;
        f.write(content.as_bytes())?;
        Ok(())
    }

    fn get_content_from_url(&self, url:&String) -> Result<String,CliError> {
        let token = String::from("token ") + self.conf.token.as_str();
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

    #[allow(dead_code)]
    pub fn print_user_public_key(&self, user:&str) -> Result<(), CliError> {
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

    #[allow(dead_code)]
    pub fn check_pam(&self, user:&String) -> Result<bool, CliError> {
        let (_,members) = self.get_team_members()?;
        Ok(members.contains_key(user))
    }

    pub fn get_team_members(&self) -> Result<(Team,HashMap<String,Member>),CliError> {
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
        let url = format!("{}/teams/{}/members",self.conf.endpoint, mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(content.as_str())?;
        let mut member_map = HashMap::new();
        for member in members { member_map.insert(member.login.clone(), member); }
        Ok(member_map)
    }

    #[allow(dead_code)]
    pub fn clear_all_caches(&self) -> Result<(), CliError> {
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
