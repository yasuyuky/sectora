mod applog;
mod buffer;
mod connection;
mod cstructs;
mod error;
mod message;
mod structs;

use buffer::Buffer;
use connection::Connection;
use cstructs::{Group, Passwd, Spwd};
use message::{ClientMessage as CMsg, DaemonMessage as DMsg, Ent, Gr, Pw, Sp};
use nix::errno::Errno;
use std::ffi::CStr;
use std::process;
use std::string::String;

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

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getpwnam_r(cnameptr: *const libc::c_char, pwptr: *mut Passwd,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Nam(string_from(cnameptr)))), errnop);
    if let DMsg::Pw { login,
                      uid,
                      gid,
                      home,
                      sh, } = msg
    {
        match (*pwptr).pack_args(&mut buffer, &login, uid, gid, &home, &sh) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getpwuid_r(uid: libc::uid_t, pwptr: *mut Passwd, buf: *mut libc::c_char,
                                                 buflen: libc::size_t, errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwuid_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Uid(uid as u64))), errnop);
    if let DMsg::Pw { login,
                      uid,
                      gid,
                      home,
                      sh, } = msg
    {
        match (*pwptr).pack_args(&mut buffer, &login, uid, gid, &home, &sh) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_setpwent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setpwent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::Set(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getpwent_r(pwptr: *mut Passwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getpwent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::Get(process::id())))), errnop);
    if let DMsg::Pw { login,
                      uid,
                      gid,
                      home,
                      sh, } = msg
    {
        match (*pwptr).pack_args(&mut buffer, &login, uid, gid, &home, &sh) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_endpwent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endpwent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Pw(Pw::Ent(Ent::End(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getspnam_r(cnameptr: *const libc::c_char, spptr: *mut Spwd,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getspnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Nam(string_from(cnameptr)))), errnop);
    if let DMsg::Sp { login, pass } = msg {
        match (*spptr).pack_args(&mut buffer, &login, &pass) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_setspent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setspent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::Set(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getspent_r(spptr: *mut Spwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getspent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::Get(process::id())))), errnop);
    if let DMsg::Sp { login, pass } = msg {
        match (*spptr).pack_args(&mut buffer, &login, &pass) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_endspent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endspent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Sp(Sp::Ent(Ent::End(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getgrgid_r(gid: libc::gid_t, grptr: *mut Group, buf: *mut libc::c_char,
                                                 buflen: libc::size_t, errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrgid_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Gid(gid as u64))), errnop);
    if let DMsg::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match (*grptr).pack_args(&mut buffer, &sector.get_group(), u64::from(gid), &members) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getgrnam_r(cnameptr: *const libc::c_char, grptr: *mut Group,
                                                 buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrnam_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Nam(string_from(cnameptr)))), errnop);
    if let DMsg::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match (*grptr).pack_args(&mut buffer, &sector.get_group(), sector.get_gid(), &members) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_setgrent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_setgrent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::Set(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::Success)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_getgrent_r(grptr: *mut Group, buf: *mut libc::c_char, buflen: libc::size_t,
                                                 errnop: *mut libc::c_int)
                                                 -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let conn = try_unwrap!(Connection::new("_nss_sectora_getgrent_r"), errnop);
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::Get(process::id())))), errnop);
    if let DMsg::Gr { sector } = msg {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        match (*grptr).pack_args(&mut buffer, &sector.get_group(), sector.get_gid(), &members) {
            Ok(_) => succeed!(),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

/// # Safety
///
/// This function intended to be called from nss
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _nss_sectora_endgrent() -> libc::c_int {
    let conn = try_unwrap!(Connection::new("_nss_sectora_endgrent"));
    let msg = try_unwrap!(conn.communicate(CMsg::Gr(Gr::Ent(Ent::End(process::id())))));
    if let DMsg::Success = msg {
        return libc::c_int::from(NssStatus::Success);
    }
    libc::c_int::from(NssStatus::TryAgain)
}
