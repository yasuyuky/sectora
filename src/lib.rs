extern crate toml;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate glob;
extern crate nix;

#[macro_use]
extern crate lazy_static;

use std::io::{Write, BufRead};
use std::ffi::CStr;
extern crate libc;

mod structs;
use structs::Config;
mod ghclient;
mod buffer;
use buffer::Buffer;
mod cstructs;
use cstructs::{Passwd, Spwd, Group};
mod runfiles;
mod statics;
use statics::{CLIENT, CONFIG};

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
    String::from(cstr.to_str().unwrap())
}

macro_rules! succeed {
    ($ret_struct_p:ident,$ret_struct:ident) => {{
        unsafe { *$ret_struct_p = $ret_struct };
        return libc::c_int::from(NssStatus::Success);
    }};
    ($ret_struct_p:ident,$ret_struct:ident,$finalize:expr) => {{
        $finalize;
        unsafe { *$ret_struct_p = $ret_struct };
        return libc::c_int::from(NssStatus::Success);
    }}
}

macro_rules! fail {
    ($ret_struct_p:ident, $return_val:expr) => {{
        unsafe { *$ret_struct_p = std::ptr::null_mut() };
        return $return_val;
    }}
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwnam_r(cnameptr: *const libc::c_char,
                                         pwptr: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwptrp: *mut *mut Passwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => fail!(pwptrp, libc::c_int::from(NssStatus::NotFound)),
    };
    if let Some(member) = team.members.get(&name) {
        match unsafe { (*pwptr).pack_args(&mut buffer, &member.login, member.id, team.id, &CONFIG) } {
            Ok(_) => succeed!(pwptrp, pwptr),
            Err(_) => fail!(pwptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(pwptrp, libc::c_int::from(NssStatus::NotFound))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwuid_r(uid: libc::uid_t,
                                         pwptr: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwptrp: *mut *mut Passwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => fail!(pwptrp, libc::c_int::from(NssStatus::NotFound)),
    };
    for member in team.members.values() {
        if uid == member.id as libc::uid_t {
            match unsafe { (*pwptr).pack_args(&mut buffer, &member.login, member.id, team.id, &CONFIG) } {
                Ok(_) => succeed!(pwptrp, pwptr),
                Err(_) => fail!(pwptrp, nix::Errno::ERANGE as libc::c_int),
            }
        }
    }
    fail!(pwptrp, libc::c_int::from(NssStatus::NotFound))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setpwent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    for member in team.members.values() {
        list_file.write(format!("{}\t{}\t{}\n", member.login, member.id, team.id).as_bytes())
                 .unwrap();
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwent_r(pwptr: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwptrp: *mut *mut Passwd)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => fail!(pwptrp, libc::c_int::from(NssStatus::Unavail)),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let id = words[1].parse::<u64>().unwrap();
        let gid = words[2].parse::<u64>().unwrap();
        match unsafe { (*pwptr).pack_args(&mut buffer, words[0], id, gid, &CONFIG) } {
            Ok(_) => succeed!(pwptrp, pwptr, runfiles::increment(idx, idx_file)),
            Err(_) => fail!(pwptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(pwptrp, libc::c_int::from(NssStatus::Unavail))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endpwent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspnam_r(cnameptr: *const libc::c_char,
                                         spptr: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         spptrp: *mut *mut Spwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => fail!(spptrp, libc::c_int::from(NssStatus::NotFound)),
    };
    if let Some(member) = team.members.get(&name) {
        match unsafe { (*spptr).pack_args(&mut buffer, &member.login, &CONFIG) } {
            Ok(_) => succeed!(spptrp, spptr),
            Err(_) => fail!(spptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(spptrp, libc::c_int::from(NssStatus::NotFound))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setspent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    for member in team.members.values() {
        list_file.write(format!("{}\t{}\n", member.login, member.id).as_bytes())
                 .unwrap();
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspent_r(spptr: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         spptrp: *mut *mut Spwd)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => fail!(spptrp, libc::c_int::from(NssStatus::Unavail)),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        match unsafe { (*spptr).pack_args(&mut buffer, words[0], &CONFIG) } {
            Ok(_) => succeed!(spptrp, spptr, runfiles::increment(idx, idx_file)),
            Err(_) => fail!(spptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(spptrp, libc::c_int::from(NssStatus::Unavail))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endspent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrgid_r(gid: libc::gid_t,
                                         grptr: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grptrp: *mut *mut Group)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => fail!(grptrp, libc::c_int::from(NssStatus::NotFound)),
    };
    let members: Vec<&str> = team.members.values().map(|m| m.login.as_str()).collect();
    if gid as u64 == team.id {
        match unsafe { (*grptr).pack_args(&mut buffer, &team.name, gid as u64, &members) } {
            Ok(_) => succeed!(grptrp, grptr),
            Err(_) => fail!(grptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(grptrp, libc::c_int::from(NssStatus::NotFound))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrnam_r(cnameptr: *const libc::c_char,
                                         grptr: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grptrp: *mut *mut Group)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => fail!(grptrp, libc::c_int::from(NssStatus::NotFound)),
    };
    let members: Vec<&str> = team.members.values().map(|m| m.login.as_str()).collect();
    if name == team.name {
        match unsafe { (*grptr).pack_args(&mut buffer, &team.name, team.id, &members) } {
            Ok(_) => succeed!(grptrp, grptr),
            Err(_) => fail!(grptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(grptrp, libc::c_int::from(NssStatus::NotFound))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setgrent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let team = match CLIENT.get_team() {
        Ok(team) => team,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let member_names = team.members.values().map(|x| x.login.as_str()).collect::<Vec<&str>>().join(" ");
    list_file.write(format!("{}\t{}\t{}\n", team.name, team.id, member_names).as_bytes())
             .unwrap();
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrent_r(grptr: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grptrp: *mut *mut Group)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => fail!(grptrp, libc::c_int::from(NssStatus::Unavail)),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let member_names: Vec<&str> = words[2].split(" ").collect();
        let gid = words[1].parse::<u64>().unwrap();
        match unsafe { (*grptr).pack_args(&mut buffer, words[0], gid, &member_names) } {
            Ok(_) => succeed!(grptrp, grptr, runfiles::increment(idx, idx_file)),
            Err(_) => fail!(grptrp, nix::Errno::ERANGE as libc::c_int),
        }
    }
    fail!(grptrp, libc::c_int::from(NssStatus::Unavail))
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endgrent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}
