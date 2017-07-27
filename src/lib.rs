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

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwnam_r(cnameptr: *const libc::c_char,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwbufp: *mut *mut Passwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let (team, members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            match unsafe { (*pw).pack_args(&mut buffer, &member.login, member.id, team.id, &CONFIG) } {
                Ok(_) => {
                    unsafe { *pwbufp = pw };
                    return libc::c_int::from(NssStatus::Success);
                }
                Err(_) => {
                    unsafe { *pwbufp = std::ptr::null_mut() };
                    return nix::Errno::ERANGE as libc::c_int;
                }

            }
        }
        None => {
            unsafe { *pwbufp = std::ptr::null_mut() };
            libc::c_int::from(NssStatus::NotFound)
        }
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwuid_r(uid: libc::uid_t,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwbufp: *mut *mut Passwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let (team, members) = CLIENT.get_team_members().unwrap();
    for member in members.values() {
        if uid == member.id as libc::uid_t {
            match unsafe { (*pw).pack_args(&mut buffer, &member.login, member.id, team.id, &CONFIG) } {
                Ok(_) => {
                    unsafe { *pwbufp = pw };
                    return libc::c_int::from(NssStatus::Success);
                }
                Err(_) => {
                    unsafe { *pwbufp = std::ptr::null_mut() };
                    return nix::Errno::ERANGE as libc::c_int;
                }
            }
        }
    }
    unsafe { *pwbufp = std::ptr::null_mut() };
    libc::c_int::from(NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setpwent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let (team, members) = match CLIENT.get_team_members() {
        Ok((team, members)) => (team, members),
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    for member in members.values() {
        list_file.write(format!("{}\t{}\t{}\n", member.login, member.id, team.id).as_bytes())
                 .unwrap();
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwent_r(pwbuf: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         pwbufp: *mut *mut Passwd)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Unavail),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let id = words[1].parse::<u64>().unwrap();
        let gid = words[2].parse::<u64>().unwrap();
        match unsafe { (*pwbuf).pack_args(&mut buffer, words[0], id, gid, &CONFIG) } {
            Ok(_) => {
                runfiles::increment(idx, idx_file);
                unsafe { *pwbufp = pwbuf };
                return libc::c_int::from(NssStatus::Success);
            }
            Err(_) => {
                unsafe { *pwbufp = std::ptr::null_mut() };
                return nix::Errno::ERANGE as libc::c_int;
            }
        }
    }
    libc::c_int::from(NssStatus::Unavail)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endpwent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspnam_r(cnameptr: *const libc::c_char,
                                         spwd: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         spbufp: *mut *mut Spwd)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let (_, members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            match unsafe { (*spwd).pack_args(&mut buffer, &member.login, &CONFIG) } {
                Ok(_) => {
                    unsafe { *spbufp = spwd };
                    libc::c_int::from(NssStatus::Success)
                }
                Err(_) => {
                    unsafe { *spbufp = std::ptr::null_mut() };
                    nix::Errno::ERANGE as libc::c_int
                }
            }
        }
        None => {
            unsafe { *spbufp = std::ptr::null_mut() };
            libc::c_int::from(NssStatus::NotFound)
        }
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setspent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let members = match CLIENT.get_team_members() {
        Ok((_, members)) => members,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    for member in members.values() {
        list_file.write(format!("{}\t{}\n", member.login, member.id).as_bytes())
                 .unwrap();
    }
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspent_r(spbuf: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         spbufp: *mut *mut Spwd)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Unavail),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        match unsafe { (*spbuf).pack_args(&mut buffer, words[0], &CONFIG) } {
            Ok(_) => {
                runfiles::increment(idx, idx_file);
                unsafe { *spbufp = spbuf };
                return libc::c_int::from(NssStatus::Success);
            }
            Err(_) => {
                unsafe { *spbufp = std::ptr::null_mut() };
                return nix::Errno::ERANGE as libc::c_int;
            }
        }
    }
    libc::c_int::from(NssStatus::Unavail)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endspent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrgid_r(gid: libc::gid_t,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grbufp: *mut *mut Group)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let (team, members) = CLIENT.get_team_members().unwrap();
    let members: Vec<&str> = members.values().map(|m| m.login.as_str()).collect();
    if gid as u64 == team.id {
        match unsafe { (*group).pack_args(&mut buffer, &team.name, gid as u64, &members) } {
            Ok(_) => {
                unsafe { *grbufp = group };
                libc::c_int::from(NssStatus::Success)
            }
            Err(_) => {
                unsafe { *grbufp = std::ptr::null_mut() };
                nix::Errno::ERANGE as libc::c_int
            }
        }
    } else {
        unsafe { *grbufp = std::ptr::null_mut() };
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrnam_r(cnameptr: *const libc::c_char,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grbufp: *mut *mut Group)
                                         -> libc::c_int {
    let mut buffer = Buffer::new(buf, buflen);
    let name = string_from(cnameptr);
    let (team, members) = CLIENT.get_team_members().unwrap();
    let members: Vec<&str> = members.values().map(|m| m.login.as_str()).collect();
    if name == team.name {
        match unsafe { (*group).pack_args(&mut buffer, &team.name, team.id, &members) } {
            Ok(_) => {
                unsafe { *grbufp = group };
                libc::c_int::from(NssStatus::Success)
            }
            Err(_) => {
                unsafe { *grbufp = std::ptr::null_mut() };
                nix::Errno::ERANGE as libc::c_int
            }
        }
    } else {
        unsafe { *grbufp = std::ptr::null_mut() };
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setgrent() -> libc::c_int {
    let mut list_file = match runfiles::create() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let (team, members) = match CLIENT.get_team_members() {
        Ok(team_members) => team_members,
        Err(_) => return libc::c_int::from(NssStatus::Success),
    };
    let member_names = members.values().map(|x| x.login.as_str()).collect::<Vec<&str>>().join(" ");
    list_file.write(format!("{}\t{}\t{}\n", team.name, team.id, member_names).as_bytes())
             .unwrap();
    libc::c_int::from(NssStatus::Success)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrent_r(grbuf: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         grbufp: *mut *mut Group)
                                         -> libc::c_int {
    let (idx, idx_file, list) = match runfiles::open() {
        Ok(ret) => ret,
        Err(_) => return libc::c_int::from(NssStatus::Unavail),
    };
    if let Some(Ok(line)) = list.lines().nth(idx) {
        let mut buffer = Buffer::new(buf, buflen);
        let words: Vec<&str> = line.split("\t").collect();
        let member_names: Vec<&str> = words[2].split(" ").collect();
        let gid = words[1].parse::<u64>().unwrap();
        match unsafe { (*grbuf).pack_args(&mut buffer, words[0], gid, &member_names) } {
            Ok(_) => {
                runfiles::increment(idx, idx_file);
                unsafe { *grbufp = grbuf };
                return libc::c_int::from(NssStatus::Success);
            }
            Err(_) => {
                unsafe { *grbufp = std::ptr::null_mut() };
                return nix::Errno::ERANGE as libc::c_int;
            }
        }
    }
    libc::c_int::from(NssStatus::Unavail)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_endgrent() -> libc::c_int {
    runfiles::cleanup().unwrap_or(());
    libc::c_int::from(NssStatus::Success)
}
