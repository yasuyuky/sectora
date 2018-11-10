use error::Error;
use glob::glob;
use hyper::client::HttpConnector;
use hyper::rt::{self, Future, Stream};
use hyper::{header, Body, Chunk, Client, Request, Response};
use hyper_tls::HttpsConnector;
use serde_json;
use statics::TEMP_DIRNAME;
use std;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::mpsc;
use structs::{Config, Member, PublicKey, RateLimit, Repo, Sector, SectorGroup, Team};

pub struct GithubClient {
    pub conf: Config,
}

impl GithubClient {
    pub fn new(config: &Config) -> GithubClient {
        if std::env::var("SSL_CERT_FILE").is_err() {
            std::env::set_var("SSL_CERT_FILE", &config.cert_path);
        }
        GithubClient { conf: config.clone() }
    }

    fn get_cache_path(url: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(TEMP_DIRNAME);
        path.push(url);
        path
    }

    fn load_contents_from_cache(&self, url: &str) -> Result<(std::fs::Metadata, String), Error> {
        let path = Self::get_cache_path(url);
        let metadata = std::fs::metadata(path.to_str().unwrap())?;
        let mut f = File::open(path.to_str().unwrap())?;
        let mut contents = String::new();
        f.read_to_string(&mut contents)?;
        Ok((metadata, contents))
    }

    fn store_contents_to_cache(&self, url: &str, contents: &str) -> Result<(), Error> {
        let path = Self::get_cache_path(url);
        std::fs::create_dir_all(path.parent().unwrap_or(std::path::Path::new("/")))?;
        let mut f = File::create(path.to_str().unwrap())?;
        f.write_all(contents.as_bytes())?;
        Ok(())
    }

