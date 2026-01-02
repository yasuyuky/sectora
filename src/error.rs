#[derive(Debug)]
pub enum Error {
    Serde,
    Io,
    Toml,
    ParseMsg,
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
    Sector(ParseSectorError),
    Gid(std::num::ParseIntError),
    Member(std::num::ParseIntError),
}

#[derive(Debug)]
pub enum ParseMessageError {
    ParseClientMessageError,
    ParseDaemonMessageError,
}

impl From<ParseMessageError> for Error {
    fn from(_err: ParseMessageError) -> Error { Error::ParseMsg }
}
