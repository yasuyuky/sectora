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
    Ent(Ent),
}

#[derive(Debug)]
pub enum Gr {
    Gid(u64),
    Nam(String),
    Ent(Ent),
}

#[derive(Debug)]
pub enum Ent {
    Set(u32),
    Get(u32),
    End(u32),
}

pub struct DividedMessage {
    pub cont: bool,
    pub message: String,
}

impl DividedMessage {
    #[allow(dead_code)]
    pub fn new(msg: &str, size: usize) -> Vec<Self> {
        let mut msgs = vec![];
        let mut idx = 0;
        while idx + size < msg.len() {
            msgs.push(Self { cont: true,
                             message: msg[idx..idx + size].to_owned() });
            idx += size
        }
        msgs.push(Self { cont: false,
                         message: msg[idx..msg.len()].to_owned() });
        msgs
    }
}

impl fmt::Display for DividedMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}:{}", i32::from(self.cont), self.message) }
}

impl FromStr for DividedMessage {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("0:") {
            Ok(Self { cont: false,
                      message: msg.to_owned() })
        } else if let Some(msg) = s.strip_prefix("1:") {
            Ok(Self { cont: true,
                      message: msg.to_owned() })
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

#[derive(Debug)]
pub enum ClientMessage {
    Cont,
    Key { user: String },
    Pam { user: String },
    CleanUp,
    RateLimit,
    SectorGroups,
    Pw(Pw),
    Sp(Sp),
    Gr(Gr),
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum DaemonMessage {
    Success,
    Error {
        message: String,
    },
    Key {
        keys: String,
    },
    Pam {
        result: bool,
    },
    RateLimit {
        limit: usize,
        remaining: usize,
        reset: usize,
    },
    SectorGroups {
        sectors: Vec<structs::SectorGroup>,
    },
    Pw {
        login: String,
        uid: u64,
        gid: u64,
        home: String,
        sh: String,
    },
    Sp {
        login: String,
        pass: String,
    },
    Gr {
        sector: structs::SectorGroup,
    },
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
            Sp::Ent(ent) => write!(f, "ent={}", ent),
        }
    }
}

impl fmt::Display for Gr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gr::Gid(gid) => write!(f, "gid={}", gid),
            Gr::Nam(name) => write!(f, "name={}", name),
            Gr::Ent(ent) => write!(f, "ent={}", ent),
        }
    }
}

impl fmt::Display for ClientMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientMessage::Cont => write!(f, "c:cont"),
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
            DaemonMessage::RateLimit { limit,
                                       remaining,
                                       reset, } => write!(f, "d:ratelimit:{}:{}:{}", limit, remaining, reset),
            DaemonMessage::SectorGroups { sectors } => {
                let ss: Vec<String> = sectors.iter().map(|s| s.to_string()).collect();
                write!(f, "d:sectors:{}", ss.join("\n"))
            }
            DaemonMessage::Pw { login,
                                uid,
                                gid,
                                home,
                                sh, } => write!(f, "d:pw:{}:{}:{}:{}:{}", login, uid, gid, home, sh),
            DaemonMessage::Sp { login, pass } => write!(f, "d:sp:{}:{}", login, pass),
            DaemonMessage::Gr { sector } => write!(f, "d:gr:{}", sector),
        }
    }
}

