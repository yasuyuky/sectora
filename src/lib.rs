extern crate futures;
extern crate glob;
extern crate hyper;
extern crate hyper_rustls;
#[macro_use]
extern crate lazy_static;
extern crate libc;
extern crate nix;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate toml;

mod buffer;
mod cstructs;
mod ghclient;
mod runfiles;
mod statics;
mod structs;

use buffer::Buffer;
use cstructs::{Group, Passwd, Spwd};
use ghclient::GithubClient;
use nix::errno::Errno;
use statics::CONF_PATH;
use std::ffi::CStr;
use std::io::{BufRead, Write};
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
        return libc::c_int::from(NssStatus::Success);
    }};
    ($finalize:expr) => {{
        $finalize;
        return libc::c_int::from(NssStatus::Success);
    }};
}

macro_rules! fail {
    ($err_no_p:ident, $err_no:expr, $return_val:expr) => {{
        unsafe { *$err_no_p = $err_no as libc::c_int };
        return libc::c_int::from($return_val);
    }};
}

macro_rules! get_or_again {
    ($getter:expr) => {{
        match $getter {
            Ok(ret) => ret,
            Err(_) => return libc::c_int::from(NssStatus::TryAgain),
        }
    }};
    ($getter:expr, $err_no_p:ident) => {{
        match $getter {
            Ok(ret) => ret,
            Err(_) => {
                unsafe { *$err_no_p = Errno::EAGAIN as libc::c_int };
                return libc::c_int::from(NssStatus::TryAgain);
            }
        }
    }};
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getpwnam_r(cnameptr: *const libc::c_char, pwptr: *mut Passwd, buf: *mut libc::c_char,
                                          buflen: libc::size_t, errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    let sectors = get_or_again!(client.get_sectors(), errnop);
    for sector in sectors {
        if let Some(member) = sector.members.get(&name) {
            match unsafe { (*pwptr).pack_args(&mut buffer, &member.login, member.id, sector.get_gid(), &client.conf) } {
                Ok(_) => succeed!(),
                Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
            }
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getpwuid_r(uid: libc::uid_t, pwptr: *mut Passwd, buf: *mut libc::c_char,
                                          buflen: libc::size_t, errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    let sectors = get_or_again!(client.get_sectors(), errnop);
    for sector in sectors {
        for member in sector.members.values() {
            if uid == member.id as libc::uid_t {
                match unsafe {
                          (*pwptr).pack_args(&mut buffer, &member.login, member.id, sector.get_gid(), &client.conf)
                      } {
                    Ok(_) => succeed!(),
                    Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
                }
            }
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_setpwent() -> libc::c_int {
    let mut list_file = get_or_again!(runfiles::create());
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))));
    let sectors = get_or_again!(client.get_sectors());
    for sector in sectors {
        for member in sector.members.values() {
            list_file.write(format!("{}\t{}\t{}\n", member.login, member.id, sector.get_gid()).as_bytes())
                     .unwrap();
        }
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getpwent_r(pwptr: *mut Passwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                          errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let (idx, idx_file, list) = get_or_again!(runfiles::open(), errnop);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let id = words[1].parse::<u64>().expect("parse id");
        let gid = words[2].parse::<u64>().expect("parse gid");
        match unsafe { (*pwptr).pack_args(&mut buffer, words[0], id, gid, &client.conf) } {
            Ok(_) => succeed!(runfiles::increment(idx, idx_file)),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_endpwent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getspnam_r(cnameptr: *const libc::c_char, spptr: *mut Spwd, buf: *mut libc::c_char,
                                          buflen: libc::size_t, errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    let sectors = get_or_again!(client.get_sectors(), errnop);
    for sector in sectors {
        if let Some(member) = sector.members.get(&name) {
            match unsafe { (*spptr).pack_args(&mut buffer, &member.login, &client.conf) } {
                Ok(_) => succeed!(),
                Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
            }
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_setspent() -> libc::c_int {
    let mut list_file = get_or_again!(runfiles::create());
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))));
    let sectors = get_or_again!(client.get_sectors());
    for sector in sectors {
        for member in sector.members.values() {
            list_file.write(format!("{}\t{}\n", member.login, member.id).as_bytes())
                     .unwrap();
        }
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getspent_r(spptr: *mut Spwd, buf: *mut libc::c_char, buflen: libc::size_t,
                                          errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let (idx, idx_file, list) = get_or_again!(runfiles::open(), errnop);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        match unsafe { (*spptr).pack_args(&mut buffer, words[0], &client.conf) } {
            Ok(_) => succeed!(runfiles::increment(idx, idx_file)),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_endspent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getgrgid_r(gid: libc::gid_t, grptr: *mut Group, buf: *mut libc::c_char,
                                          buflen: libc::size_t, errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    let sectors = get_or_again!(client.get_sectors(), errnop);
    for sector in sectors {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        if gid as u64 == sector.get_gid() {
            match unsafe { (*grptr).pack_args(&mut buffer, &sector.get_group(), gid as u64, &members) } {
                Ok(_) => succeed!(),
                Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
            }
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getgrnam_r(cnameptr: *const libc::c_char, grptr: *mut Group, buf: *mut libc::c_char,
                                          buflen: libc::size_t, errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))), errnop);
    let sectors = get_or_again!(client.get_sectors(), errnop);
    for sector in sectors {
        let members: Vec<&str> = sector.members.values().map(|m| m.login.as_str()).collect();
        if name == sector.get_group() {
            match unsafe { (*grptr).pack_args(&mut buffer, &sector.get_group(), sector.get_gid(), &members) } {
                Ok(_) => succeed!(),
                Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
            }
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_setgrent() -> libc::c_int {
    let mut list_file = get_or_again!(runfiles::create());
    let client = get_or_again!(Config::new(&CONF_PATH).and_then(|c| Ok(GithubClient::new(&c))));
    let sectors = get_or_again!(client.get_sectors());
    for sector in sectors {
        let member_names = sector.members
                                 .values()
                                 .map(|x| x.login.as_str())
                                 .collect::<Vec<&str>>()
                                 .join(" ");
        list_file.write(format!("{}\t{}\t{}\n", sector.get_group(), sector.get_gid(), member_names).as_bytes())
                 .unwrap();
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_getgrent_r(grptr: *mut Group, buf: *mut libc::c_char, buflen: libc::size_t,
                                          errnop: *mut libc::c_int)
                                          -> libc::c_int {
    let (idx, idx_file, list) = get_or_again!(runfiles::open(), errnop);
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let member_names: Vec<&str> = words[2].split(" ").collect();
        let gid = words[1].parse::<u64>().expect("parse gid");
        match unsafe { (*grptr).pack_args(&mut buffer, words[0], gid, &member_names) } {
            Ok(_) => succeed!(runfiles::increment(idx, idx_file)),
            Err(_) => fail!(errnop, Errno::ERANGE, NssStatus::TryAgain),
        }
    }
    fail!(errnop, Errno::ENOENT, NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_sectora_endgrent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}
