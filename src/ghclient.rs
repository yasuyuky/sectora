use crate::error::Error;
use crate::structs::{AppConfig, Config, Member, PublicKey, RateLimit, Repo, Sector, SectorGroup, Team};
use glob::glob;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use reqwest::{Client, Method, Request, Url, header};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// Installation tokens expire after 1 hour
const APP_TOKEN_TTL: Duration = Duration::from_secs(50 * 60);

#[derive(Deserialize)]
struct Installation {
    id: u64,
}

#[derive(Deserialize)]
struct AccessToken {
    token: String,
}

pub struct GithubClient {
    client: Client,
    pub conf: Config,
    app_token: Mutex<Option<(header::HeaderValue, Instant)>>,
}

fn b64url(bytes: &[u8]) -> String {
    openssl::base64::encode_block(bytes).trim_end_matches('=')
                                        .replace('+', "-")
                                        .replace('/', "_")
}

fn app_jwt(app_id: u64, private_key_pem: &[u8]) -> Result<String, Error> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)
                               .map_err(|_| Error::Auth)?
                               .as_secs();
    // iat is backdated to tolerate clock drift; GitHub rejects exp more than 10 minutes ahead
    let claims = format!(r#"{{"iat":{},"exp":{},"iss":{}}}"#, now - 60, now + 540, app_id);
    let msg = format!("{}.{}",
                      b64url(br#"{"alg":"RS256","typ":"JWT"}"#),
                      b64url(claims.as_bytes()));
    let key = PKey::private_key_from_pem(private_key_pem)?;
    let mut signer = Signer::new(MessageDigest::sha256(), &key)?;
    signer.update(msg.as_bytes())?;
    Ok(format!("{}.{}", msg, b64url(&signer.sign_to_vec()?)))
}

impl GithubClient {
    pub fn new(config: &Config) -> GithubClient {
        if std::env::var("SSL_CERT_FILE").is_err() {
            // SAFETY: called once at process start before other threads use TLS
            unsafe {
                std::env::set_var("SSL_CERT_FILE", &config.cert_path);
            }
        }
        let mut hmap = header::HeaderMap::new();
        if config.app.is_none() {
            let token = String::from("token ") + &config.token;
            hmap.insert(header::AUTHORIZATION,
                        header::HeaderValue::from_str(&token).expect("valid authorization header"));
        }
        hmap.insert(header::USER_AGENT,
                    header::HeaderValue::from_str("sectora").expect("valid user-agent"));
        let client = Client::builder().default_headers(hmap)
                                      .connect_timeout(Duration::from_secs(10))
                                      .timeout(Duration::from_secs(30))
                                      .build()
                                      .expect("build HTTP client");
        GithubClient { client,
                       conf: config.clone(),
                       app_token: Mutex::new(None) }
    }

    async fn app_token(&self, app: &AppConfig) -> Result<header::HeaderValue, Error> {
        if let Some((token, fetched_at)) = &*self.app_token.lock().unwrap_or_else(|e| e.into_inner())
           && fetched_at.elapsed() < APP_TOKEN_TTL
        {
            return Ok(token.clone());
        }
        let pem = std::fs::read(&app.private_key_path).map_err(|e| {
            log::warn!("failed to read GitHub App private key {}: {}", app.private_key_path, e);
            Error::Io
        })?;
        let bearer = format!("Bearer {}", app_jwt(app.id, &pem)?);
        let url = format!("{}/orgs/{}/installation", self.conf.endpoint, self.conf.org);
        let installation: Installation = self.app_request(Method::GET, &url, &bearer).await?;
        let url = format!("{}/app/installations/{}/access_tokens",
                          self.conf.endpoint, installation.id);
        let access: AccessToken = self.app_request(Method::POST, &url, &bearer).await?;
        let token = header::HeaderValue::from_str(&format!("token {}", access.token)).map_err(|_| Error::Auth)?;
        *self.app_token.lock().unwrap_or_else(|e| e.into_inner()) = Some((token.clone(), Instant::now()));
        Ok(token)
    }

    async fn app_request<T: serde::de::DeserializeOwned>(&self, method: Method, url: &str, bearer: &str)
                                                         -> Result<T, Error> {
        let req = self.client.request(method, url).header(header::AUTHORIZATION, bearer);
        let resp = req.send().await.map_err(|e| {
            log::warn!("GitHub App request failed for {}: {}", url, e);
            Error::Http
        })?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            let preview: String = body.chars().take(200).collect();
            log::warn!("GitHub App HTTP {} for {}: {}", status.as_u16(), url, preview);
            return Err(Error::Http);
        }
        Ok(serde_json::from_str(&body)?)
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

    async fn build_request(&self, url: &str) -> Result<Request, Error> {
        let parsed = Url::parse(url).map_err(|e| {
            log::warn!("invalid url {}: {}", url, e);
            Error::Request
        })?;
        let mut req = Request::new(Method::GET, parsed);
        if let Some(app) = &self.conf.app {
            req.headers_mut()
               .insert(header::AUTHORIZATION, self.app_token(app).await?);
        }
        Ok(req)
    }

    async fn build_page_request(&self, url: &str, page: u64) -> Result<Request, Error> {
        let sep = if url.contains('?') { '&' } else { '?' };
        let url_p = format!("{}{}page={}", url, sep, page);
        self.build_request(&url_p).await
    }

    async fn get_contents_from_url_page(&self, url: &str, page: u64) -> Result<Vec<serde_json::Value>, Error> {
        let req = self.build_page_request(url, page).await?;
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
        let req = self.build_request(&url).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::rsa::Rsa;
    use openssl::sign::Verifier;

    fn b64url_decode(s: &str) -> Vec<u8> {
        let mut b64 = s.replace('-', "+").replace('_', "/");
        while !b64.len().is_multiple_of(4) {
            b64.push('=');
        }
        openssl::base64::decode_block(&b64).unwrap()
    }

    #[test]
    fn app_jwt_is_rs256_signed_for_github() {
        let rsa = Rsa::generate(2048).unwrap();
        // GitHub hands out PKCS#1 ("BEGIN RSA PRIVATE KEY") PEM files
        let jwt = app_jwt(12345, &rsa.private_key_to_pem().unwrap()).unwrap();
        let parts: Vec<&str> = jwt.split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(!jwt.contains(['=', '+', '/']));

        let header: serde_json::Value = serde_json::from_slice(&b64url_decode(parts[0])).unwrap();
        assert_eq!(header["alg"], "RS256");
        let claims: serde_json::Value = serde_json::from_slice(&b64url_decode(parts[1])).unwrap();
        assert_eq!(claims["iss"], 12345);
        assert_eq!(claims["exp"].as_u64().unwrap() - claims["iat"].as_u64().unwrap(), 600);

        let key = PKey::from_rsa(rsa).unwrap();
        let mut verifier = Verifier::new(MessageDigest::sha256(), &key).unwrap();
        verifier.update(format!("{}.{}", parts[0], parts[1]).as_bytes())
                .unwrap();
        assert!(verifier.verify(&b64url_decode(parts[2])).unwrap());
    }

    #[test]
    fn app_jwt_rejects_invalid_key() {
        assert!(matches!(app_jwt(1, b"not a pem"), Err(Error::Auth)));
    }
}
