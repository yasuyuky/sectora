use crate::error::ParseMessageError;
use crate::structs;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub enum ClientMessage {
    Key { user: String },
    Pam { user: String },
    CleanUp,
    RateLimit,
    SectorGroups,
    PwUid { uid: u64 },
}

#[allow(unused)]
#[derive(Debug)]
pub enum DaemonMessage {
    Error { message: String },
    Key { keys: String },
    Pam { result: bool },
    CleanUp,
    RateLimit { limit: usize },
    SectorGroups { sectors: Vec<structs::SectorGroup> },
    Pw { login: String, uid: u64, gid: u64 },
}

impl fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientMessage::Key { user } => write!(f, "c:key:{}", user),
            ClientMessage::Pam { user } => write!(f, "c:pam:{}", user),
            ClientMessage::CleanUp => write!(f, "c:cleanup"),
            ClientMessage::RateLimit => write!(f, "c:ratelimit"),
            ClientMessage::SectorGroups => write!(f, "c:sectors"),
            ClientMessage::PwUid { uid } => write!(f, "c:pwuid:{}", uid),
        }
    }
}

impl fmt::Display for DaemonMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonMessage::Error { message } => write!(f, "d:error:{}", message),
            DaemonMessage::Key { keys } => write!(f, "d:key:{}", keys),
            DaemonMessage::Pam { result } => write!(f, "d:pam:{}", result),
            DaemonMessage::CleanUp => write!(f, "d:cleanup"),
            DaemonMessage::RateLimit { limit } => write!(f, "d:ratelimit:{}", limit),
            DaemonMessage::SectorGroups { sectors } => {
                let ss: Vec<String> = sectors.iter().map(|s| s.to_string()).collect();
                write!(f, "d:sectors:{}", ss.join("\n"))
            }
            DaemonMessage::Pw { login, uid, gid } => write!(f, "d:pw:{}:{}:{}", login, uid, gid),
        }
    }
}

impl FromStr for ClientMessage {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("c:key:") {
            Ok(ClientMessage::Key { user: String::from(s.get(6..).unwrap_or_default()) })
        } else if s.starts_with("c:pam:") {
            Ok(ClientMessage::Pam { user: String::from(s.get(6..).unwrap_or_default()) })
        } else if s == "c:cleanup" {
            Ok(ClientMessage::CleanUp)
        } else if s == "c:ratelimit" {
            Ok(ClientMessage::RateLimit)
        } else if s == "c:sectors" {
            Ok(ClientMessage::SectorGroups)
        } else if s.starts_with("c:pwuid:") {
            Ok(ClientMessage::PwUid { uid: s.get(8..).unwrap_or_default().parse::<u64>().unwrap() })
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for DaemonMessage {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("d:key:") {
            Ok(DaemonMessage::Key { keys: String::from(s.get(6..).unwrap_or_default()) })
        } else if s.starts_with("d:pam:") {
            Ok(DaemonMessage::Pam { result: FromStr::from_str(s.get(6..).unwrap_or("false")).unwrap_or(false) })
        } else if s == "d:cleanup" {
            Ok(DaemonMessage::CleanUp)
        } else if s.starts_with("d:ratelimit:") {
            Ok(DaemonMessage::RateLimit { limit: s.get(12..).unwrap_or("0").parse::<usize>().unwrap_or(0) })
        } else if s.starts_with("d:sectors:") {
            let sectors = s.get(10..)
                           .unwrap_or_default()
                           .split('\n')
                           .filter_map(|l| l.parse::<structs::SectorGroup>().ok())
                           .collect();
            Ok(DaemonMessage::SectorGroups { sectors })
        } else if s.starts_with("d:pw:") {
            let fields: Vec<String> = s.get(5..)
                                       .unwrap_or_default()
                                       .split(":")
                                       .map(|s| s.to_string())
                                       .collect();
            if fields.len() < 3 {
                return Err(ParseMessageError::ParseDaemonMessageError);
            }
            let login: String = fields[0].clone();
            match (fields[1].parse::<u64>(), fields[2].parse::<u64>()) {
                (Ok(uid), Ok(gid)) => Ok(DaemonMessage::Pw { login, uid, gid }),
                _ => Err(ParseMessageError::ParseDaemonMessageError),
            }
        } else {
            Err(ParseMessageError::ParseDaemonMessageError)
        }
    }
}
