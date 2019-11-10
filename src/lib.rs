#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate nix;
#[macro_use]
extern crate serde;
extern crate serde_json;
extern crate toml;

mod applog;
mod buffer;
mod cstructs;
mod error;
mod message;
mod statics;
mod structs;

use buffer::Buffer;
use cstructs::{Group, Passwd, Spwd};
use message::ClientMessage as CMsg;
use message::*;
use nix::errno::Errno;
use statics::CONF_PATH;
use std::ffi::CStr;
use std::os::unix::net::UnixDatagram;
use std::process;
use std::string::String;
use std::time::Duration;
use structs::Config;

#[allow(dead_code)]
enum NssStatus {
    TryAgain,
    Unavail,
    NotFound,
    Success,
}

impl From<NssStatus> for libc::c_int {
    fn from(status: NssStatus) -> libc::c_int {
        match status {
            NssStatus::TryAgain => -2,
            NssStatus::Unavail => -1,
            NssStatus::NotFound => 0,
            NssStatus::Success => 1,
        }
    }
}

fn string_from(cstrptr: *const libc::c_char) -> String {
    let cstr: &CStr = unsafe { CStr::from_ptr(cstrptr) };
    String::from(cstr.to_str().unwrap_or(""))
}

macro_rules! succeed {
    () => {{
        log::debug!("Success!");
        return libc::c_int::from(NssStatus::Success);
    }};
}

macro_rules! fail {
    ($err_no_p:ident, $err_no:expr, $return_val:expr) => {{
        *$err_no_p = $err_no as libc::c_int;
        log::debug!("Faill!");
        return libc::c_int::from($return_val);
    }};
}

macro_rules! try_unwrap {
    ($getter:expr) => {{
        match $getter {
            Ok(ret) => {
                log::debug!("Ok: {:?}", ret);
                ret
            }
            Err(e) => {
                log::debug!("failed (will retry): {:?}", e);
                return libc::c_int::from(NssStatus::TryAgain);
            }
        }
    }};
    ($getter:expr, $err_no_p:ident) => {{
        match $getter {
            Ok(ret) => ret,
            Err(e) => {
                log::debug!("failed (will retry): {:?}", e);
                *$err_no_p = Errno::EAGAIN as libc::c_int;
                return libc::c_int::from(NssStatus::TryAgain);
            }
        }
    }};
}

#[derive(Debug)]
struct Connection {
    conf: Config,
    conn: UnixDatagram,
}

impl Connection {
    fn new(logid: &str) -> Result<Self, error::Error> {
        applog::init(Some("libsectora"));
        log::debug!("{}", logid);
        let conf = Config::from_path(&CONF_PATH)?;
        let conn = Self::connect_daemon(&conf)?;
        Ok(Self { conf, conn })
    }

    fn socket_path(conf: &Config) -> String { format!("{}/{}", &conf.socket_dir, process::id()) }

    fn connect_daemon(conf: &Config) -> Result<UnixDatagram, error::Error> {
        let socket = UnixDatagram::bind(&Self::socket_path(conf))?;
        log::debug!("{:?}", socket);
        socket.set_read_timeout(Some(Duration::from_secs(5)))?;
        socket.connect(&conf.socket_path)?;
        Ok(socket)
    }

    fn communicate(&self, msg: CMsg) -> Result<DaemonMessage, error::Error> {
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

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getpwnam_r(cnameptr: *const libc::c_char, pwptr: *mut Passwd,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Nam(name))), errnop);
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conn.conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getpwuid_r(uid: libc::uid_t, pwptr: *mut Passwd, buf: *mut libc::c_char,
                                                 buflen: libc::size_t, errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwuid_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Uid(uid as u64))), errnop);
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conn.conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_setpwent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setpwent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::Set(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getpwent_r(pwptr: *mut Passwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::Get(process::id())))), errnop);
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conn.conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_endpwent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endpwent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::End(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getspnam_r(cnameptr: *const libc::c_char, spptr: *mut Spwd,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getspnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Nam(name))), errnop);
    if let DaemonMessage::Sp { login } = msg {
        match { (*spptr).pack_args(&mut buffer, &login, &conn.conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_setspent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setspent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::Set(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getspent_r(spptr: *mut Spwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getspent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::Get(process::id())))), errnop);
    if let DaemonMessage::Sp { login } = msg {
        match { (*spptr).pack_args(&mut buffer, &login, &conn.conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_endspent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endspent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::End(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getgrgid_r(gid: libc::gid_t, grptr: *mut Group, buf: *mut libc::c_char,
                                                 buflen: libc::size_t, errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrgid_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Gid(gid as u64))), errnop);
    if let DaemonMessage::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match { (*grptr).pack_args(&mut buffer, &sector.get_group(), u64::from(gid), &members) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getgrnam_r(cnameptr: *const libc::c_char, grptr: *mut Group,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Nam(name))), errnop);
    if let DaemonMessage::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match { (*grptr).pack_args(&mut buffer, &sector.get_group(), sector.get_gid(), &members) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_setgrent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setgrent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::Set(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getgrent_r(grptr: *mut Group, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::Get(process::id())))), errnop);
    if let DaemonMessage::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match { (*grptr).pack_args(&mut buffer, &sector.get_group(), sector.get_gid(), &members) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_endgrent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endgrent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::End(process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}
