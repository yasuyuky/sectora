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
use message::*;
use nix::errno::Errno;
use statics::CONF_PATH;
use std::ffi::CStr;
use std::os::unix::net::UnixDatagram;
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

macro_rules! get_or_again {
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

fn connect_daemon(conf: &Config) -> Result<(UnixDatagram, String), error::Error> {
    let client_socket_path = format!("{}/{}", &conf.socket_dir, std::process::id());
    let socket = match UnixDatagram::bind(&client_socket_path) {
        Ok(socket) => socket,
        Err(e) => {
            log::debug!("ERROR: failed to bind socket, {}", e);
            return Err(error::Error::from(e));
        }
    };
    log::debug!("{:?}", socket);
    match socket.set_read_timeout(Some(Duration::from_secs(5))) {
        Ok(_) => (),
        Err(e) => {
            log::debug!("ERROR: failed to set timeout, {}", e);
            return Err(error::Error::from(e));
        }
    };
    match socket.connect(&conf.socket_path) {
        Ok(_) => (),
        Err(e) => {
            log::debug!("ERROR: failed to connect socket, {}", e);
            return Err(error::Error::from(e));
        }
    };
    Ok((socket, client_socket_path))
}

fn send_recv(conn: &(UnixDatagram, String), msg: ClientMessage) -> Result<DaemonMessage, error::Error> {
    match conn.0.send(msg.to_string().as_bytes()) {
        Ok(_) => (),
        Err(e) => {
            log::debug!("ERROR: failed to send msg, {}", e);
            return Err(error::Error::from(e));
        }
    };
    let mut buf = [0u8; 4096];
    let recv_cnt = match conn.0.recv(&mut buf) {
        Ok(cnt) => cnt,
        Err(e) => {
            log::debug!("ERROR: failed to recv msg, {}", e);
            return Err(error::Error::from(e));
        }
    };
    let s = String::from_utf8(buf[..recv_cnt].to_vec()).unwrap();
    log::debug!("recieved: {}", s);
    std::fs::remove_file(&conn.1).unwrap_or_default();
    match s.parse::<DaemonMessage>() {
        Ok(msg) => Ok(msg),
        Err(e) => {
            log::debug!("ERROR: failed to parse msg, {:?}", e);
            Err(error::Error::from(e))
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getpwnam_r(cnameptr: *const libc::c_char, pwptr: *mut Passwd,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getpwnam_r");
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf), errnop);
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Pw(Pw::Nam(name))), errnop);
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conf) } {
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
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getpwuid_r");
    let mut buffer = Buffer::new(buf, buflen);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf), errnop);
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Pw(Pw::Uid(uid as u64))), errnop);
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_setpwent() -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_setpwent");
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Pw(Pw::Ent(Ent::Set(std::process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getpwent_r(pwptr: *mut Passwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getpwent_r");
    let mut buffer = Buffer::new(buf, buflen);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Pw(Pw::Ent(Ent::Get(std::process::id())))));
    if let DaemonMessage::Pw { login, uid, gid } = msg {
        match { (*pwptr).pack_args(&mut buffer, &login, uid, gid, &conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_endpwent() -> libc::c_int {
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Pw(Pw::Ent(Ent::End(std::process::id())))));
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
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getspnam_r");
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf), errnop);
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Sp(Sp::Nam(name))), errnop);
    if let DaemonMessage::Sp { login } = msg {
        match { (*spptr).pack_args(&mut buffer, &login, &conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_setspent() -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_setspent");
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Sp(Sp::Ent(Ent::Set(std::process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getspent_r(spptr: *mut Spwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getspent_r");
    let mut buffer = Buffer::new(buf, buflen);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Sp(Sp::Ent(Ent::Get(std::process::id())))));
    if let DaemonMessage::Sp { login } = msg {
        match { (*spptr).pack_args(&mut buffer, &login, &conf) } {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_endspent() -> libc::c_int {
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Sp(Sp::Ent(Ent::End(std::process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getgrgid_r(gid: libc::gid_t, grptr: *mut Group, buf: *mut libc::c_char,
                                                 buflen: libc::size_t, errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getgrgid_r");
    let mut buffer = Buffer::new(buf, buflen);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf), errnop);
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Gr(Gr::Gid(gid as u64))), errnop);
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
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getgrnam_r");
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf), errnop);
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Gr(Gr::Nam(name))), errnop);
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
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_setgrent");
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Gr(Gr::Ent(Ent::Set(std::process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub unsafe extern "C" fn _nss_sectora_getgrent_r(grptr: *mut Group, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    applog::init(Some("libsectora"));
    log::debug!("_nss_sectora_getgrent_r");
    let mut buffer = Buffer::new(buf, buflen);
    let conf = get_or_again!(Config::from_path(&CONF_PATH), errnop);
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Gr(Gr::Ent(Ent::Get(std::process::id())))));
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
    let conf = get_or_again!(Config::from_path(&CONF_PATH));
    let conn = get_or_again!(connect_daemon(&conf));
    let msg = get_or_again!(send_recv(&conn, ClientMessage::Gr(Gr::Ent(Ent::End(std::process::id())))));
    if let DaemonMessage::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}
