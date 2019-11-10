use crate::applog;
use crate::error;
use crate::message::*;
use crate::statics::CONF_PATH;
use crate::structs::Config;
use std::os::unix::net::UnixDatagram;
use std::time::Duration;

#[derive(Debug)]
pub struct Connection {
    pub conf: Config,
    conn: UnixDatagram,
}

impl Connection {
    pub fn new(logid: &str) -> Result<Self, error::Error> {
        applog::init(Some("sectora"));
        log::debug!("{}", logid);
        let conf = Config::from_path(&CONF_PATH)?;
        let conn = Self::connect_daemon(&conf)?;
        Ok(Self { conf, conn })
    }

    fn socket_path(conf: &Config) -> String { format!("{}/{}", &conf.socket_dir, std::process::id()) }

    fn connect_daemon(conf: &Config) -> Result<UnixDatagram, error::Error> {
        let socket = UnixDatagram::bind(&Self::socket_path(conf))?;
        log::debug!("{:?}", socket);
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.connect(&conf.socket_path)?;
        Ok(socket)
    }

    pub fn communicate(&self, msg: ClientMessage) -> Result<DaemonMessage, error::Error> {
        self.conn.send(msg.to_string().as_bytes())?;
        let mut buf = [0u8; 4096];
        let recv_cnt = match self.conn.recv(&mut buf) {
            Ok(cnt) => cnt,
            Err(e) => {
                log::debug!("ERROR: failed to recv msg, {}", e);
                return Err(error::Error::from(e));
            }
        };
        let s = String::from_utf8(buf[..recv_cnt].to_vec()).unwrap();
        log::debug!("recieved: {}", s);
        Ok(s.parse::<DaemonMessage>()?)
    }
}

impl Drop for Connection {
    fn drop(&mut self) { std::fs::remove_file(&Self::socket_path(&self.conf)).unwrap_or_default(); }
}
