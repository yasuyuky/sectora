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

use std::io::{Error, ErrorKind};

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

    fn write(&mut self, data: *const libc::c_char, len: usize) -> Result<*mut libc::c_char, Error> {
        if self.buflen < len as libc::size_t {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "ERANGE"));
        }
        unsafe {
            let pos = self.buf.offset(self.offset);
            std::ptr::copy(data as *mut i8, pos, len);
            self.offset += len as isize;
            self.buflen -= len as libc::size_t;
            Ok(pos)
        }
    }

    fn add_pointers(&mut self, ptrs:&Vec<*mut libc::c_char>)
        -> Result<*mut *mut libc::c_char, Error> {
        use std::mem::size_of;
        let step = std::cmp::max(size_of::<*mut libc::c_char>()/size_of::<libc::c_char>(), 1);
        if self.buflen < (ptrs.len()+1) * step as libc::size_t {
            return Err(Error::new(ErrorKind::AddrNotAvailable, "ERANGE"));
        }
        unsafe {
            let mem = self.buf.offset(self.offset) as *mut *mut libc::c_char;
            for (i,p) in ptrs.iter().enumerate() {
                *(mem.offset(i as isize)) = *p;
                self.offset += step as isize;
                self.buflen -= step as libc::size_t;
            }
            *(mem.offset(ptrs.len() as isize)) = std::ptr::null_mut::<libc::c_char>();
            Ok(mem)
        }
    }

    fn write_string(&mut self, s: &str) -> Result<*mut libc::c_char, Error> {
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
            shell:&str) -> Result<(), Error> {
        self.name = buf.write_string(name)?;
        self.passwd = buf.write_string(passwd)?;
        self.dir = buf.write_string(dir)?;
        self.shell = buf.write_string(shell)?;
        self.gecos = buf.write_string(gecos)?;
        self.uid = uid;
        self.gid = gid;
        Ok(())
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
            match unsafe{(*pw).pack(
                    &mut buffer,
                    &member.login,
                    "x",
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    "",
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
            )} {
                Ok(_) => libc::c_int::from(NssStatus::Success),
                Err(_) => nix::Errno::ERANGE as libc::c_int
            }
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
            match unsafe {(*pw).pack(
                    &mut buffer,
                    &member.login,
                    "x",
                    member.id as libc::uid_t,
                    CLIENT.conf.gid as libc::gid_t,
                    "",
                    &CLIENT.conf.home.replace("{}",member.login.as_str()),
                    &CLIENT.conf.sh,
                )} {
                Ok(_) => return libc::c_int::from(NssStatus::Success),
                Err(_) => return nix::Errno::ERANGE as libc::c_int
            }
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
        ) -> Result<(), Error> {
        self.namp = buf.write_string(namp)?;
        self.pwdp = buf.write_string(pwdp)?;
        self.lstchg = lstchg;
        self.min = min;
        self.max = max;
        self.warn = warn;
        self.inact = inact;
        self.expire = expire;
        self.flag = flag;
        Ok(())
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
            match unsafe {(*spwd).pack(
                    &mut buffer,
                    &member.login,
                    "!!",
                    -1,-1,-1,-1,-1,-1,0
                )} {
                Ok(_) => libc::c_int::from(NssStatus::Success),
                Err(_) => nix::Errno::ERANGE as libc::c_int
            }

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
            mem:&Vec<String>,
        ) -> Result<(), Error> {
        self.name = buf.write_string(name)?;
        self.passwd = buf.write_string(passwd)?;
        self.gid = gid;
        let mut ptrs = Vec::<*mut libc::c_char>::new();
        for m in mem {
            ptrs.push(buf.write_string(&m)?);
        }
        self.mem = buf.add_pointers(&ptrs)?;
        Ok(())
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
        match unsafe{(*group).pack(
                &mut buffer,
                &team.name,
                "x",
                gid,
                &members)} {
            Ok(_) => libc::c_int::from(NssStatus::Success),
            Err(_) => nix::Errno::ERANGE as libc::c_int,
        }
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
        match unsafe{(*group).pack(
                &mut buffer,
                &team.name,
                "x",
                team.id as libc::gid_t,
                &members
            )} {
            Ok(_) => libc::c_int::from(NssStatus::Success),
            Err(_) => nix::Errno::ERANGE as libc::c_int
        }

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
