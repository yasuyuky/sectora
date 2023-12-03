#[derive(Debug)]
pub enum Error {
    Serde(serde_json::Error),
    Io(std::io::Error),
    Toml(toml::de::Error),
    ParseMessageError(ParseMessageError),
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Error { Error::Serde(err) }
}
impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Error { Error::Io(err) }
}
impl From<toml::de::Error> for Error {
    fn from(err: toml::de::Error) -> Error { Error::Toml(err) }
}

#[derive(Debug)]
pub enum ParseSectorTypeError {
    UnknownType,
}

#[derive(Debug)]
pub enum ParseSectorError {
    Id(std::num::ParseIntError),
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
    fn from(err: ParseMessageError) -> Error { Error::ParseMessageError(err) }
}
