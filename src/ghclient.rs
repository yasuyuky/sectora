use glob::glob;
use reqwest;
use reqwest::header::Authorization;
use serde_json;
use std;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use structs::{CliError, Config, Member, PublicKey, Repo, Sector, SectorGroup, Team};

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
        path.push("sectora-cache");
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
        let mut all_contents: Vec<serde_json::value::Value> = Vec::new();
        let mut page = 1;
        loop {
            let mut new_array = self.get_content_from_url_page(url, page)?;
            if new_array.is_empty() {
                break;
            }
            all_contents.append(&mut new_array);
            page += 1;
        }
        let content = serde_json::ser::to_string(&all_contents)?;
        self.store_content_to_cache(url, &content)?;
        Ok(content)
    }

    fn get_content_from_url_page(&self, url: &str, page: u64) -> Result<Vec<serde_json::value::Value>, CliError> {
        let token = String::from("token ") + &self.conf.token;
        let sep = if url.contains("?") { "&" } else { "?" };
        let url_p = format!("{}{}page={}", url, sep, page);
        let res = self.client.get(&url_p).header(Authorization(token)).send();
        let mut content = String::new();
        res?.read_to_string(&mut content)?;
        Ok(serde_json::from_str::<Vec<serde_json::value::Value>>(&content)?)
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
        let sectors = self.get_sectors()?;
        Ok(sectors.iter().any(|team| team.members.contains_key(user)))
    }

    pub fn get_sectors(&self) -> Result<Vec<SectorGroup>, CliError> {
        let mut sectors: Vec<SectorGroup> = self.get_teams_result()?;
        sectors.append(&mut self.get_repos_result()?);
        Ok(sectors)
    }

    fn get_teams_result(&self) -> Result<Vec<SectorGroup>, CliError> {
        let gh_teams = self.get_team_map()?;
        let mut teams = Vec::new();
        for team_conf in &self.conf.team {
            if let &Some(gh_team) = &gh_teams.get(&team_conf.name) {
                teams.push(SectorGroup { sector: Sector::from(gh_team.clone()),
                                         gid: team_conf.gid.clone(),
                                         group: team_conf.group.clone(),
                                         members: self.get_team_members(gh_team.id)?, });
            }
        }
        Ok(teams)
    }

    fn get_team_map(&self) -> Result<HashMap<String, Team>, CliError> {
        let url = format!("{}/orgs/{}/teams", self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(&content)?;
        Ok(teams.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    fn get_team_members(&self, mid: u64) -> Result<HashMap<String, Member>, CliError> {
        let url = format!("{}/teams/{}/members", self.conf.endpoint, mid);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&content)?;
        Ok(members.iter()
                  .map(|m| (m.login.clone(), m.clone()))
                  .collect())
    }

    fn get_repos_result(&self) -> Result<Vec<SectorGroup>, CliError> {
        let gh_repos = self.get_repo_map()?;
        let mut repos = Vec::new();
        for repo_conf in &self.conf.repo {
            if let &Some(gh_repo) = &gh_repos.get(&repo_conf.name) {
                repos.push(SectorGroup { sector: Sector::from(gh_repo.clone()),
                                         gid: repo_conf.gid.clone(),
                                         group: repo_conf.group.clone(),
                                         members: self.get_repo_collaborators(&gh_repo.name)?, });
            }
        }
        Ok(repos)
    }

    fn get_repo_map(&self) -> Result<HashMap<String, Repo>, CliError> {
        let url = format!("{}/orgs/{}/repos", self.conf.endpoint, self.conf.org);
        let content = self.get_content(&url)?;
        let repos = serde_json::from_str::<Vec<Repo>>(&content)?;
        Ok(repos.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    fn get_repo_collaborators(&self, repo_name: &str) -> Result<HashMap<String, Member>, CliError> {
        let url = format!("{}/repos/{}/{}/collaborators?affiliation=outside",
                          self.conf.endpoint, self.conf.org, repo_name);
        let content = self.get_content(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&content)?;
        Ok(members.iter()
                  .map(|m| (m.login.clone(), m.clone()))
                  .collect())
    }

    #[allow(dead_code)]
    pub fn clear_all_caches(&self) -> Result<(), CliError> {
        let mut path = std::env::temp_dir();
        path.push("sectora-cache/**/*");
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
