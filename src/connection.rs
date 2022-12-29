use crate::applog;
use crate::error;
use crate::message::*;
use crate::structs::SocketConfig as Config;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

#[derive(Debug)]
pub struct Connection {
    conf: Config,
    conn: UnixDatagram,
}

impl Connection {
    pub fn new(logid: &str) -> Result<Self, error::Error> {
        applog::init(Some("sectora"));
        log::debug!("{}", logid);
        let conf = Config::new();
        let conn = Self::connect_daemon(&conf)?;
        Ok(Self { conf, conn })
    }

    fn socket_path(conf: &Config) -> String { format!("{}/{}", &conf.socket_dir, std::process::id()) }

    fn connect_daemon(conf: &Config) -> Result<UnixDatagram, error::Error> {
        let socket = UnixDatagram::bind(Self::socket_path(conf))?;
        log::debug!("{:?}", socket);
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.connect(&conf.socket_path)?;
        Ok(socket)
    }

    pub fn communicate(&self, msg: ClientMessage) -> Result<DaemonMessage, error::Error> {
        self.conn.send(msg.to_string().as_bytes())?;
        let mut msgstr = String::new();
        let mut buf = [0u8; 4096];
        while let Ok(cnt) = self.conn.recv(&mut buf) {
            log::debug!("msg cnt, {}", cnt);
            let msg = String::from_utf8(buf[..cnt].to_vec()).unwrap()
                                                            .parse::<DividedMessage>()?;
            msgstr.push_str(&msg.message);
            if msg.cont {
                let _ = self.conn.send(ClientMessage::Cont.to_string().as_bytes())?;
            } else {
                break;
            }
        }
        log::debug!("recieved: {}", msgstr);
        Ok(msgstr.parse::<DaemonMessage>()?)
    }
}

impl Drop for Connection {
    fn drop(&mut self) { let _ = std::fs::remove_file(Self::socket_path(&self.conf)); }
}
