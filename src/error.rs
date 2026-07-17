#[derive(Debug)]
pub enum Error {
    Serde,
    Io,
    Toml,
    ParseMsg,
    /// HTTP transport, DNS, TLS, or non-success status from GitHub API
    Http,
    /// URL parse / request construction failure
    Request,
}

impl From<serde_json::Error> for Error {
    fn from(_err: serde_json::Error) -> Error { Error::Serde }
}
impl From<std::io::Error> for Error {
    fn from(_err: std::io::Error) -> Error { Error::Io }
}
impl From<toml::de::Error> for Error {
    fn from(_err: toml::de::Error) -> Error { Error::Toml }
}
impl From<reqwest::Error> for Error {
    fn from(_err: reqwest::Error) -> Error { Error::Http }
}

#[derive(Debug)]
pub enum ParseSectorTypeError {
    UnknownType,
}

#[derive(Debug)]
pub enum ParseSectorError {
    Id,
    Type(ParseSectorTypeError),
    BadFormat,
}

#[derive(Debug)]
pub enum ParseSectorGroupError {
    Sector,
    Gid,
    Member,
}

#[derive(Debug)]
pub enum ParseMessageError {
    ParseClientMessageError,
    ParseDaemonMessageError,
}

impl From<ParseMessageError> for Error {
    fn from(_err: ParseMessageError) -> Error { Error::ParseMsg }
}
