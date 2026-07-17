use crate::error::Error;
use crate::structs::{Config, Member, PublicKey, RateLimit, Repo, Sector, SectorGroup, Team};
use glob::glob;
use reqwest::{Client, Method, Request, Url, header};
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::time::Duration;

pub struct GithubClient {
    client: Client,
    pub conf: Config,
}

impl GithubClient {
    pub fn new(config: &Config) -> GithubClient {
        if std::env::var("SSL_CERT_FILE").is_err() {
            // SAFETY: called once at process start before other threads use TLS
            unsafe {
                std::env::set_var("SSL_CERT_FILE", &config.cert_path);
            }
        }
        let token = String::from("token ") + &config.token;
        let mut hmap = header::HeaderMap::new();
        hmap.insert(header::AUTHORIZATION,
                    header::HeaderValue::from_str(&token).expect("valid authorization header"));
        hmap.insert(header::USER_AGENT,
                    header::HeaderValue::from_str("sectora").expect("valid user-agent"));
        let client = Client::builder().default_headers(hmap)
                                      .connect_timeout(Duration::from_secs(10))
                                      .timeout(Duration::from_secs(30))
                                      .build()
                                      .expect("build HTTP client");
        GithubClient { client,
                       conf: config.clone() }
    }

    fn get_cache_path(&self, url: &str) -> std::path::PathBuf {
        let mut path = std::path::PathBuf::default();
        path.push(&self.conf.cache_dir);
        path.push(url);
        path
    }

    fn load_contents_from_cache(&self, url: &str) -> Result<(std::fs::Metadata, String), Error> {
        let path = self.get_cache_path(url);
        let metadata = std::fs::metadata(path.to_str().unwrap())?;
        let mut f = File::open(path.to_str().unwrap())?;
        let mut contents = String::default();
        f.read_to_string(&mut contents)?;
        Ok((metadata, contents))
    }