    fn get_contents_from_url(&self, url: &str) -> Result<String, Error> {
        let mut all_contents: Vec<serde_json::value::Value> = Vec::new();
        let mut page = 1;
        loop {
            let mut new_array = self.get_contents_from_url_page(url, page)?;
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

    fn build_request(&self, url: &str) -> Result<Request<Body>, Error> {
        let token = String::from("token ") + &self.conf.token;
        Request::get(url).header(header::AUTHORIZATION, token.as_str())
                         .header(header::USER_AGENT, "sectora")
                         .body(Body::empty())
                         .map_err(Error::from)
    }

    fn build_page_request(&self, url: &str, page: u64) -> Result<Request<Body>, Error> {
        let sep = if url.contains('?') { '&' } else { '?' };
        let url_p = format!("{}{}page={}", url, sep, page);
        self.build_request(&url_p)
    }

    fn build_https_client() -> Result<Client<HttpsConnector<HttpConnector>>, Error> {
        Ok(Client::builder().build(HttpsConnector::new(4)?))
    }

    fn run_request(&self, req: Request<Body>) -> Result<Chunk, Error> {
        let concat_body = |res: Response<Body>| res.into_body().concat2();
        let (tx, rx) = mpsc::sync_channel(1);
        let ex = tx.clone();
        let send_res = move |r| tx.send(Ok(r)).expect("send response");
        let send_err = move |e| ex.send(Err(Error::from(e))).expect("send err");
        let hc = Self::build_https_client()?;
        rt::run(rt::lazy(move || hc.request(req).and_then(concat_body).map(send_res).map_err(send_err)));
        rx.recv().expect("recv response")
    }

    fn get_contents_from_url_page(&self, url: &str, page: u64) -> Result<Vec<serde_json::Value>, Error> {
        let req = self.build_page_request(url, page)?;
        self.run_request(req)
            .and_then(|body| serde_json::from_slice(&body).map_err(Error::from))
    }

    fn get_contents(&self, url: &str) -> Result<String, Error> {
        match self.load_contents_from_cache(url) {
            Ok((metadata, cache_contents)) => match std::time::SystemTime::now().duration_since(metadata.modified()?) {
                Ok(caching_duration) => {
                    if caching_duration.as_secs() > self.conf.cache_duration {
                        match self.get_contents_from_url(url) {
                            Ok(contents_from_url) => Ok(contents_from_url),
                            Err(_) => Ok(cache_contents),
                        }
                    } else {
                        Ok(cache_contents)
                    }
                }
                Err(_) => Ok(cache_contents),
            },
            Err(_) => self.get_contents_from_url(url),
        }
    }

    #[allow(dead_code)]
    pub fn print_user_public_key(&self, user: &str) -> Result<(), Error> {
        let keys = self.get_user_public_key(user)?;
        println!("{}", keys);
        Ok(())
    }

    fn get_user_public_key(&self, user: &str) -> Result<String, Error> {
        let url = format!("{}/users/{}/keys", self.conf.endpoint, user);
        let contents = self.get_contents(&url)?;
        let keys = serde_json::from_str::<Vec<PublicKey>>(&contents)?;
        Ok(keys.iter().map(|k| k.key.clone()).collect::<Vec<String>>().join("\n"))
    }

    #[allow(dead_code)]
    pub fn check_pam(&self, user: &str) -> Result<bool, Error> {
        let sectors = self.get_sectors()?;
        Ok(sectors.iter().any(|team| team.members.contains_key(user)))
    }

    pub fn get_sectors(&self) -> Result<Vec<SectorGroup>, Error> {
        let mut sectors: Vec<SectorGroup> = self.get_teams_result()?;
        sectors.append(&mut self.get_repos_result()?);
        Ok(sectors)
    }

    fn get_teams_result(&self) -> Result<Vec<SectorGroup>, Error> {
        let gh_teams = self.get_team_map()?;
        let mut teams = Vec::new();
        for team_conf in &self.conf.team {
            if let &Some(gh_team) = &gh_teams.get(&team_conf.name) {
                teams.push(SectorGroup { sector: Sector::from(gh_team.clone()),
                                         gid: team_conf.gid.clone(),
                                         group: team_conf.group.clone(),
                                         members: self.get_team_members(gh_team.id)? });
            }
        }
        Ok(teams)
    }

    fn get_team_map(&self) -> Result<HashMap<String, Team>, Error> {
        let url = format!("{}/orgs/{}/teams", self.conf.endpoint, self.conf.org);
        let contents = self.get_contents(&url)?;
        let teams = serde_json::from_str::<Vec<Team>>(&contents)?;
        Ok(teams.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    fn get_team_members(&self, mid: u64) -> Result<HashMap<String, Member>, Error> {
        let url = format!("{}/teams/{}/members", self.conf.endpoint, mid);
        let contents = self.get_contents(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&contents)?;
        Ok(members.iter().map(|m| (m.login.clone(), m.clone())).collect())
    }

    fn get_repos_result(&self) -> Result<Vec<SectorGroup>, Error> {
        let gh_repos = self.get_repo_map()?;
        let mut repos = Vec::new();
        for repo_conf in &self.conf.repo {
            if let &Some(gh_repo) = &gh_repos.get(&repo_conf.name) {
                repos.push(SectorGroup { sector: Sector::from(gh_repo.clone()),
                                         gid: repo_conf.gid.clone(),
                                         group: repo_conf.group.clone(),
                                         members: self.get_repo_collaborators(&gh_repo.name)? });
            }
        }
        Ok(repos)
    }

    fn get_repo_map(&self) -> Result<HashMap<String, Repo>, Error> {
        let url = format!("{}/orgs/{}/repos", self.conf.endpoint, self.conf.org);
        let contents = self.get_contents(&url)?;
        let repos = serde_json::from_str::<Vec<Repo>>(&contents)?;
        Ok(repos.iter().map(|t| (t.name.clone(), t.clone())).collect())
    }

    fn get_repo_collaborators(&self, repo_name: &str) -> Result<HashMap<String, Member>, Error> {
        let url = format!("{}/repos/{}/{}/collaborators?affiliation=outside",
                          self.conf.endpoint, self.conf.org, repo_name);
        let contents = self.get_contents(&url)?;
        let members = serde_json::from_str::<Vec<Member>>(&contents)?;
        Ok(members.iter().map(|m| (m.login.clone(), m.clone())).collect())
    }

    fn get_rate_limit(&self) -> Result<RateLimit, Error> {
        let url = format!("{}/rate_limit", self.conf.endpoint);
        let req = self.build_request(&url)?;
        self.run_request(req)
            .and_then(|body| serde_json::from_slice(&body).map_err(Error::from))
    }

    #[allow(dead_code)]
    pub fn print_rate_limit(&self) -> Result<(), Error> {
        let rate_limit = self.get_rate_limit()?;
        println!("{:?}", rate_limit);
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear_all_caches(&self) -> Result<(), Error> {
        let mut path = std::env::temp_dir();
        path.push(TEMP_DIRNAME);
        path.push("**/*");
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
