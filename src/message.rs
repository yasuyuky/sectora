use crate::error::ParseMessageError;
use crate::structs;
use std::fmt;
use std::str::FromStr;

#[derive(Debug)]
pub enum Pw {
    Uid(u64),
    Nam(String),
    Ent(Ent),
}

#[derive(Debug)]
pub enum Sp {
    Nam(String),
}

#[derive(Debug)]
pub enum Gr {
    Gid(u64),
    Nam(String),
}

#[derive(Debug)]
pub enum Ent {
    Set(u32),
    Get(u32),
    End(u32),
}

#[derive(Debug)]
pub enum ClientMessage {
    Key { user: String },
    Pam { user: String },
    CleanUp,
    RateLimit,
    SectorGroups,
    Pw(Pw),
    Sp(Sp),
    Gr(Gr),
}

#[allow(unused)]
#[derive(Debug)]
pub enum DaemonMessage {
    Success,
    Error { message: String },
    Key { keys: String },
    Pam { result: bool },
    RateLimit { limit: usize },
    SectorGroups { sectors: Vec<structs::SectorGroup> },
    Pw { login: String, uid: u64, gid: u64 },
    Sp { login: String },
    Gr { sector: structs::SectorGroup },
}

impl fmt::Display for Ent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ent::Set(pid) => write!(f, "set|{}", pid),
            Ent::Get(pid) => write!(f, "get|{}", pid),
            Ent::End(pid) => write!(f, "end|{}", pid),
        }
    }
}

impl fmt::Display for Pw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Pw::Uid(uid) => write!(f, "uid={}", uid),
            Pw::Nam(name) => write!(f, "name={}", name),
            Pw::Ent(ent) => write!(f, "ent={}", ent),
        }
    }
}

impl fmt::Display for Sp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Sp::Nam(name) => write!(f, "name={}", name),
        }
    }
}

impl fmt::Display for Gr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gr::Gid(gid) => write!(f, "gid={}", gid),
            Gr::Nam(name) => write!(f, "name={}", name),
        }
    }
}

impl fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientMessage::Key { user } => write!(f, "c:key:{}", user),
            ClientMessage::Pam { user } => write!(f, "c:pam:{}", user),
            ClientMessage::CleanUp => write!(f, "c:cleanup"),
            ClientMessage::RateLimit => write!(f, "c:ratelimit"),
            ClientMessage::SectorGroups => write!(f, "c:sectors"),
            ClientMessage::Pw(pw) => write!(f, "c:pw:{}", pw),
            ClientMessage::Sp(sp) => write!(f, "c:sp:{}", sp),
            ClientMessage::Gr(gr) => write!(f, "c:gr:{}", gr),
        }
    }
}

impl fmt::Display for DaemonMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DaemonMessage::Error { message } => write!(f, "d:error:{}", message),
            DaemonMessage::Success => write!(f, "d:success"),
            DaemonMessage::Key { keys } => write!(f, "d:key:{}", keys),
            DaemonMessage::Pam { result } => write!(f, "d:pam:{}", result),
            DaemonMessage::RateLimit { limit } => write!(f, "d:ratelimit:{}", limit),
            DaemonMessage::SectorGroups { sectors } => {
                let ss: Vec<String> = sectors.iter().map(|s| s.to_string()).collect();
                write!(f, "d:sectors:{}", ss.join("\n"))
            }
            DaemonMessage::Pw { login, uid, gid } => write!(f, "d:pw:{}:{}:{}", login, uid, gid),
            DaemonMessage::Sp { login } => write!(f, "d:sp:{}", login),
            DaemonMessage::Gr { sector } => write!(f, "d:gr:{}", sector),
        }
    }
}

impl FromStr for Ent {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("set|") {
            Ok(Ent::Set(s.get(4..).unwrap_or_default().parse::<u32>().unwrap()))
        } else if s.starts_with("get|") {
            Ok(Ent::Get(s.get(4..).unwrap_or_default().parse::<u32>().unwrap()))
        } else if s.starts_with("end|") {
            Ok(Ent::End(s.get(4..).unwrap_or_default().parse::<u32>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Pw {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("uid=") {
            Ok(Pw::Uid(s.get(4..).unwrap_or_default().parse::<u64>().unwrap()))
        } else if s.starts_with("name=") {
            Ok(Pw::Nam(String::from(s.get(5..).unwrap_or_default())))
        } else if s.starts_with("ent=") {
            Ok(Pw::Ent(s.get(4..).unwrap_or_default().parse::<Ent>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Sp {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("name=") {
            Ok(Sp::Nam(String::from(s.get(5..).unwrap_or_default())))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Gr {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("gid=") {
            Ok(Gr::Gid(s.get(4..).unwrap_or_default().parse::<u64>().unwrap()))
        } else if s.starts_with("name=") {
            Ok(Gr::Nam(String::from(s.get(5..).unwrap_or_default())))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
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
        } else if s.starts_with("c:pw:") {
            Ok(ClientMessage::Pw(s.get(5..).unwrap_or_default().parse::<Pw>()?))
        } else if s.starts_with("c:sp:") {
            Ok(ClientMessage::Sp(s.get(5..).unwrap_or_default().parse::<Sp>()?))
        } else if s.starts_with("c:gr:") {
            Ok(ClientMessage::Gr(s.get(5..).unwrap_or_default().parse::<Gr>()?))
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
        } else if s == "d:success" {
            Ok(DaemonMessage::Success)
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
        } else if s.starts_with("d:sp:") {
            match s.get(5..).unwrap_or_default().parse::<String>() {
                Ok(login) => Ok(DaemonMessage::Sp { login }),
                _ => Err(ParseMessageError::ParseDaemonMessageError),
            }
        } else if s.starts_with("d:gr:") {
            match s.get(5..).unwrap_or_default().parse::<structs::SectorGroup>() {
                Ok(sector) => Ok(DaemonMessage::Gr { sector }),
                _ => Err(ParseMessageError::ParseDaemonMessageError),
            }
        } else {
            Err(ParseMessageError::ParseDaemonMessageError)
        }
    }
}