    fn store_contents_to_cache(&self, url: &str, contents: &str) -> Result<(), Error> {
        let path = self.get_cache_path(url);
        std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("/")))?;
        let mut f = File::create(path.to_str().unwrap())?;
        f.write_all(contents.as_bytes())?;
        Ok(())
    }

    async fn get_contents_from_url(&self, url: &str) -> Result<String, Error> {
        let mut all_contents: Vec<serde_json::value::Value> = Vec::new();
        let mut page = 1;
        loop {
            let mut new_array = self.get_contents_from_url_page(url, page).await?;
            if new_array.is_empty() {
                break;
            }
            all_contents.append(&mut new_array);
            page += 1;
        }
        let contents = serde_json::ser::to_string(&all_contents)?;
        self.store_contents_to_cache(url, &contents)?;
        Ok(contents)
    }

    fn build_request(&self, url: &str) -> Result<Request, Error> {
        let parsed = Url::parse(url).map_err(|e| {
            log::warn!("invalid url {}: {}", url, e);
            Error::Request
        })?;
        Ok(Request::new(Method::GET, parsed))
    }

    fn build_page_request(&self, url: &str, page: u64) -> Result<Request, Error> {
        let sep = if url.contains('?') { '&' } else { '?' };
        let url_p = format!("{}{}page={}", url, sep, page);
        self.build_request(&url_p)
    }

    async fn get_contents_from_url_page(&self, url: &str, page: u64) -> Result<Vec<serde_json::Value>, Error> {
        let req = self.build_page_request(url, page)?;
        let resp = self.client.execute(req).await.map_err(|e| {
            log::warn!("GitHub request failed for {} page {}: {}", url, page, e);
            Error::Http
        })?;
        let status = resp.status();
        if !status.is_success() {
            // Read body only for logging; 5xx/rate-limit HTML would panic on json()
            let body = resp.text().await.unwrap_or_default();
            let preview: String = body.chars().take(200).collect();
            log::warn!("GitHub HTTP {} for {} page {}: {}", status.as_u16(), url, page, preview);
            return Err(Error::Http);
        }
        let body = resp.text().await.map_err(|e| {
            log::warn!("GitHub body read failed for {} page {}: {}", url, page, e);
            Error::Http
        })?;
        serde_json::from_str::<Vec<serde_json::Value>>(&body).map_err(|e| {
            let preview: String = body.chars().take(200).collect();
            log::warn!("GitHub JSON decode failed for {} page {}: {}; body={}", url, page, e, preview);
            Error::Serde
        })
    }

    async fn get_contents(&self, url: &str) -> Result<String, Error> {
        match self.load_contents_from_cache(url) {
            Ok((metadata, cache_contents)) => match std::time::SystemTime::now().duration_since(metadata.modified()?) {
                Ok(caching_duration) => {
                    if caching_duration.as_secs() > self.conf.cache_duration {
                        match self.get_contents_from_url(url).await {
                            Ok(contents_from_url) => Ok(contents_from_url),
                            Err(e) => {
                                log::warn!("refresh failed for {}, using stale cache: {:?}", url, e);
                                Ok(cache_contents)
                            }
                        }
                    } else {
                        Ok(cache_contents)
                    }
                }
                Err(_) => Ok(cache_contents),
            },
            Err(_) => self.get_contents_from_url(url).await,
        }
    }

    pub async fn get_user_public_keys(&self, user: &str) -> Result<Vec<String>, Error> {
        let url = format!("{}/users/{}/keys", self.conf.endpoint, user);
        let contents = self.get_contents(&url).await?;
        let keys = serde_json::from_str::<Vec<PublicKey>>(&contents)?;
        Ok(keys.iter().map(|k| k.key.clone()).collect())
    }

    pub async fn check_pam(&self, user: &str) -> Result<bool, Error> {
        let sectors = self.get_sectors().await?;
        Ok(sectors.iter().any(|team| team.members.contains_key(user)))
    }

    pub async fn get_sectors(&self) -> Result<Vec<SectorGroup>, Error> {
        let mut sectors: Vec<SectorGroup> = self.get_teams_result().await?;
        sectors.append(&mut self.get_repos_result().await?);
        Ok(sectors)
    }

    async fn get_teams_result(&self) -> Result<Vec<SectorGroup>, Error> {
        let gh_teams = self.get_team_map(&self.conf.org).await?;
        let mut teams = Vec::new();
        for team_conf in &self.conf.team {
            if let Some(gh_team) = gh_teams.get(&team_conf.name) {
                teams.push(SectorGroup { sector: Sector::from(gh_team.clone()),
                                         gid: team_conf.gid,
                                         group: team_conf.group.clone(),
                                         members: self.get_team_members(gh_team.id).await? });
            }
        }
        Ok(teams)
    }

    async fn get_team_map(&self, org: &str) -> Result<HashMap<String, Team>, Error> {
        let url = format!("{}/orgs/{}/teams", self.conf.endpoint, org);
        let contents = self.get_contents(&url).await?;
        let teams = serde_json::from_str::<Vec<Team>>(&contents)?;
        Ok(teams.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    async fn get_team_members(&self, mid: u64) -> Result<HashMap<String, Member>, Error> {
        let url = format!("{}/teams/{}/members", self.conf.endpoint, mid);
        let contents = self.get_contents(&url).await?;
        let members = serde_json::from_str::<Vec<Member>>(&contents)?;
        Ok(members.iter().map(|m| (m.login.clone(), m.clone())).collect())
    }

    async fn get_repos_result(&self) -> Result<Vec<SectorGroup>, Error> {
        let gh_repos = self.get_repo_map(&self.conf.org).await?;
        let mut repos = Vec::new();
        for repo_conf in &self.conf.repo {
            if let Some(gh_repo) = gh_repos.get(&repo_conf.name) {
                repos.push(SectorGroup { sector: Sector::from(gh_repo.clone()),
                                         gid: repo_conf.gid,
                                         group: repo_conf.group.clone(),
                                         members: self.get_repo_collaborators(&self.conf.org, &gh_repo.name)
                                                      .await? });
            }
        }
        Ok(repos)
    }

    async fn get_repo_map(&self, org: &str) -> Result<HashMap<String, Repo>, Error> {
        let url = format!("{}/orgs/{}/repos", self.conf.endpoint, org);
        let contents = self.get_contents(&url).await?;
        let repos = serde_json::from_str::<Vec<Repo>>(&contents)?;
        Ok(repos.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    async fn get_repo_collaborators(&self, org: &str, repo_name: &str) -> Result<HashMap<String, Member>, Error> {
        let url = format!("{}/repos/{}/{}/collaborators?affiliation=outside",
                          self.conf.endpoint, org, repo_name);
        let contents = self.get_contents(&url).await?;
        let members = serde_json::from_str::<Vec<Member>>(&contents)?;
        Ok(members.iter().map(|m| (m.login.clone(), m.clone())).collect())
    }

    pub async fn get_rate_limit(&self) -> Result<RateLimit, Error> {
        let url = format!("{}/rate_limit", self.conf.endpoint);
        let req = self.build_request(&url)?;
        let resp = self.client.execute(req).await.map_err(|e| {
            log::warn!("rate_limit request failed: {}", e);
            Error::Http
        })?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            let preview: String = body.chars().take(200).collect();
            log::warn!("rate_limit HTTP {}: {}", status.as_u16(), preview);
            return Err(Error::Http);
        }
        let body = resp.text().await.map_err(|e| {
            log::warn!("rate_limit body read failed: {}", e);
            Error::Http
        })?;
        serde_json::from_str(&body).map_err(|e| {
            let preview: String = body.chars().take(200).collect();
            log::warn!("rate_limit JSON decode failed: {}; body={}", e, preview);
            Error::Serde
        })
    }

    pub async fn clear_all_caches(&self) -> Result<(), Error> {
        let mut path = self.get_cache_path("");
        path.push("**/*");
        let pattern = path.to_str().ok_or(Error::Io)?;
        let entries = glob(pattern).map_err(|e| {
            log::warn!("cache glob failed: {}", e);
            Error::Io
        })?;
        for entry in entries {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        std::fs::remove_file(path)?
                    }
                }
                Err(e) => log::warn!("cache glob entry error: {:?}", e),
            }
        }
        Ok(())
    }
}
