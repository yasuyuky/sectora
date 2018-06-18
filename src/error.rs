use http;
use hyper;
use hyper_tls;
use serde_json;
use std;
use toml;

#[derive(Debug)]
pub enum CliError {
    Serde(serde_json::Error),
    Io(std::io::Error),
    Toml(toml::de::Error),
    Http(http::Error),
    Hyper(hyper::Error),
    HyperTls(hyper_tls::Error),
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
impl From<http::Error> for CliError {
    fn from(err: http::Error) -> CliError { CliError::Http(err) }
}
impl From<hyper_tls::Error> for CliError {
    fn from(err: hyper_tls::Error) -> CliError { CliError::HyperTls(err) }
}