impl FromStr for Ent {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("set|") {
            Ok(Ent::Set(msg.parse::<u32>().unwrap()))
        } else if let Some(msg) = s.strip_prefix("get|") {
            Ok(Ent::Get(msg.parse::<u32>().unwrap()))
        } else if let Some(msg) = s.strip_prefix("end|") {
            Ok(Ent::End(msg.parse::<u32>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Pw {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("uid=") {
            Ok(Pw::Uid(msg.parse::<u64>().unwrap()))
        } else if let Some(msg) = s.strip_prefix("name=") {
            Ok(Pw::Nam(String::from(msg)))
        } else if let Some(msg) = s.strip_prefix("ent=") {
            Ok(Pw::Ent(msg.parse::<Ent>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Sp {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("name=") {
            Ok(Sp::Nam(String::from(msg)))
        } else if let Some(msg) = s.strip_prefix("ent=") {
            Ok(Sp::Ent(msg.parse::<Ent>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for Gr {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("gid=") {
            Ok(Gr::Gid(msg.parse::<u64>().unwrap()))
        } else if let Some(msg) = s.strip_prefix("name=") {
            Ok(Gr::Nam(String::from(msg)))
        } else if let Some(msg) = s.strip_prefix("ent=") {
            Ok(Gr::Ent(msg.parse::<Ent>().unwrap()))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for ClientMessage {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "c:cont" {
            Ok(ClientMessage::Cont)
        } else if let Some(msg) = s.strip_prefix("c:key:") {
            Ok(ClientMessage::Key { user: String::from(msg) })
        } else if let Some(msg) = s.strip_prefix("c:pam:") {
            Ok(ClientMessage::Pam { user: String::from(msg) })
        } else if s == "c:cleanup" {
            Ok(ClientMessage::CleanUp)
        } else if s == "c:ratelimit" {
            Ok(ClientMessage::RateLimit)
        } else if s == "c:sectors" {
            Ok(ClientMessage::SectorGroups)
        } else if let Some(msg) = s.strip_prefix("c:pw:") {
            Ok(ClientMessage::Pw(msg.parse::<Pw>()?))
        } else if let Some(msg) = s.strip_prefix("c:sp:") {
            Ok(ClientMessage::Sp(msg.parse::<Sp>()?))
        } else if let Some(msg) = s.strip_prefix("c:gr:") {
            Ok(ClientMessage::Gr(msg.parse::<Gr>()?))
        } else {
            Err(ParseMessageError::ParseClientMessageError)
        }
    }
}

impl FromStr for DaemonMessage {
    type Err = ParseMessageError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(msg) = s.strip_prefix("d:key:") {
            Ok(DaemonMessage::Key { keys: String::from(msg) })
        } else if let Some(msg) = s.strip_prefix("d:pam:") {
            Ok(DaemonMessage::Pam { result: FromStr::from_str(msg).unwrap_or(false) })
        } else if s == "d:success" {
            Ok(DaemonMessage::Success)
        } else if let Some(msg) = s.strip_prefix("d:ratelimit:") {
            let fields: Vec<String> = msg.split(':').map(|s| s.to_string()).collect();
            if fields.len() < 3 {
                return Err(ParseMessageError::ParseDaemonMessageError);
            }
            let limit = fields[0].clone().parse().unwrap_or(0);
            let remaining = fields[1].clone().parse().unwrap_or(0);
            let reset = fields[2].clone().parse().unwrap_or(0);
            Ok(DaemonMessage::RateLimit { limit,
                                          remaining,
                                          reset })
        } else if let Some(msg) = s.strip_prefix("d:sectors:") {
            let sectors = msg.split('\n')
                             .filter_map(|l| l.parse::<structs::SectorGroup>().ok())
                             .collect();
            Ok(DaemonMessage::SectorGroups { sectors })
        } else if let Some(msg) = s.strip_prefix("d:pw:") {
            let fields: Vec<String> = msg.split(':').map(|s| s.to_string()).collect();
            if fields.len() < 5 {
                return Err(ParseMessageError::ParseDaemonMessageError);
            }
            let login: String = fields[0].clone();
            let home: String = fields[3].clone();
            let sh: String = fields[4].clone();
            match (fields[1].parse::<u64>(), fields[2].parse::<u64>()) {
                (Ok(uid), Ok(gid)) => Ok(DaemonMessage::Pw { login,
                                                             uid,
                                                             gid,
                                                             home,
                                                             sh }),
                _ => Err(ParseMessageError::ParseDaemonMessageError),
            }
        } else if let Some(msg) = s.strip_prefix("d:sp:") {
            let fields: Vec<String> = msg.split(':').map(|s| s.to_string()).collect();
            if fields.len() < 2 {
                return Err(ParseMessageError::ParseDaemonMessageError);
            }
            Ok(DaemonMessage::Sp { login: fields[0].clone(),
                                   pass: fields[1].clone() })
        } else if let Some(msg) = s.strip_prefix("d:gr:") {
            match msg.parse::<structs::SectorGroup>() {
                Ok(sector) => Ok(DaemonMessage::Gr { sector }),
                _ => Err(ParseMessageError::ParseDaemonMessageError),
            }
        } else {
            Err(ParseMessageError::ParseDaemonMessageError)
        }
    }
}
