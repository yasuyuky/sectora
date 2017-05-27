extern crate toml;
extern crate reqwest;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate glob;

#[macro_use]
extern crate lazy_static;

use std::ffi::{CStr,CString};
extern crate libc;

mod structs;
mod ghclient;
use ghclient::GithubClient;

lazy_static! {
    static ref CLIENT:GithubClient = GithubClient::new(
        std::env::var("GHTEAMAUTH_CONFIG")
                 .unwrap_or(String::from("/etc/ghteam-auth.conf"))
                 .as_str()
    ).unwrap();
}

#[allow(dead_code)]
enum NssStatus {
    TryAgain,
    Unavail,
    NotFound,
    Success,
}

impl From<NssStatus> for libc::c_int {
    fn from(status:NssStatus) -> libc::c_int {
        match status {
            NssStatus::TryAgain => -2,
            NssStatus::Unavail  => -1,
            NssStatus::NotFound => 0,
            NssStatus::Success  => 1,
        }
    }
}

#[derive(Debug)]
struct Buffer {
    buf: *mut libc::c_char,
    offset: isize,
    buflen: libc::size_t,
}

impl Buffer {
    fn new(buf: *mut libc::c_char,buflen: libc::size_t) -> Self { Self{buf,offset:0,buflen} }

    fn write(&mut self, data: *const libc::c_char, len: usize) -> *mut libc::c_char {
        unsafe {
            let pos = self.buf.offset(self.offset);
            std::ptr::copy(data as *mut i8, pos, len);
            self.offset += len as isize;
            self.buflen -= len as libc::size_t; //TODO: range checking
            pos
        }
    }

    fn write_string(&mut self, s: &str) -> *mut libc::c_char {
        let cs = CString::new(s).unwrap();
        self.write(cs.as_ptr(), s.len() + 1)
    }
}

#[repr(C)]
pub struct Passwd {
    name:   *mut libc::c_char,
    passwd: *mut libc::c_char,
    uid:    libc::uid_t,
    gid:    libc::gid_t,
    gecos:  *mut libc::c_char,
    dir:    *mut libc::c_char,
    shell:  *mut libc::c_char,
}

impl Passwd {
    fn pack(&mut self,
            buf:&mut Buffer,
            name:&str,
            passwd:&str,
            uid:libc::uid_t,
            gid:libc::gid_t,
            gecos:&str,
            dir:&str,
            shell:&str) {
        self.name = buf.write_string(name);
        self.passwd = buf.write_string(passwd);
        self.dir = buf.write_string(dir);
        self.shell = buf.write_string(shell);
        self.gecos = buf.write_string(gecos);
        self.uid = uid;
        self.gid = gid;
    }

}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwnam_r(cnameptr: *const libc::c_char,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (_,members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            unsafe {
                (*pw).pack(
                    &mut buffer,
                    &member.login,
                    "x",
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    "",
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
                );
            }
            libc::c_int::from(NssStatus::Success)
        },
        None => libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwuid_r(uid: libc::uid_t,
                                         pw: *mut Passwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let (_,members) = CLIENT.get_team_members().unwrap();
    for member in members.values() {
        if uid == member.id as libc::uid_t {
            unsafe {
                (*pw).pack(
                    &mut buffer,
                    &member.login,
                    "x",
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    "",
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
                );
            }
            return libc::c_int::from(NssStatus::Success);
        }
    }
    libc::c_int::from(NssStatus::NotFound)
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setpwent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getpwent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endpwent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }


#[repr(C)]
pub struct Spwd {
    namp:   *mut libc::c_char,
    pwdp:   *mut libc::c_char,
    lstchg:      libc::c_long,
    min:         libc::c_long,
    max:         libc::c_long,
    warn:        libc::c_long,
    inact:       libc::c_long,
    expire:      libc::c_long,
    flag:        libc::c_ulong,
}

impl Spwd {
    fn pack(&mut self,
            buf:&mut Buffer,
            namp:&str,
            pwdp:&str,
            lstchg:libc::c_long,
            min:libc::c_long,
            max:libc::c_long,
            warn:libc::c_long,
            inact:libc::c_long,
            expire:libc::c_long,
            flag:libc::c_ulong,
        ) {
        self.namp = buf.write_string(namp);
        self.pwdp = buf.write_string(pwdp);
        self.lstchg = lstchg;
        self.min = min;
        self.max = max;
        self.warn = warn;
        self.inact = inact;
        self.expire = expire;
        self.flag = flag;
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspnam_r(cnameptr: *const libc::c_char,
                                         spwd: *mut Spwd,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (_,members) = CLIENT.get_team_members().unwrap();
    match members.get(&name) {
        Some(member) => {
            unsafe {
                (*spwd).pack(
                    &mut buffer,
                    &member.login,
                    "!!",
                    -1,-1,-1,-1,-1,-1,0
                );
            }
            libc::c_int::from(NssStatus::Success)
        },
        None => libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setspent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getspent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endspent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }


#[repr(C)]
pub struct Group {
    name:   *mut libc::c_char,
    passwd: *mut libc::c_char,
    gid:         libc::gid_t,
    mem:    *mut *mut libc::c_char,
}

impl Group {
    fn pack(&mut self,
            buf:&mut Buffer,
            name:&str,
            passwd:&str,
            gid:libc::gid_t,
            mem:&Vec<String>){
        self.name = buf.write_string(name);
        self.passwd = buf.write_string(passwd);
        self.gid = gid;
        let mut ptrs = Vec::<*mut libc::c_char>::new();
        for m in mem {
            ptrs.push(buf.write_string(&m));
        }
        unsafe {
            self.mem = buf.buf.offset(buf.offset) as *mut *mut libc::c_char;
            for (i,p) in ptrs.iter().enumerate() {
                *(self.mem.offset(i as isize)) = *p;
            }
            *(self.mem.offset(ptrs.len() as isize)) = 0x0 as *mut libc::c_char;
        }
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrgid_r(gid: libc::gid_t,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let (team,members) = CLIENT.get_team_members().unwrap();
    let members:Vec<String> = members.values().map(|m| m.login.clone()).collect::<Vec<String>>();
    if gid == CLIENT.conf.gid as libc::gid_t {
        unsafe {
            (*group).pack(
                &mut buffer,
                &team.name,
                "x",
                gid,
                &members
            )
        }
        libc::c_int::from(NssStatus::Success)
    } else {
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrnam_r(cnameptr: *const libc::c_char,
                                         group: *mut Group,
                                         buf: *mut libc::c_char,
                                         buflen: libc::size_t,
                                         _: *mut libc::c_int) -> libc::c_int {
    let mut buffer = Buffer::new(buf,buflen);
    let cname: &CStr = unsafe {CStr::from_ptr(cnameptr)};
    let name = String::from(cname.to_str().unwrap());
    let (team,members) = CLIENT.get_team_members().unwrap();
    let members:Vec<String> = members.values().map(|m| m.login.clone()).collect::<Vec<String>>();
    if name == CLIENT.conf.team {
        unsafe {
            (*group).pack(
                &mut buffer,
                &team.name,
                "x",
                team.id as libc::gid_t,
                &members
            )
        }
        libc::c_int::from(NssStatus::Success)
    } else {
        libc::c_int::from(NssStatus::NotFound)
    }
}

#[no_mangle]
pub extern "C" fn _nss_ghteam_setgrent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_getgrent_r() -> libc::c_int { libc::c_int::from(NssStatus::Unavail) }

#[no_mangle]
pub extern "C" fn _nss_ghteam_endgrent() -> libc::c_int { libc::c_int::from(NssStatus::Success) }
