use Config;
use buffer::Buffer;
use libc;
use std::io::Error;
use std::path::Path;
use structs::UserConfig;

#[repr(C)]
pub struct Passwd {
    name: *mut libc::c_char,
    passwd: *mut libc::c_char,
    uid: libc::uid_t,
    gid: libc::gid_t,
    gecos: *mut libc::c_char,
    dir: *mut libc::c_char,
    shell: *mut libc::c_char,
}

impl Passwd {
    fn pack(&mut self, buf: &mut Buffer, name: &str, passwd: &str, uid: libc::uid_t, gid: libc::gid_t, gecos: &str,
            dir: &str, shell: &str)
            -> Result<(), Error> {
        self.name = buf.write_string(name)?;
        self.passwd = buf.write_string(passwd)?;
        self.dir = buf.write_string(dir)?;
        self.shell = buf.write_string(shell)?;
        self.gecos = buf.write_string(gecos)?;
        self.uid = uid;
        self.gid = gid;
        Ok(())
    }

    pub fn pack_args(&mut self, buf: &mut Buffer, name: &str, id: u64, gid: u64, conf: &Config) -> Result<(), Error> {
        let home = conf.home.replace("{}", name);
        let sh: String = match UserConfig::new(&Path::new(&home).join(&conf.user_conf_path)) {
            Ok(personal) => match personal.sh {
                Some(sh) => {
                    if Path::new(&sh).exists() {
                        sh
                    } else {
                        conf.sh.clone()
                    }
                }
                None => conf.sh.clone(),
            },
            Err(_) => conf.sh.clone(),
        };
        self.pack(buf, name, "x", id as libc::uid_t, gid as libc::gid_t, "", &home, &sh)
    }
}

#[repr(C)]
pub struct Spwd {
    namp: *mut libc::c_char,
    pwdp: *mut libc::c_char,
    lstchg: libc::c_long,
    min: libc::c_long,
    max: libc::c_long,
    warn: libc::c_long,
    inact: libc::c_long,
    expire: libc::c_long,
    flag: libc::c_ulong,
}

impl Spwd {
    fn pack(&mut self, buf: &mut Buffer, namp: &str, pwdp: &str, lstchg: libc::c_long, min: libc::c_long,
            max: libc::c_long, warn: libc::c_long, inact: libc::c_long, expire: libc::c_long, flag: libc::c_ulong)
            -> Result<(), Error> {
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

    pub fn pack_args(&mut self, buf: &mut Buffer, name: &str, conf: &Config) -> Result<(), Error> {
        let home = conf.home.replace("{}", name);
        let pass: String = match UserConfig::new(&Path::new(&home).join(&conf.user_conf_path)) {
            Ok(personal) => match personal.pass {
                Some(pass) => pass,
                None => String::from("*"),
            },
            Err(_) => String::from("*"),
        };
        self.pack(buf, name, &pass, -1, -1, -1, -1, -1, -1, 0)
    }
}

#[repr(C)]
pub struct Group {
    name: *mut libc::c_char,
    passwd: *mut libc::c_char,
    gid: libc::gid_t,
    mem: *mut *mut libc::c_char,
}

impl Group {
    fn pack(&mut self, buf: &mut Buffer, name: &str, passwd: &str, gid: libc::gid_t, mem: &Vec<&str>)
            -> Result<(), Error> {
        self.name = buf.write_string(name)?;
        self.passwd = buf.write_string(passwd)?;
        self.gid = gid;
        self.mem = buf.write_vecstr(mem)?;
        Ok(())
    }

    pub fn pack_args(&mut self, buf: &mut Buffer, name: &str, id: u64, members: &Vec<&str>) -> Result<(), Error> {
        self.pack(buf, name, "x", id as libc::gid_t, &members)
    }
}
